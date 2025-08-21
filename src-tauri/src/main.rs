// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

mod parse;

fn print_map(map: &HashMap<String, parse::Value>) {
    for (key, value) in map {
        print!("{}: ", key);
        match value {
            parse::Value::Str(s) => println!("{s}"),
            parse::Value::Number(n) => println!("{n}"),
            parse::Value::Dict(d) => print_map(d),
            parse::Value::Hashes(h) => print_hashes(h),
        };
    }
}

fn print_hashes(hashes: &Vec<[u8; 20]>) {
    for hash in hashes {
        print!("{:?} ", hash);
    }
}

fn main() {
    // bittorrent_lib::run()
    let mut file = File::open("../sample.torrent").unwrap();

    // Create a buffer to hold the bytes
    let mut buffer = Vec::new();

    // Read the file contents into the buffer
    file.read_to_end(&mut buffer).unwrap();
    let dict = crate::parse::parse_metainfo(&buffer);
    print_map(&dict);
}
