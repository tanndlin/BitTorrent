use serde::{Deserialize, Serialize};
use std::collections::HashMap;

static DICTIONARY_START: u8 = b'd';
static DICTIONARY_END: u8 = b'e';
static INTEGER_START: u8 = b'i';
static INTEGER_END: u8 = b'e';
static COLON: u8 = b':';

#[derive(Serialize, Deserialize)]
pub struct Torrent {
    pub name: String,
    pub tracker: String,
    pub hashes: Vec<[u8; 20]>,
}

pub fn parse_metainfo(content: &Vec<u8>) -> HashMap<String, Value> {
    let mut index: usize = 0;
    parse_dictionary(content, &mut index)
}

pub enum Value {
    Number(usize),
    Str(String),
    Dict(HashMap<String, Value>),
    Hashes(Vec<[u8; 20]>),
}

fn parse_dictionary(content: &Vec<u8>, index: &mut usize) -> HashMap<String, Value> {
    assert!(content[*index] == DICTIONARY_START);

    *index += 1; // move past 'd'
    let mut map = HashMap::new();

    while content[*index] != DICTIONARY_END {
        let key = get_string(content, index);
        if key == "pieces" {
            map.insert(key, Value::Hashes(parse_hashes(content, index)));
            continue;
        }

        let value = if content[*index] == INTEGER_START {
            *index += 1;
            let ret = Value::Number(get_next_number(content, index));
            *index += 1;
            ret
        } else if content[*index] == DICTIONARY_START {
            Value::Dict(parse_dictionary(content, index))
        } else {
            Value::Str(get_string(content, index))
        };

        map.insert(key, value);
    }

    assert!(content[*index] == DICTIONARY_END);
    *index += 1; // move past 'e'

    map
}

fn parse_hashes(content: &Vec<u8>, index: &mut usize) -> Vec<[u8; 20]> {
    let mut size = get_next_number(content, index);
    *index += 1;

    let mut hashes = Vec::new();
    while size > 0 {
        let hash_slice = &content[*index..*index + 20];
        let mut hash = [0u8; 20];
        hash.copy_from_slice(hash_slice);
        hashes.push(hash);
        *index += 20;
        size -= 20;
    }

    hashes
}

fn get_string(content: &Vec<u8>, index: &mut usize) -> String {
    let size = get_next_number(content, index);
    *index += 1;
    let ret = String::from_utf8(content[*index..size + *index].to_vec()).unwrap();
    *index += size;

    ret
}

fn get_next_number(content: &Vec<u8>, index: &mut usize) -> usize {
    let mut size: usize = 0;
    loop {
        let char = content[*index] as char;
        if !char.is_ascii_digit() {
            break;
        }
        size *= 10;
        size += char.to_digit(10).unwrap() as usize;
        *index += 1;
    }

    size
}
