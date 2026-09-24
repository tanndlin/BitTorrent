use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};

use crate::{bencoding::torrent::Torrent, connection::Peer, util::Journal};

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

impl TryFrom<[u8; 68]> for PeerHandshake {
    type Error = String;
    fn try_from(bytes: [u8; 68]) -> Result<Self, Self::Error> {
        let pstr_len = bytes[0] as usize;
        let pstr = String::from_utf8(bytes[1..1 + pstr_len].to_vec())
            .map_err(|_| "Unable to parse PSTR")?;
        let mut reserved = [0; 8];
        reserved.copy_from_slice(&bytes[1 + pstr_len..1 + pstr_len + 8]);
        let mut info_hash = [0; 20];
        info_hash.copy_from_slice(&bytes[1 + pstr_len + 8..1 + pstr_len + 28]);
        let mut peer_id = [0; 20];
        peer_id.copy_from_slice(&bytes[1 + pstr_len + 28..1 + pstr_len + 48]);

        Ok(PeerHandshake {
            pstr,
            reserved,
            info_hash,
            peer_id,
        })
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
    pub journal: Journal,
    pub pieces: HashMap<u32, PieceProgress>,
    pub needed_pieces: HashSet<u32>,
    pub connected_peers: HashSet<Peer>,
}

impl From<&Torrent> for TorrentProgress {
    fn from(torrent: &Torrent) -> Self {
        let journal = Journal::new(
            &torrent.info.name,
            torrent.total_length() as usize,
            torrent.info.piece_length as usize,
        )
        .unwrap();

        let mut needed_pieces = HashSet::new();
        let pieces: HashMap<_, _> =
            (0u32..torrent.total_length().div_ceil(torrent.info.piece_length) as u32)
                .map(|piece_index| {
                    let written = journal
                        .pieces_written
                        .get(&(piece_index))
                        .copied()
                        .unwrap_or(false);
                    if written {
                        return (piece_index, PieceProgress::Completed);
                    }

                    needed_pieces.insert(piece_index);
                    let piece_length = torrent.get_piece_length(piece_index as usize);

                    (
                        piece_index,
                        PieceProgress::InProgress(PieceProgressData::new(
                            piece_index,
                            piece_length,
                            torrent.info.pieces[piece_index as usize],
                        )),
                    )
                })
                .collect();

        TorrentProgress {
            journal,
            pieces,
            needed_pieces,
            connected_peers: HashSet::new(),
        }
    }
}

pub enum PieceProgress {
    InProgress(PieceProgressData),
    Completed,
}

pub struct PieceProgressData {
    index: u32,
    length: u32,
    total_blocks: u32,
    completed_blocks: u32,
    buffer: Vec<u8>, // length bytes, allocated on first block to save upfront memory cost
    pub data: HashMap<u32, BlockProgress>,
    expected_hash: [u8; 20],
}

impl PieceProgressData {
    pub fn new(index: u32, length: u32, expected_hash: [u8; 20]) -> Self {
        let block_size = 16 * 1024; // 16 KB blocks
        let mut data = HashMap::new();
        let mut offset = 0;
        while offset < length {
            let block_length = std::cmp::min(block_size, length - offset);
            data.insert(
                offset,
                BlockProgress {
                    length: block_length,
                    inflight: false,
                    complete: false,
                },
            );
            offset += block_length;
        }

        Self {
            index,
            length,
            total_blocks: data.len() as u32,
            completed_blocks: 0,
            // Allocated on the first block, so pieces that haven't started cost nothing
            buffer: Vec::new(),
            data,
            expected_hash,
        }
    }

    pub fn get_final_data(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.completed_blocks < self.total_blocks {
            return Ok(None);
        }

        // Chech hash
        let mut hasher = Sha1::new();
        hasher.update(&self.buffer);
        let piece_hash: [u8; 20] = hasher.finalize().into();
        if piece_hash == self.expected_hash {
            return Ok(Some(std::mem::take(&mut self.buffer)));
        }

        Err(format!(
            "Hash mismatch for piece {}: expected {:x?}, got {:x?}",
            self.index, self.expected_hash, piece_hash
        ))
    }

    pub fn reset(&mut self) {
        self.completed_blocks = 0;
        self.data.iter_mut().for_each(|(_, block)| {
            block.inflight = false;
            block.complete = false;
        });
    }

    pub fn add_data(&mut self, begin: u32, block: &[u8]) -> bool {
        let Some(slot) = self.data.get_mut(&begin) else {
            return false;
        };
        if slot.length != block.len() as u32 {
            return false;
        }

        if !slot.complete {
            slot.complete = true;
            self.completed_blocks += 1;
        }
        slot.inflight = false;

        if self.buffer.is_empty() {
            self.buffer = vec![0; self.length as usize];
        }
        self.buffer[begin as usize..begin as usize + block.len()].copy_from_slice(block);
        true
    }
}

pub struct BlockProgress {
    pub length: u32,
    pub inflight: bool,
    pub complete: bool,
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
