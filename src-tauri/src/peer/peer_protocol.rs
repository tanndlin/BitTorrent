use rand::seq::IteratorRandom;
use sha1::{Digest, Sha1};
// use tauri::http::request;

use crate::{
    bencoding::{self, torrent::Torrent},
    connection::Peer,
    peer::types::{
        BlockProgress, PeerHandshake, PeerMessage, PeerMessageID, PieceProgress, PieceProgressData,
        TorrentProgress,
    },
    util::peer_message_stream::PeerMessageStream,
};
use std::{
    fs::create_dir_all,
    io::{Read, Write},
    net::TcpStream,
    sync::{Arc, Mutex},
};

pub fn connect_to_peer(
    peer: &Peer,
    torrent: &Torrent,
    progress: Arc<Mutex<TorrentProgress>>,
) -> Result<(), String> {
    let mut stream = TcpStream::connect((peer.ip, peer.port))
        .map_err(|e| format!("Failed to connect to peer {}: {}", peer.ip, e))?;
    println!("Connected to peer: {:?}", stream);

    let mut reserved = [0; 8];
    reserved[5] |= 0x10;

    let handshake_request = PeerHandshake {
        pstr: "BitTorrent protocol".to_owned(),
        reserved,
        info_hash: torrent.info_hash,
        peer_id: *b"-TR2940-fuckmek6wWLc",
    };

    let handshake_bytes = Vec::from(&handshake_request);
    println!("Sending handshake: {:?}", handshake_bytes);
    stream
        .write_all(&handshake_bytes)
        .expect("Failed to send handshake");

    let mut response_buf = [0; 68];
    stream
        .read_exact(&mut response_buf)
        .expect("Failed to read handshake response");
    let handshake_response = PeerHandshake::from(response_buf);
    println!("Received handshake response: {:?}", handshake_response);

    let mut peer_message_stream = PeerMessageStream::new(stream);
    let mut is_choked = true;
    let mut bitfield: Vec<u8> = vec![];
    let mut inflight = 0u32;
    // Fill bitfield with 0s for all pieces
    let num_bytes = torrent.info.pieces.len().div_ceil(8);
    bitfield.resize(num_bytes, 0);

    // Send my bitfield message
    let num_bytes = torrent.info.pieces.len().div_ceil(8);
    let mut bitfield_payload = vec![0u8; num_bytes];

    for i in 0..torrent.info.pieces.len() {
        let byte_index = i / 8;
        let bit_index = 7 - (i % 8);
        if progress.lock().unwrap().pieces.contains_key(&(i as u32)) {
            bitfield_payload[byte_index] |= 1 << bit_index;
        }
    }
    let bitfield_message = PeerMessage {
        id: PeerMessageID::Bitfield,
        length: (1 + bitfield_payload.len()) as u32,
        payload: bitfield_payload,
    };
    let bitfield_bytes = Vec::from(&bitfield_message);
    println!(
        "Sending bitfield message of length: {:?}",
        bitfield_bytes.len()
    );
    peer_message_stream
        .write_all(&bitfield_bytes)
        .expect("Failed to send bitfield message");

    let interested_message = PeerMessage::create_interested();
    let interested_bytes = Vec::from(&interested_message);
    println!("Sending interested message: {:?}", interested_bytes);
    peer_message_stream
        .write_all(&interested_bytes)
        .expect("Failed to send interested message");

    while progress
        .lock()
        .unwrap()
        .pieces
        .values()
        .any(|p| !matches!(p, PieceProgress::Completed(_)))
    {
        println!("Checking for messages from peer...");

        match peer_message_stream.try_read_message() {
            Ok(Some(message)) => {
                handle_message(&message, &mut is_choked, &mut bitfield, progress.clone());
                continue;
            }
            Ok(None) => {}
            Err(e) => {
                println!("Error reading from peer: {}, closing connection", e);
                return Err(format!("Error reading from peer: {}", e));
            }
        }

        println!("No messages from peer, checking if we can request pieces...");

        if !is_choked && bitfield.is_empty() {
            // Just wait for message
            println!("Peer is not choked but has no pieces, waiting for message...");
            let message = peer_message_stream.get_next_message();
            handle_message(&message, &mut is_choked, &mut bitfield, progress.clone());
            continue;
        }

        if bitfield.is_empty() {
            continue;
        }

        if is_choked {
            continue;
        }

        println!("Peer is not choked and has pieces, checking for needed pieces...");

        // Choose 5 random pieces that the peer has and that we don't have
        let needed_pieces = (0..torrent.info.pieces.len() as u32)
            .filter(|&i| bitfield_contains_piece(&bitfield, i))
            .filter(|&i| {
                !matches!(
                    progress.lock().unwrap().pieces.get(&i),
                    Some(PieceProgress::Completed(_))
                )
            })
            .choose_multiple(&mut rand::rng(), 5);

        println!("Needed pieces: {:?}", needed_pieces);

        for piece_index in needed_pieces {
            let torrent_progress = progress.lock().unwrap();
            if let PieceProgress::InProgress(piece_progress) =
                torrent_progress.pieces.get(&piece_index).unwrap()
            {
                println!("Requesting piece index: {}", piece_index);
                let mut start = 0;
                while start < torrent.get_piece_length(piece_index as usize) && inflight < 5 {
                    let block_progress = piece_progress.data.get(&start).unwrap();
                    if block_progress.inflight {
                        start += 16 * 1024;
                        continue;
                    }

                    let request_message =
                        PeerMessage::create_request(piece_index, start, block_progress.length);

                    peer_message_stream
                        .write_all(&Vec::from(&request_message))
                        .expect("Failed to send request message");
                    inflight += 1;
                    start += 16 * 1024;
                }
            }
        }
    }

    // Close stream
    drop(peer_message_stream);
    println!("Finished downloading all pieces, closing connection to peer");
    Ok(())
}

fn write_piece_to_file(progress: &TorrentProgress, piece_index: u32) {
    create_dir_all("/pieces").expect("Failed to create pieces directory");

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(format!("/pieces/{}.bin", piece_index))
        .expect("Failed to open file");
    file.write_all(&progress.pieces[&piece_index].get_final_data().unwrap())
        .expect("Failed to write piece to file");
}

fn handle_message(
    message: &PeerMessage,
    is_choked: &mut bool,
    bitfield: &mut Vec<u8>,
    progress: Arc<Mutex<TorrentProgress>>,
) {
    // println!("Message ID: {:?}, Length: {}", message.id, message.length);

    match message.id {
        PeerMessageID::KeepAlive => {
            println!("Received keep-alive message");
        }
        PeerMessageID::Choke => {
            println!("Peer choked us");
            *is_choked = true;
        }
        PeerMessageID::Unchoke => {
            println!("Peer unchoked us");
            *is_choked = false;
        }
        PeerMessageID::Interested => {
            println!("Peer is interested");
        }
        PeerMessageID::NotInterested => {
            println!("Peer is not interested");
        }
        PeerMessageID::Have => {
            let piece_index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            println!("Peer has piece index: {}", piece_index);

            let byte_index = (piece_index / 8) as usize;
            let bit_index = 7 - (piece_index % 8);
            if byte_index < bitfield.len() {
                bitfield[byte_index] |= 1 << bit_index;
            }
        }
        PeerMessageID::Bitfield => {
            println!("Received bitfield: {:?}", message.payload);
            *bitfield = message.payload.clone();
        }
        PeerMessageID::Request => {
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let length = u32::from_be_bytes(message.payload[8..12].try_into().unwrap());
            println!(
                "Peer requested piece index: {}, begin: {}, length: {}",
                index, begin, length
            );
        }
        PeerMessageID::Piece => {
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let block = &message.payload[8..];
            println!(
                "Received piece index: {}, begin: {}, block length: {}",
                index,
                begin,
                block.len()
            );

            let mut progress = progress.lock().unwrap();
            let final_data = if let Some(PieceProgress::InProgress(piece_progress)) =
                progress.pieces.get_mut(&index)
            {
                piece_progress.data.insert(
                    begin,
                    BlockProgress {
                        begin,
                        length: block.len() as u32,
                        inflight: false,
                        data: Some(block.to_vec()),
                    },
                );
                piece_progress.get_final_data()
            } else {
                println!(
                    "Received piece data for index {} that is not in progress",
                    index
                );
                None
            };

            if let Some(data) = final_data {
                progress
                    .pieces
                    .insert(index, PieceProgress::Completed(data));
                write_piece_to_file(&progress, index);
            }
        }
        PeerMessageID::Cancel => {
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let length = u32::from_be_bytes(message.payload[8..12].try_into().unwrap());
            println!(
                "Peer canceled request for piece index: {}, begin: {}, length: {}",
                index, begin, length
            );
        }
        PeerMessageID::Port => {
            let port = u16::from_be_bytes(message.payload[0..2].try_into().unwrap());
            println!("Peer's DHT port: {}", port);
        }
        PeerMessageID::Extended => {
            println!("Received extension message");
            let extension_id = message.payload[0];
            let extension_id_str = match extension_id {
                0 => "ut_metadata",
                1 => "ut_pex",
                2 => "ut_holepunch",
                _ => "unknown",
            };
            println!("Extension ID: {} ({})", extension_id, extension_id_str);

            let dictionary =
                bencoding::decode::parse_dictionary(&message.payload[1..], &mut 0usize);
            println!("Decoded extension message: {:?}", dictionary);
        }
    }
}

fn bitfield_contains_piece(bitfield: &[u8], piece_index: u32) -> bool {
    let byte_index = piece_index / 8;
    let bit_index = 7 - (piece_index % 8);
    if let Some(byte) = bitfield.get(byte_index as usize) {
        (byte >> bit_index) & 1 == 1
    } else {
        false
    }
}
