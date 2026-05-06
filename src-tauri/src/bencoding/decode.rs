use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;

use crate::bencoding::torrent::{self, File, Info, Torrent, Tracker};

pub fn parse_metainfo(content: &Vec<u8>) -> Torrent {
    let parsed = decode_dictionary(content, &mut 0).unwrap();
    let dict = match parsed {
        Value::Dict(map) => map,
        _ => panic!("metainfo is not a dictionary"),
    };
    // print_map(&dict);

    let mut trackers = if let Some(Value::List(announce_list)) = dict.get("announce-list") {
        let mut trackers = Vec::new();
        for tier in announce_list {
            if let Value::List(tier_list) = tier {
                for url in tier_list {
                    if let Value::Str(s) = url {
                        trackers.push(match s.starts_with("http") {
                            true => Tracker::Http(s.clone()),
                            false => Tracker::Udp(s.clone()),
                        });
                    }
                }
            }
        }
        trackers
    } else if let Some(Value::Str(announce)) = dict.get("announce") {
        vec![match announce.starts_with("http") {
            true => Tracker::Http(announce.clone()),
            false => Tracker::Udp(announce.clone()),
        }]
    } else {
        vec![]
    };

    // Look for "nodes"
    if let Some(Value::List(nodes)) = dict.get("nodes") {
        for node in nodes {
            if let Value::List(node_info) = node {
                if node_info.len() == 2 {
                    if let (Value::Str(host), Value::Number(port)) = (&node_info[0], &node_info[1])
                    {
                        trackers.push(Tracker::Dht(format!("{}:{}", host, port)));
                    }
                }
            }
        }
    }

    let info_hash = get_info_hash(content, 0).unwrap();
    // Print as a hex string
    println!(
        "Info hash: {:02x?}",
        info_hash.map(|b| format!("{:02x}", b)).join("")
    );

    Torrent {
        trackers,
        info_hash,
        info: match &dict["info"] {
            Value::Dict(info_map) => {
                let name = match &info_map["name"] {
                    Value::Str(s) => s.clone(),
                    _ => panic!("info.name is not a string"),
                };
                let piece_length = match &info_map["piece length"] {
                    Value::Number(n) => *n as u64,
                    _ => panic!("info.piece length is not a number"),
                };
                let pieces = match &info_map["pieces"] {
                    Value::Hashes(h) => h.clone(),
                    _ => panic!("info.pieces is not a list of hashes"),
                };
                let length = match info_map.get("length") {
                    Some(Value::Number(n)) => Some(*n),
                    Some(_) => panic!("info.length is not a number"),
                    None => None,
                };
                let files = match info_map.get("files") {
                    Some(Value::List(l)) => {
                        let mut file_list = Vec::new();
                        for file_value in l {
                            match file_value {
                                Value::Dict(file_map) => {
                                    let length = match &file_map["length"] {
                                        Value::Number(n) => *n,
                                        _ => panic!("file.length is not a number"),
                                    };
                                    let path = match &file_map["path"] {
                                        Value::List(p) => p
                                            .iter()
                                            .map(|v| match v {
                                                Value::Str(s) => s.clone(),
                                                _ => panic!("file.path element is not a string"),
                                            })
                                            .collect(),
                                        _ => panic!("file.path is not a list"),
                                    };
                                    file_list.push(File { length, path });
                                }
                                _ => panic!("file entry is not a dictionary"),
                            }
                        }
                        Some(file_list)
                    }
                    Some(_) => panic!("info.files is not a list"),
                    None => None,
                };

                Info {
                    name,
                    piece_length,
                    pieces,
                    length,
                    files,
                }
            }
            _ => panic!("info is not a dictionary"),
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Value {
    Number(i64),
    Str(String),
    Bytes(Vec<u8>),
    Dict(HashMap<String, Value>),
    List(Vec<Value>),
    Hashes(Vec<[u8; 20]>),
    Hash([u8; 20]),
    Peers(Vec<[u8; 6]>),
}

pub fn decode_dictionary(content: &[u8], index: &mut usize) -> Result<Value, String> {
    if content[*index] != torrent::DICTIONARY_START {
        return Err(format!(
            "Expected dictionary start 'd' at index {}, found '{}'",
            index, content[*index] as char
        ));
    }

    *index += 1; // move past 'd'
    let mut map = HashMap::new();

    while content[*index] != torrent::DICTIONARY_END {
        let key = get_string(content, index)?;
        if key == "pieces" {
            map.insert(key, Value::Hashes(parse_hashes(content, index)?));
            continue;
        }

        if key == "peers" {
            map.insert(key, Value::Peers(parse_peers(content, index)?));
            continue;
        }

        let value = parse_next(content, index)?;
        map.insert(key, value);
    }

    if content[*index] != torrent::DICTIONARY_END {
        return Err(format!(
            "Expected dictionary end 'e' at index {}, found '{}'",
            index, content[*index] as char
        ));
    }
    *index += 1; // move past 'e'

    Ok(Value::Dict(map))
}

fn parse_next(content: &[u8], index: &mut usize) -> Result<Value, String> {
    if content[*index] == torrent::INTEGER_START {
        parse_number(content, index)
    } else if content[*index] == torrent::DICTIONARY_START {
        decode_dictionary(content, index)
    } else if content[*index] == torrent::LIST_START {
        parse_list(content, index)
    } else {
        let bytes = get_bytes(content, index)?;
        // Try UTF-8, fall back to raw bytes
        Ok(match String::from_utf8(bytes.clone()) {
            Ok(s) => Value::Str(s),
            Err(_) => Value::Bytes(bytes),
        })
    }
}

fn parse_number(content: &[u8], index: &mut usize) -> Result<Value, String> {
    *index += 1;
    let number = get_next_number(content, index);
    if content[*index] != torrent::INTEGER_END {
        return Err(format!(
            "Expected integer end 'e' at index {}, found '{}'",
            index, content[*index] as char
        ));
    }
    *index += 1; // move past 'e'
    Ok(Value::Number(number))
}

fn parse_list(content: &[u8], index: &mut usize) -> Result<Value, String> {
    *index += 1; // move past 'l'
    let mut list = Vec::new();
    while content[*index] != torrent::LIST_END {
        list.push(parse_next(content, index)?);
    }

    if content[*index] != torrent::LIST_END {
        return Err(format!(
            "Expected list end 'e' at index {}, found '{}'",
            index, content[*index] as char
        ));
    }

    *index += 1; // move past 'e'
    Ok(Value::List(list))
}

// Fix the parse_hashes function
fn parse_hashes(content: &[u8], index: &mut usize) -> Result<Vec<[u8; 20]>, String> {
    let size = get_next_number(content, index);

    // Check for colon separator
    if content[*index] != torrent::COLON {
        return Err(format!(
            "Expected colon ':' after pieces length at index {}, found '{}'",
            index, content[*index] as char
        ));
    }

    *index += 1; // move past ':'

    let mut hashes = Vec::new();
    let mut remaining = size;

    while remaining >= 20 {
        let hash_slice = &content[*index..*index + 20];
        let mut hash = [0u8; 20];
        hash.copy_from_slice(hash_slice);
        hashes.push(hash);
        *index += 20;
        remaining -= 20;
    }

    Ok(hashes)
}

fn parse_peers(content: &[u8], index: &mut usize) -> Result<Vec<[u8; 6]>, String> {
    let size = get_next_number(content, index) as usize;
    if content[*index] != torrent::COLON {
        return Err(format!(
            "Expected colon ':' after peers length at index {}, found '{}'",
            index, content[*index] as char
        ));
    }

    *index += 1; // move past ':'
    let mut peers = Vec::new();
    let end = *index + size;
    while *index + 6 <= end {
        let peer_slice = &content[*index..*index + 6];
        let mut peer = [0u8; 6];
        peer.copy_from_slice(peer_slice);
        peers.push(peer);
        *index += 6;
    }

    Ok(peers)
}

fn get_bytes(content: &[u8], index: &mut usize) -> Result<Vec<u8>, String> {
    let size = get_next_number(content, index) as usize;
    if content[*index] != torrent::COLON {
        return Err(format!(
            "Expected colon ':' after byte string length at index {}, found '{}'",
            index, content[*index] as char
        ));
    }

    *index += 1;
    let bytes = content[*index..*index + size].to_vec();
    *index += size;
    Ok(bytes)
}

fn get_string(content: &[u8], index: &mut usize) -> Result<String, String> {
    let bytes = get_bytes(content, index)?;
    String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 string at index {}: {}", index, e))
}

fn get_next_number(content: &[u8], index: &mut usize) -> i64 {
    let mut n: i64 = 0;
    let mut negative: i64 = 1;
    loop {
        let char = content[*index] as char;
        if n == 0 && char == '-' {
            negative = -1;
            *index += 1;
            continue;
        }

        if !char.is_ascii_digit() {
            break;
        }
        n *= 10;
        n += char.to_digit(10).unwrap() as i64;
        *index += 1;
    }

    n * negative
}

#[allow(dead_code)]
pub fn print_map(map: &HashMap<String, Value>) {
    for (key, value) in map {
        print!("{}: ", key);
        match value {
            Value::Str(s) => println!("{s}"),
            Value::Bytes(b) => println!("{:?}", b),
            Value::Number(n) => println!("{n}"),
            Value::Dict(d) => print_map(d),
            Value::Hashes(h) => print_hashes(h),
            Value::List(l) => print_list(l),
            Value::Hash(h) => print!("{:?} ", h),
            Value::Peers(p) => print!("{:?} ", p),
        };
    }
}

fn print_list(list: &Vec<Value>) {
    for value in list {
        match value {
            Value::Str(s) => print!("{s} "),
            Value::Bytes(b) => print!("{:?} ", b),
            Value::Number(n) => print!("{n} "),
            Value::Dict(d) => print_map(d),
            Value::Hashes(h) => print_hashes(h),
            Value::List(l) => print_list(l),
            Value::Hash(h) => print!("{:?} ", h),
            Value::Peers(p) => print!("{:?} ", p),
        }
    }
    println!();
}

fn print_hashes(hashes: &Vec<[u8; 20]>) {
    for hash in hashes {
        print!("{:?} ", hash);
    }
}

pub fn get_info_hash(content: &Vec<u8>, start: usize) -> Result<[u8; 20], String> {
    let mut index = start;

    // Find the "info" dictionary
    while index < content.len() {
        if content[index] == torrent::DICTIONARY_START {
            index += 1;
            let key = get_string(content, &mut index)?;
            if key == "info" {
                // We found the "info" key, now hash the corresponding dictionary
                let start = index - key.len() - 1; // include the length prefix and colon
                let mut hasher = Sha1::new();
                decode_dictionary(content, &mut index)?; // parse to move the index forward
                let end = index; // end of the "info" dictionary

                hasher.update(&content[start..end]);
                return Ok(hasher.finalize().into());
            } else {
                // Skip this dictionary entry
                return get_info_hash(content, index);
            }
        } else {
            let next = parse_next(content, &mut index)?;
            dbg!(&next);
            if let Value::Str(value) = next {
                if value == "info" {
                    let start = index;
                    let mut hasher = Sha1::new();
                    decode_dictionary(content, &mut index); // parse to move the index forward
                    let end = index; // end of the "info" dictionary
                    hasher.update(&content[start..end]);
                    return Ok(hasher.finalize().into());
                }
            }
        }
    }

    panic!("'info' dictionary not found");
}
