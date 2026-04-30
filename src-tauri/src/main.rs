// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// use tauri::{http, utils::config::parse};

mod bencoding;
mod connection;
mod peer;
mod util;

use crate::{
    bencoding::{decode, torrent::Torrent},
    connection::{Event, HTTPResponse, Peer, ToUrl, TrackerRequest, TrackerResponse},
    peer::peer_protocol::connect_to_peer,
};

fn main() {
    // bittorrent_lib::run();

    // Get abs path of main file
    let exe_path = std::env::current_exe().expect("Failed to get current exe path");
    let exe_dir = exe_path.parent().expect("Failed to get parent directory");
    let pattern = exe_dir.join("*.torrent");
    println!("Searching for .torrent files in: {}", pattern.display());

    let path = glob::glob(pattern.to_str().unwrap())
        .expect("Failed to read glob pattern")
        .next()
        .expect("No .torrent files found")
        .expect("Failed to read path");
    let content = std::fs::read(path).expect("Failed to read file");
    let parsed = decode::parse_metainfo(&content);

    let http_trackers = parsed.trackers.iter().filter(|t| t.starts_with("http"));

    let peers: Vec<Peer> = http_trackers
        .into_iter()
        .flat_map(|tracker| {
            let response = get_peers_http(&parsed, tracker).unwrap();
            println!("Tracker Response: {:?}", response);

            if let Some(err) = response.failure {
                println!("Tracker failure reason: {:?}", err);
                return vec![];
            }

            let response = response.success.expect("No success response from tracker");
            println!("Interval: {}", response.interval);
            println!("Leechers: {}", response.incomplete);
            println!("Seeders: {}", response.complete);
            println!("Peers: {:?}", response.peers);

            if response.peers.is_empty() {
                println!("No peers available from tracker");
                return vec![];
            }

            response.peers
        })
        .collect();

    println!("Total peers collected: {}", peers.len());
    dbg!(&peers);

    if peers.is_empty() {
        println!("No peers available to connect to.");
        return;
    }

    let peer = &peers[0];
    println!("First peer IP: {}, Port: {}", peer.ip, peer.port);
    connect_to_peer(peer, &parsed);
}

fn get_peers_http(torrent: &Torrent, tracker: &str) -> Result<TrackerResponse, String> {
    println!("Testing HTTP tracker: {}", tracker);

    let left = if let Some(length) = torrent.info.length {
        length as u64
    } else {
        torrent.info.files.as_ref().unwrap()[0].length as u64
    };

    // send a connect request
    let connection_request = TrackerRequest {
        info_hash: torrent.info_hash,
        peer_id: *b"-TR2940-fuckmek6wWLc",
        downloaded: 0,
        left,
        uploaded: 0,
        event: Event::Started,
        ip: None,
        key: None,
        num_want: Some(100),
        port: 6969,
        compact: 1,
        no_peer_id: false,
        tracker_id: None,
    };

    let url = format!("{}{}", tracker, connection_request.to_url_params());
    println!("Request URL: {}", url);
    let response = reqwest::blocking::get(&url).expect("Failed to send request");
    let status = response.status();
    println!("Response Status: {}", status);

    let bytes = response.bytes().expect("Failed to read bytes");
    let text = String::from_utf8_lossy(&bytes);
    println!("Response Body: {:?}", text);

    if !status.is_success() {
        return Err("Failed to get a successful response from the tracker".to_string());
    }

    let tracker_response = TrackerResponse::from_http_response(bytes.as_ref());
    dbg!(&tracker_response);

    Ok(tracker_response)
}
