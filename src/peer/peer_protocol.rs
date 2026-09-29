use rand::seq::IteratorRandom;

use crate::{
    bencoding::torrent::Torrent,
    connection::Peer,
    peer::types::{
        PeerHandshake, PeerMessage, PeerMessageID, PeerState, PieceProgress, TorrentProgress,
    },
    util::{peer_message_stream::PeerMessageStream, CancellationToken},
};
use std::{
    fmt::Display,
    net::{SocketAddr, TcpStream},
    sync::{
        atomic::{AtomicU64, Ordering::SeqCst},
        mpsc::Sender,
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
            PeerProtocolError::HandshakeError(e) => f.write_str(&format!("Handshake error: {e}")),
            PeerProtocolError::ReceivedError(e) => f.write_str(&format!("Received error: {e}")),
            PeerProtocolError::Unknown(e) => f.write_str(&format!("Unknown error: {e}")),
        }
    }
}

pub fn connect_to_peer(
    peer: &Peer,
    torrent: &Torrent,
    progress: &Arc<RwLock<TorrentProgress>>,
    num_completed_pieces: &Arc<AtomicU64>,
    mut tx: Sender<Option<(u32, Vec<u8>)>>,
    cancel: &CancellationToken,
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
    // println!("{} - Connected", peer);

    let mut peer_message_stream = PeerMessageStream::new(stream);

    let num_bitfield_bytes = torrent.info.pieces.len().div_ceil(8);
    let mut peer_state = PeerState::new(peer.clone(), num_bitfield_bytes);
    handle_handshake(torrent, progress, &mut peer_message_stream)?;

    // The handshake needs the long timeout; after that, reads should return
    // quickly so the loop can keep sending requests
    peer_message_stream
        .set_read_timeout(Duration::from_millis(1))
        .map_err(|_| PeerProtocolError::ConnectionClosed)?;

    peer_message_stream
        .write_all(&PeerMessage::create_interested())
        .map_err(|_| PeerProtocolError::ConnectionClosed)?;

    let mut rand = rand::rng();
    while !cancel.is_cancelled() {
        if let Some(message) = peer_message_stream.try_read_message()? {
            handle_message(
                message,
                &mut peer_state,
                &mut peer_message_stream,
                progress,
                num_completed_pieces,
                &mut tx,
            )?;
        };

        // Once everything is downloaded, only answer the peer (seeding)
        if num_completed_pieces.load(SeqCst) < torrent.info.pieces.len() as u64 {
            if peer_state.bitfield.is_empty() || peer_state.is_choked {
                continue;
            }

            // Choose 5 random pieces that the peer has and that we don't have
            {
                let progress = progress.read().unwrap();
                // Remove the piece from requested pieces if another peer already completed it
                peer_state
                    .requested_pieces
                    .retain(|piece_index| progress.needed_pieces.contains(piece_index));

                let missing = 5usize.saturating_sub(peer_state.requested_pieces.len());
                if missing > 0 {
                    let new_pieces = progress
                        .needed_pieces
                        .iter()
                        .copied()
                        .filter(|&piece_index| {
                            bitfield_contains_piece(&peer_state.bitfield, piece_index)
                                && !peer_state.requested_pieces.contains(&piece_index)
                        })
                        .choose_multiple(&mut rand, missing);
                    peer_state.requested_pieces.extend(new_pieces);
                }
            }

            let mut request_bytes = vec![];
            for piece_index in &peer_state.requested_pieces.clone() {
                if let PieceProgress::InProgress(piece_progress) =
                    &mut *progress.read().unwrap().pieces[*piece_index as usize]
                        .lock()
                        .unwrap()
                {
                    let mut start = 0;
                    let mut begin = 0;
                    while begin < torrent.get_piece_length(*piece_index as usize)
                        && peer_state.inflight < MAX_INFLIGHT_REQUESTS
                    {
                        // println!(
                        //     "Requesting piece index: {}, begin: {}, length: {}",
                        //     piece_index,
                        //     start,
                        //     16 * 1024
                        // );
                        let block_progress = piece_progress.data.get_mut(start as usize).unwrap();
                        if block_progress.inflight || block_progress.complete {
                            start += 1;
                            begin += 16 * 1024;
                            continue;
                        }

                        PeerMessage::create_request(*piece_index, begin, block_progress.length)
                            .encode_to(&mut request_bytes);

                        // Mark block as inflight
                        block_progress.inflight = true;

                        peer_state.inflight += 1;
                        start += 1;
                        begin += 16 * 1024;
                    }
                }
            }

            peer_message_stream
                .write_all(&request_bytes)
                .map_err(|_| PeerProtocolError::ConnectionClosed)?;
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
) -> Result<(), PeerProtocolError> {
    let mut reserved = [0; 8];
    reserved[5] |= 0x10;
    let handshake_request = PeerHandshake {
        reserved,
        info_hash: torrent.info_hash,
        peer_id: *b"-TR2940-fuckmek6wWLc",
    };

    peer_message_stream
        .write_all(&handshake_request)
        .map_err(|e| PeerProtocolError::HandshakeError(format!("Failed to send handshake: {e}")))?;
    let mut response_buf = [0; 68];
    peer_message_stream
        .read_exact(&mut response_buf)
        .map_err(|e| {
            PeerProtocolError::HandshakeError(format!("Failed to read handshake response: {e}"))
        })?;

    let handshake =
        PeerHandshake::try_from(response_buf).map_err(PeerProtocolError::HandshakeError)?;
    if handshake.info_hash != torrent.info_hash {
        return Err(PeerProtocolError::HandshakeError(
            "Received incorrect info_hash".to_string(),
        ));
    }

    let num_bitfield_bytes = torrent.info.pieces.len().div_ceil(8);
    let mut bitfield_payload = vec![0; num_bitfield_bytes];
    {
        let progress = progress.read().unwrap();
        for i in 0..torrent.info.pieces.len() {
            let byte_index = i / 8;
            let bit_index = 7 - (i % 8);
            if let PieceProgress::Completed = &mut *progress.pieces.get(i).unwrap().lock().unwrap()
            {
                bitfield_payload[byte_index] |= 1 << bit_index;
            }
        }
    }

    let bitfield_message = PeerMessage {
        id: PeerMessageID::Bitfield,
        length: (1 + bitfield_payload.len()) as u32,
        payload: bitfield_payload,
    };
    peer_message_stream
        .write_all(&bitfield_message)
        .map_err(|_| {
            PeerProtocolError::HandshakeError("Failed to send bitfield message".to_string())
        })?;

    peer_message_stream
        .write_all(&PeerMessage::create_unchoke())
        .map_err(|_| {
            PeerProtocolError::HandshakeError("Failed to send unchoke message".to_string())
        })?;

    Ok(())
}

fn handle_message(
    message: PeerMessage,
    peer_state: &mut PeerState,
    peer_message_stream: &mut PeerMessageStream,
    progress: &Arc<RwLock<TorrentProgress>>,
    completed_pieces: &Arc<AtomicU64>,
    tx: &mut Sender<Option<(u32, Vec<u8>)>>,
) -> Result<(), PeerProtocolError> {
    // println!("Message ID: {:?}, Length: {}", message.id, message.length);

    #[allow(clippy::match_same_arms)]
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
            peer_state.bitfield = message.payload;
        }
        PeerMessageID::Request => {
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let length = u32::from_be_bytes(message.payload[8..12].try_into().unwrap());
            // println!(
            //     "Peer requested piece index: {}, begin: {}, length: {}",
            //     index, begin, length
            // );

            let block = progress.read().unwrap().get_block(index, begin, length);
            match block {
                Ok(data) => peer_message_stream
                    .write_all(&PeerMessage::create_piece(index, begin, &data))
                    .map_err(|_| PeerProtocolError::ConnectionClosed)?,
                Err(e) => println!("{} - Ignoring request: {e}", peer_state.peer),
            }
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

            let progress_read = progress.read().unwrap();
            let mut piece = progress_read.pieces[index as usize].lock().unwrap();
            let final_data = if let PieceProgress::InProgress(piece_progress) = &mut *piece {
                if begin % (16 * 1024) == 0 {
                    piece_progress.add_data((begin / (16 * 1024)) as usize, block);
                }

                match piece_progress.get_final_data() {
                    Ok(Some(data)) => {
                        // Remove the piece from requested pieces
                        peer_state.requested_pieces.retain(|&i| i != index);
                        *piece = PieceProgress::Completed;
                        Some(data)
                    }
                    Ok(None) => None,
                    Err(e) => {
                        piece_progress.reset();
                        println!("Error validating piece {index}: {e}, resetting progress");
                        None
                    }
                }
            } else {
                println!("Received piece data for index {index} that is not in progress");
                None
            };

            drop(piece);
            drop(progress_read);

            if let Some(data) = final_data {
                // println!("Completed piece index: {}, writing to file", index);
                progress.write().unwrap().needed_pieces.remove(&index);

                tx.send(Some((index, data))).unwrap();
                completed_pieces.fetch_add(1, SeqCst);
            }
        }
        PeerMessageID::Cancel => {
            let index = u32::from_be_bytes(message.payload[0..4].try_into().unwrap());
            let begin = u32::from_be_bytes(message.payload[4..8].try_into().unwrap());
            let length = u32::from_be_bytes(message.payload[8..12].try_into().unwrap());
            println!(
                "Peer canceled request for piece index: {index}, begin: {begin}, length: {length}"
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
    Ok(())
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
