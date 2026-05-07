use std::collections::{HashMap, HashSet};

use sha1::{Digest, Sha1};

use crate::{bencoding::torrent::Torrent, connection::Peer};

#[derive(Debug)]
pub struct PeerHandshake {
    pub pstr: String,
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl From<&PeerHandshake> for Vec<u8> {
    fn from(handshake: &PeerHandshake) -> Vec<u8> {
        let mut buf = vec![];
        buf.push(handshake.pstr.len() as u8);
        buf.extend_from_slice(handshake.pstr.as_bytes());
        buf.extend_from_slice(&handshake.reserved);
        buf.extend_from_slice(&handshake.info_hash);
        buf.extend_from_slice(&handshake.peer_id);

        buf
    }
}

impl From<[u8; 68]> for PeerHandshake {
    fn from(bytes: [u8; 68]) -> Self {
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
    Extended = 20,
}

#[derive(Debug)]
pub struct PeerMessage {
    pub id: PeerMessageID,
    pub length: u32,
    pub payload: Vec<u8>,
}

impl From<&PeerMessage> for Vec<u8> {
    fn from(message: &PeerMessage) -> Self {
        let mut buf = vec![];
        buf.extend_from_slice(&(message.length).to_be_bytes());
        buf.push(message.id as u8);
        buf.extend_from_slice(&message.payload);

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

pub struct TorrentProgress {
    pub pieces: HashMap<u32, PieceProgress>,
    pub connected_peers: HashSet<Peer>,
}

impl From<&Torrent> for TorrentProgress {
    fn from(torrent: &Torrent) -> Self {
        let pieces = torrent
            .info
            .pieces
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let mut data = HashMap::<_, _>::new();
                let piece_length = torrent.get_piece_length(i);
                let block_size = 16 * 1024; // 16 KB blocks
                let mut offset = 0;
                while offset < piece_length {
                    let block_length = std::cmp::min(block_size, piece_length - offset);
                    data.insert(
                        offset,
                        BlockProgress {
                            begin: offset,
                            length: block_length,
                            inflight: false,
                            data: None,
                        },
                    );
                    offset += block_length;
                }

                (
                    i as u32,
                    PieceProgress::InProgress(PieceProgressData {
                        index: i as u32,
                        length: torrent.get_piece_length(i),
                        data,
                        expected_hash: torrent.info.pieces[i],
                    }),
                )
            })
            .collect();

        TorrentProgress {
            pieces,
            connected_peers: HashSet::new(),
        }
    }
}

pub enum PieceProgress {
    InProgress(PieceProgressData),
    Completed(Vec<u8>),
}

impl PieceProgress {
    pub fn get_final_data(&self) -> Result<Option<Vec<u8>>, String> {
        match self {
            PieceProgress::Completed(data) => Ok(Some(data.clone())),
            PieceProgress::InProgress(progress) => progress.get_final_data(),
        }
    }
}

pub struct PieceProgressData {
    pub index: u32,
    pub length: u32,
    pub data: HashMap<u32, BlockProgress>,
    pub expected_hash: [u8; 20],
}

impl PieceProgressData {
    pub fn get_final_data(&self) -> Result<Option<Vec<u8>>, String> {
        let mut final_data = vec![0; self.length as usize];

        let keys_sorted: Vec<u32> = self.data.keys().cloned().collect();
        for begin in keys_sorted {
            let block_progress = self.data.get(&begin).unwrap();
            if block_progress.data.is_none() {
                return Ok(None);
            }

            let block_data = block_progress.data.as_ref().unwrap();
            final_data[begin as usize..(begin + block_progress.length) as usize]
                .copy_from_slice(block_data);
        }

        // Chech hash
        let mut hasher = Sha1::new();
        hasher.update(&final_data);
        let piece_hash: [u8; 20] = hasher.finalize().into();
        if piece_hash == self.expected_hash {
            return Ok(Some(final_data));
        }

        Err(format!(
            "Hash mismatch for piece {}: expected {:x?}, got {:x?}",
            self.index, self.expected_hash, piece_hash
        ))
    }

    pub fn reset(&mut self) {
        self.data.iter_mut().for_each(|(_, block)| {
            block.inflight = false;
            block.data = None;
        });
    }
}

pub struct BlockProgress {
    pub begin: u32,
    pub length: u32,
    pub inflight: bool,
    pub data: Option<Vec<u8>>,
}

pub struct PeerState {
    pub peer: String,
    pub is_choked: bool,
    pub inflight: u32,
    pub bitfield: Vec<u8>,
    pub requested_pieces: Vec<u32>,
}

impl PeerState {
    pub fn new(peer: String, num_bitfield_bytes: usize) -> Self {
        let is_choked = true;
        let inflight = 0u32;
        let bitfield: Vec<u8> = vec![0; num_bitfield_bytes];
        PeerState {
            peer,
            is_choked,
            inflight,
            bitfield,
            requested_pieces: vec![],
        }
    }
}
