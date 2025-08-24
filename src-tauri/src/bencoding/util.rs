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

pub struct Download<'a> {
    pub torrent: &'a Torrent,
    pub peer_id: String,
    pub ip: Option<String>,
    pub port: i32,
    pub uploaded: i32,
    pub downloaded: i32,
    pub left: i32,
    pub event: Option<Event>,
}

pub enum Event {
    Started,
    Completed,
    Stopped,
    Empty,
}

impl Event {
    pub fn to_string(&self) -> String {
        match self {
            Event::Started => "started".to_string(),
            Event::Completed => "completed".to_string(),
            Event::Stopped => "stopped".to_string(),
            Event::Empty => "empty".to_string(),
        }
    }

    pub fn from_string(str: &str) -> Event {
        match str {
            "started" => Event::Started,
            "completed" => Event::Completed,
            "stopped" => Event::Stopped,
            "empty" => Event::Empty,
            _ => panic!("Unexpected event value {str}"),
        }
    }
}
