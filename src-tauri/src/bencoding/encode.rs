use std::collections::HashMap;

use crate::bencoding::{
    decode::Value,
    util::{
        Torrent, COLON, DICTIONARY_END, DICTIONARY_START, INTEGER_END, INTEGER_START, LIST_END,
        LIST_START,
    },
};

#[allow(dead_code)]
fn encode_torrent(torrent: &Torrent) -> Vec<u8> {
    let mut ret = HashMap::<String, Value>::new();
    let mut trackers = Vec::<Value>::new();
    for tracker in &torrent.trackers {
        trackers.push(Value::Str(tracker.clone()));
    }

    ret.insert("annouce-list".to_string(), Value::List(trackers));

    let mut info = HashMap::<String, Value>::new();
    info.insert("name".to_string(), Value::Str(torrent.info.name.clone()));
    info.insert(
        "piece length".to_string(),
        Value::Number(torrent.info.piece_length),
    );
    info.insert(
        "pieces".to_string(),
        Value::Hashes(torrent.info.pieces.clone()),
    );

    // Either length or files will be present
    if let Some(length) = torrent.info.length {
        info.insert("length".to_string(), Value::Number(length));
    } else {
        let mut files = Vec::<Value>::new();
        for file in torrent
            .info
            .files
            .as_ref()
            .expect("Neither length nor files was present in metainfo")
        {
            let mut file_dict = HashMap::<String, Value>::new();
            // let mut path = Vec::<Value>::new();
            // for dir in file.path {
            //     path.push(Value::Str(dir));
            // }
            let path = file
                .path
                .iter()
                .map(|dir| Value::Str(dir.to_string()))
                .collect();

            file_dict.insert("length".to_string(), Value::Number(file.length));
            file_dict.insert("path".to_string(), Value::List(path));
            files.push(Value::Dict(file_dict));
        }

        info.insert("files".to_string(), Value::List(files));
    }

    ret.insert("info".to_string(), Value::Dict(info));
    encode_value(Value::Dict(ret))
}

fn encode_value(value: Value) -> Vec<u8> {
    match value {
        Value::Number(n) => encode_number(n),
        Value::Str(s) => encode_string(&s),
        Value::Dict(dict) => encode_dictionary(dict),
        Value::List(l) => encode_list(l),
        Value::Hashes(h) => encode_hashes(h),
        Value::Hash(h) => h.to_vec(),
    }
}

fn encode_dictionary(dict: HashMap<String, Value>) -> Vec<u8> {
    let mut ret = Vec::<u8>::new();
    ret.push(DICTIONARY_START);

    for (key, value) in dict {
        ret.extend_from_slice(&encode_string(&key));
        ret.extend_from_slice(&encode_value(value));
    }

    ret.push(DICTIONARY_END);
    ret
}

fn encode_list(l: Vec<Value>) -> Vec<u8> {
    let mut ret = Vec::<u8>::new();
    ret.push(LIST_START);

    for value in l {
        ret.extend_from_slice(&encode_value(value));
    }

    ret.push(LIST_END);
    ret
}

fn encode_number(number: i32) -> Vec<u8> {
    let mut ret = Vec::<u8>::new();
    ret.push(INTEGER_START);
    ret.extend_from_slice(number.to_string().as_bytes());
    ret.push(INTEGER_END);

    ret
}

fn encode_string(string: &str) -> Vec<u8> {
    let mut ret = Vec::<u8>::new();

    let length = string.len();
    ret.extend_from_slice(length.to_string().as_bytes());
    ret.push(COLON);
    ret.extend_from_slice(string.as_bytes());

    ret
}

fn encode_hashes(hashes: Vec<[u8; 20]>) -> Vec<u8> {
    let mut ret = Vec::<u8>::new();

    for hash in hashes {
        ret.extend_from_slice(&hash);
    }

    ret
}
