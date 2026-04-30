use sha1::{Digest, Sha1};
// use tauri::http::request;

use crate::{
    bencoding::{self, torrent::Torrent},
    connection::Peer,
    peer::types::{PeerHandshake, PeerMessage, PeerMessageID},
    util::peer_message_stream::PeerMessageStream,
};
use std::{
    collections::HashMap,
    fs::create_dir_all,
    io::{Read, Write},
    net::TcpStream,
};

pub fn connect_to_peer(peer: &Peer, torrent: &Torrent) -> Result<(), String> {
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
    let mut bitfield: Vec<u32> = vec![];
    let mut pieces = HashMap::<u32, Vec<u8>>::new();
    // Find all files in pieces directory
    for entry in std::fs::read_dir("pieces").unwrap_or_else(|_| {
        create_dir_all("pieces").expect("Failed to create pieces directory");
        std::fs::read_dir("pieces").expect("Failed to read pieces directory")
    }) {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_stem() {
                if let Some(piece_index) = filename.to_str().and_then(|s| s.parse::<u32>().ok()) {
                    let data = std::fs::read(&path).expect("Failed to read piece file");
                    pieces.insert(piece_index, data);
                    println!("Found existing piece: {}", piece_index);
                }
            }
        }
    }

    println!(
        "Downloaded {}/{} pieces",
        pieces.len(),
        torrent.info.pieces.len()
    );

    // Send my bitfield message
    let num_bytes = torrent.info.pieces.len().div_ceil(8);
    let mut bitfield_payload = vec![0u8; num_bytes];

    for i in 0..torrent.info.pieces.len() {
        let byte_index = i / 8;
        let bit_index = 7 - (i % 8);
        if pieces.contains_key(&(i as u32)) {
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

    while pieces.len() < (torrent.info.pieces.len()) {
        // Check if there are any messages to read from the peer
        if let Some(message) = peer_message_stream.try_read_message() {
            handle_message(&message, &mut is_choked, &mut bitfield);
            continue;
        }

        if !is_choked && bitfield.is_empty() {
            // Just wait for message
            println!("Peer is not choked but has no pieces, waiting for message...");
            let message = peer_message_stream.get_next_message();
            handle_message(&message, &mut is_choked, &mut bitfield);
            continue;
        }

        if !bitfield
            .iter()
            .any(|&piece_index| !pieces.contains_key(&piece_index))
        {
            continue;
        }

        let needed_piece_index = match bitfield
            .iter()
            .find(|&&piece_index| !pieces.contains_key(&piece_index))
        {
            Some(&index) => index,
            None => {
                continue;
            }
        };

        if is_choked {
            continue;
        }

        let piece = get_piece_from_peer(
            &mut peer_message_stream,
            torrent,
            needed_piece_index,
            &mut is_choked,
            &mut bitfield,
        );
        match piece {
            Ok(data) => {
                println!("Successfully downloaded piece: {} bytes", data.len());
                pieces.insert(needed_piece_index, data);
                // write piece to file
                create_dir_all("pieces").expect("Failed to create pieces directory");

                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(format!("pieces/{}.bin", needed_piece_index))
                    .expect("Failed to open file");
                file.write_all(&pieces[&needed_piece_index])
                    .expect("Failed to write piece to file");

                println!(
                    "Downloaded {}/{} pieces",
                    pieces.len(),
                    torrent.info.pieces.len()
                );
            }
            Err(e) => {
                println!("Failed to download piece: {}", e);
            }
        }
    }

    // Close stream
    drop(peer_message_stream);
    println!("Finished downloading all pieces, closing connection to peer");
    Ok(())
}

fn get_piece_from_peer(
    stream: &mut PeerMessageStream,
    torrent: &Torrent,
    piece_index: u32,
    is_choked: &mut bool,
    bitfield: &mut Vec<u32>,
) -> Result<Vec<u8>, String> {
    let piece_length = torrent.info.piece_length as u32;
    let mut piece_buffer = vec![0; piece_length as usize];

    let mut requests = vec![0];
    while requests.last().unwrap() < &piece_length {
        let start = *requests.last().unwrap();
        if start + 16384 < piece_length {
            requests.push(start + 16384);
        } else {
            requests.push(piece_length);
        }
    }

    println!("Sending request for piece index: {}", piece_index);
    for start in &requests {
        let length = if piece_length - start < 16384 {
            piece_length - start
        } else {
            16384
        };
        let request_message = PeerMessage::create_request(piece_index, *start, length);
        let request_bytes = Vec::from(&request_message);
        stream
            .write_all(&request_bytes)
            .expect("Failed to send request");
    }

    let mut received_blocks = 0;

    while received_blocks < requests.len() {
        let message = stream.get_next_message();
        handle_message(&message, is_choked, bitfield);

        // Check if it's a piece message
        if matches!(message.id, PeerMessageID::Piece) {
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let block = &message.payload[8..];

            if index == piece_index {
                piece_buffer[begin as usize..(begin + block.len() as u32) as usize]
                    .copy_from_slice(block);
                received_blocks += 1;
                // println!(
                //     "Received block for piece index {}: begin {}, length {}",
                //     index,
                //     begin,
                //     block.len()
                // );
            }
        }
    }

    let mut hasher = Sha1::new();
    hasher.update(&piece_buffer);
    let piece_hash: [u8; 20] = hasher.finalize().into();
    let expected_hash = &torrent.info.pieces[piece_index as usize];
    if piece_hash == *expected_hash {
        println!("Piece index {} verified successfully", piece_index);
        Ok(piece_buffer)
    } else {
        Err(format!("Piece index {} verification failed", piece_index))
    }
}

fn handle_message(message: &PeerMessage, is_choked: &mut bool, bitfield: &mut Vec<u32>) {
    println!("Message ID: {:?}, Length: {}", message.id, message.length);

    match message.id {
        PeerMessageID::KeepAlive => {
            println!("Received keep-alive message");
        }
        PeerMessageID::Choke => {
            *is_choked = true;
            println!("Peer choked us");
        }
        PeerMessageID::Unchoke => {
            *is_choked = false;
            println!("Peer unchoked us");
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
            bitfield.push(piece_index);
        }
        PeerMessageID::Bitfield => {
            *bitfield = message
                .payload
                .iter()
                .flat_map(|byte| {
                    (0..8)
                        .rev()
                        .map(move |bit| if (byte >> bit) & 1 == 1 { 1 } else { 0 })
                })
                .collect();
            println!("Received bitfield: {:?}", message.payload);
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
            // let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            // let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            // let block = &message.payload[8..];
            // println!(
            //     "Received piece index: {}, begin: {}, block length: {}",
            //     index,
            //     begin,
            //     block.len()
            // );
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
            println!("Received extension message: {:?}", message.payload);
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
