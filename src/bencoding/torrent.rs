use std::fmt;

use serde::{Deserialize, Serialize};

use crate::bencoding::decode;

pub static DICTIONARY_START: u8 = b'd';
pub static DICTIONARY_END: u8 = b'e';
pub static INTEGER_START: u8 = b'i';
pub static INTEGER_END: u8 = b'e';
pub static LIST_START: u8 = b'l';
pub static LIST_END: u8 = b'e';
pub static COLON: u8 = b':';

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Torrent {
    pub trackers: Vec<Tracker>,
    pub info: Info,
    pub info_hash: [u8; 20],
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Info {
    pub name: String,
    pub piece_length: u64,
    pub pieces: Vec<[u8; 20]>,
    pub length: Option<i64>,
    pub files: Option<Vec<File>>,
}

impl fmt::Debug for Info {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Info")
            .field("name", &self.name)
            .field("piece_length", &self.piece_length)
            .field("pieces", &format_args!("[{} hashes]", self.pieces.len()))
            .field("length", &self.length)
            .field("files", &self.files)
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct File {
    pub length: i64,
    pub path: Vec<String>,
}

impl Torrent {
    pub fn read(path: &str) -> Result<Self, String> {
        let content =
            std::fs::read(path).map_err(|e| format!("Unable to read torrent file: {e}"))?;
        Ok(decode::parse_metainfo(&content))
    }

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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tracker {
    Http(String),
    Udp(String),
    Dht(String),
}

impl From<Tracker> for String {
    fn from(tracker: Tracker) -> Self {
        match tracker {
            Tracker::Http(url) => url,
            Tracker::Udp(url) => url,
            Tracker::Dht(url) => url,
        }
    }
}
