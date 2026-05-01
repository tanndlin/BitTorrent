use std::{collections::HashMap, hash::Hash};

use serde::{Deserialize, Serialize};

pub static DICTIONARY_START: u8 = b'd';
pub static DICTIONARY_END: u8 = b'e';
pub static INTEGER_START: u8 = b'i';
pub static INTEGER_END: u8 = b'e';
pub static LIST_START: u8 = b'l';
pub static LIST_END: u8 = b'e';
pub static COLON: u8 = b':';

#[derive(Serialize, Deserialize)]
pub struct Torrent {
    pub trackers: Vec<String>,
    pub info: Info,
    pub info_hash: [u8; 20],
}

#[derive(Serialize, Deserialize)]
pub struct Info {
    pub name: String,
    pub piece_length: u64,
    pub pieces: Vec<[u8; 20]>,
    pub length: Option<i64>,
    pub files: Option<Vec<File>>,
}

#[derive(Serialize, Deserialize)]
pub struct File {
    pub length: i64,
    pub path: Vec<String>,
}

impl Torrent {
    pub fn total_length(&self) -> u64 {
        if let Some(length) = self.info.length {
            length as u64
        } else {
            self.info
                .files
                .as_ref()
                .unwrap()
                .iter()
                .map(|f| f.length as u64)
                .sum()
        }
    }

    pub fn get_piece_length(&self, piece_index: usize) -> u32 {
        let piece_length = self.info.piece_length;
        let total_length = self.total_length();
        let last_piece_length = total_length % piece_length;

        (if piece_index == self.info.pieces.len() - 1 && last_piece_length != 0 {
            last_piece_length
        } else {
            piece_length
        } as u32)
    }
}
