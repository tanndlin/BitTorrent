use sha1::{Digest, Sha1};

use crate::{
    bencoding::torrent::Torrent,
    connection::{FromByte, Peer, ToByte},
    util::PeerMessageStream,
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpStream,
};

#[derive(Debug)]
pub struct PeerHandshake {
    pub pstr: String,
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl ToByte for PeerHandshake {
    fn to_be_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::<u8>::new();
        buf.push(self.pstr.len() as u8);
        buf.extend_from_slice(self.pstr.as_bytes());
        buf.extend_from_slice(&self.reserved);
        buf.extend_from_slice(&self.info_hash);
        buf.extend_from_slice(&self.peer_id);

        buf
    }
}

impl FromByte for PeerHandshake {
    fn from_be_bytes(bytes: &[u8]) -> Self {
        let pstr_len = bytes[0] as usize;
        let pstr = String::from_utf8(bytes[1..1 + pstr_len].to_vec()).unwrap();
        let mut reserved = [0; 8];
        reserved.copy_from_slice(&bytes[1 + pstr_len..1 + pstr_len + 8]);
        let mut info_hash = [0; 20];
        info_hash.copy_from_slice(&bytes[1 + pstr_len + 8..1 + pstr_len + 28]);
        let mut peer_id = [0; 20];
        peer_id.copy_from_slice(&bytes[1 + pstr_len + 28..1 + pstr_len + 48]);

        PeerHandshake {
            pstr,
            reserved,
            info_hash,
            peer_id,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PeerMessageID {
    KeepAlive = -1,
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
    Port = 9,
}

pub struct PeerMessage {
    pub id: PeerMessageID,
    pub length: u32,
    pub payload: Vec<u8>,
}

impl ToByte for PeerMessage {
    fn to_be_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::<u8>::new();
        buf.extend_from_slice(&(self.length).to_be_bytes());
        buf.push(self.id as u8);
        buf.extend_from_slice(&self.payload);

        buf
    }
}

impl PeerMessage {
    pub fn create_request(index: u32, begin: u32, length: u32) -> Self {
        let mut payload = Vec::<u8>::new();
        payload.extend_from_slice(&index.to_be_bytes());
        payload.extend_from_slice(&begin.to_be_bytes());
        payload.extend_from_slice(&length.to_be_bytes());
        PeerMessage {
            id: PeerMessageID::Request,
            length: 13,
            payload,
        }
    }

    pub fn create_interested() -> Self {
        PeerMessage {
            id: PeerMessageID::Interested,
            length: 1,
            payload: vec![],
        }
    }
}

pub fn connect_to_peer(peer: &Peer, torrent: &Torrent) {
    let mut stream = TcpStream::connect((peer.ip, peer.port)).expect("Failed to connect to peer");
    println!("Connected to peer: {:?}", stream);

    let handshake_request = PeerHandshake {
        pstr: "BitTorrent protocol".to_owned(),
        reserved: [0; 8],
        info_hash: torrent.info_hash,
        peer_id: *b"-TR2940-fuckmek6wWLc",
    };

    let handshake_bytes = handshake_request.to_be_bytes();
    println!("Sending handshake: {:?}", handshake_bytes);
    stream
        .write_all(&handshake_bytes)
        .expect("Failed to send handshake");

    let mut response_buf = [0; 68];
    stream
        .read_exact(&mut response_buf)
        .expect("Failed to read handshake response");
    let handshake_response = PeerHandshake::from_be_bytes(&response_buf);
    println!("Received handshake response: {:?}", handshake_response);

    let mut peer_message_stream = PeerMessageStream::new(stream);
    let mut is_choked = true;
    let mut bitfield: Vec<u32> = vec![];
    let mut pieces = HashMap::<u32, Vec<u8>>::new();

    while pieces.len() < (torrent.info.pieces.len()) {
        println!(
            "Downloaded {}/{} pieces",
            pieces.len(),
            torrent.info.pieces.len()
        );

        loop {
            let message = peer_message_stream.get_next_message();
            handle_message(&message, &mut is_choked, &mut bitfield);

            if is_choked {
                println!("Cannot request pieces, we are choked");
            }

            if bitfield
                .iter()
                .any(|&piece_index| !pieces.contains_key(&piece_index))
            {
                break;
            }
        }

        let needed_piece_index = bitfield
            .iter()
            .find(|&&piece_index| !pieces.contains_key(&piece_index))
            .unwrap();

        let piece = get_piece_from_peer(&mut peer_message_stream, torrent, *needed_piece_index);
        match piece {
            Ok(data) => {
                println!("Successfully downloaded piece: {} bytes", data.len());
                pieces.insert(*needed_piece_index, data);
            }
            Err(e) => {
                println!("Failed to download piece: {}", e);
            }
        }
    }

    // Close stream
    drop(peer_message_stream);
}

fn get_piece_from_peer(
    stream: &mut PeerMessageStream,
    torrent: &Torrent,
    piece_index: u32,
) -> Result<Vec<u8>, String> {
    let interested_message = PeerMessage::create_interested();
    let interested_bytes = interested_message.to_be_bytes();
    println!("Sending interested message: {:?}", interested_bytes);
    stream
        .write_all(&interested_bytes)
        .expect("Failed to send interested message");

    let piece_length = torrent.info.piece_length as u32;
    let mut piece_buffer = vec![0; piece_length as usize];
    let mut start = 0;
    loop {
        let length = if piece_length - start < 16384 {
            piece_length - start
        } else {
            16384
        };
        let request_message = PeerMessage::create_request(piece_index, start, length);
        let request_bytes = request_message.to_be_bytes();
        println!(
            "Sending request for piece index {}: {:?}",
            piece_index, request_bytes
        );
        stream
            .write_all(&request_bytes)
            .expect("Failed to send request");

        let message = stream.get_next_message();
        handle_message(&message, &mut false, &mut vec![]);

        let mut piece_received = false;
        if let PeerMessageID::Piece = message.id {
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            if index != piece_index {
                println!(
                    "Received piece index {} but requested {}",
                    index, piece_index
                );
                continue;
            }

            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let block = &message.payload[8..];
            piece_buffer[begin as usize..begin as usize + block.len()].copy_from_slice(block);
            println!("Received piece index {} from peer", piece_index);
            piece_received = true;
        }

        if !piece_received {
            println!("Did not receive piece, retrying...");
        } else {
            start += 16384;
            if start >= piece_length {
                println!("Completed downloading piece index {}", piece_index);
                break;
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
    println!(
        "Message ID: {:?}, Length: {}, Payload: {:?}",
        message.id, message.length, message.payload
    );

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
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let block = &message.payload[8..];
            println!(
                "Received piece index: {}, begin: {}, block length: {}",
                index,
                begin,
                block.len()
            );
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
    }
}
