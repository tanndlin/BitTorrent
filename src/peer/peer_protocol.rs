use rand::seq::{IndexedRandom, IteratorRandom};

use crate::{
    bencoding::torrent::Torrent,
    connection::Peer,
    peer::types::{
        BlockProgress, PeerHandshake, PeerMessage, PeerMessageID, PeerState, PieceProgress,
        TorrentProgress,
    },
    util::peer_message_stream::PeerMessageStream,
};
use std::{
    collections::HashSet,
    fmt::Display,
    io::Read,
    net::{SocketAddr, TcpStream},
    sync::{
        atomic::{AtomicU64, Ordering::SeqCst},
        Arc, RwLock,
    },
    time::Duration,
};

const MAX_INFLIGHT_REQUESTS: u32 = 200;
const IO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum PeerProtocolError {
    FailedToConnect,
    ConnectionClosed,
    HandshakeError(String),
    ReceivedError(String),
    Unknown(String),
}

impl Display for PeerProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerProtocolError::FailedToConnect => f.write_str("Failed to connect"),
            PeerProtocolError::ConnectionClosed => f.write_str("Connection closed"),
            PeerProtocolError::HandshakeError(e) => f.write_str(&format!("Handshake error: {}", e)),
            PeerProtocolError::ReceivedError(e) => f.write_str(&format!("Received error: {}", e)),
            PeerProtocolError::Unknown(e) => f.write_str(&format!("Unknown error: {}", e)),
        }
    }
}

pub fn connect_to_peer(
    peer: &Peer,
    torrent: &Torrent,
    progress: Arc<RwLock<TorrentProgress>>,
    num_completed_pieces: Arc<AtomicU64>,
) -> Result<(), PeerProtocolError> {
    let stream = TcpStream::connect_timeout(&SocketAddr::new(peer.ip, peer.port), IO_TIMEOUT)
        .map_err(|_| PeerProtocolError::FailedToConnect)?;
    // Without these, a peer that accepts the connection but never answers the
    // handshake (or stops reading) blocks this thread forever
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|_| PeerProtocolError::FailedToConnect)?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|_| PeerProtocolError::FailedToConnect)?;
    let peer = format!("{}:{}", peer.ip, peer.port);
    // println!("{} - Connected", peer);

    let mut peer_message_stream = PeerMessageStream::new(stream);
    let mut peer_state = handle_handshake(torrent, &progress, &mut peer_message_stream, peer)?;

    let interested_message = PeerMessage::create_interested();
    let interested_bytes = Vec::from(&interested_message);
    // println!("Sending interested message: {:?}", interested_bytes);
    peer_message_stream
        .write_all(&interested_bytes)
        .map_err(|_| PeerProtocolError::ConnectionClosed)?;

    let mut rand = rand::rng();

    while num_completed_pieces.load(SeqCst) < torrent.info.pieces.len() as u64 {
        let got_message = if let Some(message) = peer_message_stream.try_read_message()? {
            handle_message(
                &message,
                &mut peer_state,
                progress.clone(),
                num_completed_pieces.clone(),
            );
            true
        } else {
            false
        };

        if !got_message {
            std::thread::sleep(Duration::from_millis(10));
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
        let needed_pieces: Vec<u32> = progress
            .read()
            .unwrap()
            .needed_pieces
            .iter()
            .copied()
            .filter(|&piece_index| bitfield_contains_piece(&peer_state.bitfield, piece_index))
            .collect();

        while peer_state.requested_pieces.len() < 5 {
            if let Some(piece_index) = needed_pieces.choose(&mut rand) {
                peer_state.requested_pieces.push(*piece_index);
            } else {
                break;
            }
        }

        for piece_index in &peer_state.requested_pieces.clone() {
            let mut torrent_progress = progress.write().unwrap();
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
                    if block_progress.inflight || block_progress.complete {
                        start += 16 * 1024;
                        continue;
                    }

                    let request_message =
                        PeerMessage::create_request(*piece_index, start, block_progress.length);

                    peer_message_stream
                        .write_all(&Vec::from(&request_message))
                        .map_err(|_| PeerProtocolError::ConnectionClosed)?;

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
    Ok(())
}

fn handle_handshake(
    torrent: &Torrent,
    progress: &Arc<RwLock<TorrentProgress>>,
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
        .map_err(|e| {
            PeerProtocolError::HandshakeError(format!("Failed to send handshake: {}", e))
        })?;
    let mut response_buf = [0; 68];
    peer_message_stream
        .stream
        .stream
        .read_exact(&mut response_buf)
        .map_err(|e| {
            PeerProtocolError::HandshakeError(format!("Failed to read handshake response: {}", e))
        })?;

    PeerHandshake::try_from(response_buf).map_err(PeerProtocolError::HandshakeError)?;

    let num_bitfield_bytes = torrent.info.pieces.len().div_ceil(8);
    let peer_state = PeerState::new(peer, num_bitfield_bytes);
    let mut bitfield_payload = vec![0; num_bitfield_bytes];
    {
        let progress = progress.read().unwrap();
        for i in 0u32..torrent.info.pieces.len() as u32 {
            let byte_index = i / 8;
            let bit_index = 7 - (i % 8);
            if let PieceProgress::Completed = &mut *progress.pieces.get(&i).unwrap().lock().unwrap()
            {
                bitfield_payload[byte_index as usize] |= 1 << bit_index;
            }
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

fn handle_message(
    message: &PeerMessage,
    peer_state: &mut PeerState,
    progress: Arc<RwLock<TorrentProgress>>,
    completed_pieces: Arc<AtomicU64>,
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

            let mut progress = progress.write().unwrap();
            let final_data = if let Some(PieceProgress::InProgress(piece_progress)) =
                progress.pieces.get_mut(&index)
            {
                piece_progress.add_data(begin, block);

                match piece_progress.get_final_data() {
                    Ok(Some(data)) => {
                        // Remove the piece from requested pieces
                        peer_state.requested_pieces.retain(|&i| i != index);
                        progress.pieces.insert(index, PieceProgress::Completed);
                        progress.needed_pieces.remove(&index);
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
                progress.journal.write_piece(index, &data).unwrap();
                completed_pieces.fetch_add(1, SeqCst);
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

            // let dictionary =
            //     bencoding::decode::decode_dictionary(&message.payload[1..], &mut 0usize);
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
