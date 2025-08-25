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
}

#[derive(Serialize, Deserialize)]
pub struct Info {
    pub name: String,
    pub piece_length: i32,
    pub pieces: Vec<[u8; 20]>,
    pub length: Option<i32>,
    pub files: Option<Vec<File>>,
}

#[derive(Serialize, Deserialize)]
pub struct File {
    pub length: i32,
    pub path: Vec<String>,
}
