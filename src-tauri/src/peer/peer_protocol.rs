use rand::seq::IteratorRandom;
// use tauri::http::request;

use crate::{
    bencoding::{self, torrent::Torrent},
    connection::Peer,
    peer::types::{
        BlockProgress, PeerHandshake, PeerMessage, PeerMessageID, PeerState, PieceProgress,
        TorrentProgress,
    },
    util::peer_message_stream::PeerMessageStream,
};
use std::{
    collections::HashSet,
    fs::create_dir_all,
    io::{Read, Write},
    net::TcpStream,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_INFLIGHT_REQUESTS: u32 = 200;

#[derive(Debug)]
pub enum PeerProtocolError {
    FailedToConnect,
    ConnectionClosed,
    HandshakeError(String),
    ReceivedError(String),
    Unknown(String),
}

pub fn connect_to_peer(
    peer: &Peer,
    torrent: &Torrent,
    progress: Arc<Mutex<TorrentProgress>>,
) -> Result<(), PeerProtocolError> {
    let stream =
        TcpStream::connect((peer.ip, peer.port)).map_err(|_| PeerProtocolError::FailedToConnect)?;
    let peer = format!("{}:{}", peer.ip, peer.port);
    // println!("{} - Connected", peer);

    let mut peer_message_stream = PeerMessageStream::new(stream);
    let mut peer_state = handle_handshake(torrent, &progress, &mut peer_message_stream, peer)?;

    let interested_message = PeerMessage::create_interested();
    let interested_bytes = Vec::from(&interested_message);
    // println!("Sending interested message: {:?}", interested_bytes);
    peer_message_stream
        .write_all(&interested_bytes)
        .expect("Failed to send interested message");

    let mut last_progress_check = Instant::now();
    let mut download_complete = false;

    loop {
        if last_progress_check.elapsed() > Duration::from_millis(500) {
            download_complete = progress
                .lock()
                .unwrap()
                .pieces
                .values()
                .all(|p| matches!(p, PieceProgress::Completed(_)));
            last_progress_check = Instant::now();
        }

        if download_complete {
            break;
        }
        if let Some(message) = peer_message_stream.try_read_message()? {
            handle_message(&message, &mut peer_state, progress.clone());
            continue;
        }

        if !peer_state.is_choked && peer_state.bitfield.is_empty() {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }

        if peer_state.bitfield.is_empty() || peer_state.is_choked {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }

        // Choose 5 random pieces that the peer has and that we don't have
        let completed_pieces: HashSet<u32> = {
            let prog = progress.lock().unwrap();
            prog.pieces
                .iter()
                .filter(|(_, v)| matches!(v, PieceProgress::Completed(_)))
                .map(|(k, _)| *k)
                .collect()
        };

        let needed_pieces = (0..torrent.info.pieces.len() as u32)
            .filter(|&i| bitfield_contains_piece(&peer_state.bitfield, i))
            .filter(|&i| !completed_pieces.contains(&i));

        while peer_state.requested_pieces.len() < 2 {
            if let Some(piece_index) = needed_pieces.clone().choose(&mut rand::rng()) {
                peer_state.requested_pieces.push(piece_index);
            } else {
                break;
            }
        }

        for piece_index in &peer_state.requested_pieces.clone() {
            let mut torrent_progress = progress.lock().unwrap();
            if let PieceProgress::InProgress(piece_progress) =
                torrent_progress.pieces.get_mut(piece_index).unwrap()
            {
                let mut start = 0;
                while start < torrent.get_piece_length(*piece_index as usize)
                    && peer_state.inflight < MAX_INFLIGHT_REQUESTS
                {
                    // println!(
                    //     "Requesting piece index: {}, begin: {}, length: {}",
                    //     piece_index,
                    //     start,
                    //     16 * 1024
                    // );
                    let block_progress = piece_progress.data.get_mut(&start).unwrap();
                    if block_progress.inflight || block_progress.data.is_some() {
                        start += 16 * 1024;
                        continue;
                    }

                    let request_message =
                        PeerMessage::create_request(*piece_index, start, block_progress.length);

                    peer_message_stream
                        .write_all(&Vec::from(&request_message))
                        .expect("Failed to send request message");

                    // Mark block as inflight
                    block_progress.inflight = true;

                    peer_state.inflight += 1;
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

fn handle_handshake(
    torrent: &Torrent,
    progress: &Arc<Mutex<TorrentProgress>>,
    peer_message_stream: &mut PeerMessageStream,
    peer: String,
) -> Result<PeerState, PeerProtocolError> {
    let mut reserved = [0; 8];
    reserved[5] |= 0x10;
    let handshake_request = PeerHandshake {
        pstr: "BitTorrent protocol".to_owned(),
        reserved,
        info_hash: torrent.info_hash,
        peer_id: *b"-TR2940-fuckmek6wWLc",
    };
    let handshake_bytes = Vec::from(&handshake_request);
    // println!("{} - Sending handshake: {:?}", peer, handshake_bytes);
    peer_message_stream
        .write_all(&handshake_bytes)
        .expect("Failed to send handshake");
    let mut response_buf = [0; 68];
    peer_message_stream
        .stream
        .stream
        .read_exact(&mut response_buf)
        .map_err(|e| {
            PeerProtocolError::HandshakeError(format!("Failed to read handshake response: {}", e))
        })?;
    let handshake_response = PeerHandshake::from(response_buf);
    // println!(
    //     "{} - Received handshake response: {:?}",
    //     peer, handshake_response
    // );

    let num_bitfield_bytes = torrent.info.pieces.len().div_ceil(8);
    let peer_state = PeerState::new(peer, num_bitfield_bytes);
    let mut bitfield_payload = vec![0; num_bitfield_bytes];
    for i in 0..torrent.info.pieces.len() {
        let byte_index = i / 8;
        let bit_index = 7 - (i % 8);
        if let PieceProgress::Completed(_) =
            progress.lock().unwrap().pieces.get(&(i as u32)).unwrap()
        {
            bitfield_payload[byte_index] |= 1 << bit_index;
        }
    }
    let bitfield_message = PeerMessage {
        id: PeerMessageID::Bitfield,
        length: (1 + bitfield_payload.len()) as u32,
        payload: bitfield_payload,
    };
    let bitfield_bytes = Vec::from(&bitfield_message);
    peer_message_stream
        .write_all(&bitfield_bytes)
        .map_err(|_| {
            PeerProtocolError::HandshakeError("Failed to send bitfield message".to_string())
        })?;

    Ok(peer_state)
}

fn write_piece_to_file(progress: &TorrentProgress, piece_index: u32) {
    create_dir_all("/pieces").expect("Failed to create pieces directory");

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(format!("/pieces/{}.bin", piece_index))
        .expect("Failed to open file");
    file.write_all(
        &progress.pieces[&piece_index]
            .get_final_data()
            .unwrap()
            .unwrap(),
    )
    .expect("Failed to write piece to file");
}

fn handle_message(
    message: &PeerMessage,
    peer_state: &mut PeerState,
    progress: Arc<Mutex<TorrentProgress>>,
) {
    // println!("Message ID: {:?}, Length: {}", message.id, message.length);

    match message.id {
        PeerMessageID::KeepAlive => {
            // println!("Received keep-alive message");
        }
        PeerMessageID::Choke => {
            // println!("{} - Choked us", peer_state.peer);
            peer_state.is_choked = true;
        }
        PeerMessageID::Unchoke => {
            // println!("{} - Unchoked us", peer_state.peer);
            peer_state.is_choked = false;
        }
        PeerMessageID::Interested => {
            // println!("{} - Is interested", peer_state.peer);
        }
        PeerMessageID::NotInterested => {
            // println!("{} - Is not interested", peer_state.peer);
        }
        PeerMessageID::Have => {
            let piece_index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            // println!("{} - Has piece index: {}", peer_state.peer, piece_index);

            let byte_index = (piece_index / 8) as usize;
            let bit_index = 7 - (piece_index % 8);
            if byte_index < peer_state.bitfield.len() {
                peer_state.bitfield[byte_index] |= 1 << bit_index;
            }
        }
        PeerMessageID::Bitfield => {
            // println!(
            //     "{} - Received bitfield: {:?}",
            //     peer_state.peer, message.payload
            // );
            peer_state.bitfield = message.payload.clone();
        }
        PeerMessageID::Request => {
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let length = u32::from_be_bytes(message.payload[8..12].try_into().unwrap());
            // println!(
            //     "Peer requested piece index: {}, begin: {}, length: {}",
            //     index, begin, length
            // );
        }
        PeerMessageID::Piece => {
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let block = &message.payload[8..];
            // println!(
            //     "{} - Received piece index: {}, begin: {}, block length: {}",
            //     peer_state.peer,
            //     index,
            //     begin,
            //     block.len()
            // );
            peer_state.inflight = peer_state.inflight.saturating_sub(1);

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

                match piece_progress.get_final_data() {
                    Ok(Some(data)) => {
                        // Remove the piece from requested pieces
                        peer_state.requested_pieces.retain(|&i| i != index);
                        Some(data)
                    }
                    Ok(None) => None,
                    Err(e) => {
                        piece_progress.reset();
                        println!(
                            "Error validating piece {}: {}, resetting progress",
                            index, e
                        );
                        None
                    }
                }
            } else {
                println!(
                    "Received piece data for index {} that is not in progress",
                    index
                );
                None
            };

            if let Some(data) = final_data {
                // println!("Completed piece index: {}, writing to file", index);
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
            // println!("Peer's DHT port: {}", port);
        }
        PeerMessageID::Extended => {
            // println!("Received extension message");
            let extension_id = message.payload[0];
            let extension_id_str = match extension_id {
                0 => "ut_metadata",
                1 => "ut_pex",
                2 => "ut_holepunch",
                _ => "unknown",
            };
            // println!("Extension ID: {} ({})", extension_id, extension_id_str);

            let dictionary =
                bencoding::decode::decode_dictionary(&message.payload[1..], &mut 0usize);
            // println!("Decoded extension message: {:?}", dictionary);
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
