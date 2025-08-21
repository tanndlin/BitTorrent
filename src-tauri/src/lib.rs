mod parse;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

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

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn parse_torrent(buffer: Vec<u8>) -> parse::Torrent {
    // Read the file contents into the buffer
    let dict = crate::parse::parse_metainfo(&buffer);
    print_map(&dict);

    parse::Torrent {
        name: dict
            .get("name")
            .and_then(|v| match v {
                parse::Value::Str(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default(),
        tracker: dict
            .get("announce")
            .and_then(|v| match v {
                parse::Value::Str(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default(),
        hashes: dict
            .get("pieces")
            .and_then(|v| match v {
                parse::Value::Hashes(h) => Some(h.clone()),
                _ => None,
            })
            .unwrap_or_default(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .invoke_handler(tauri::generate_handler![parse_torrent])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
