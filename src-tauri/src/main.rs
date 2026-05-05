// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(
    all(not(debug_assertions), feature = "desktop"),
    windows_subsystem = "windows"
)]

// use tauri::{http, utils::config::parse};

mod bencoding;
mod connection;
mod dht;
mod peer;
mod util;

use std::{
    fs::create_dir_all,
    io::Write,
    sync::{Arc, Mutex},
    thread,
};

use crate::{
    bencoding::{
        decode,
        torrent::{Torrent, Tracker},
    },
    connection::{Event, HTTPResponse, Peer, ToUrl, TrackerRequest, TrackerResponse},
    dht::dht_node::DHTNode,
    peer::{
        peer_protocol::connect_to_peer,
        types::{PieceProgress, TorrentProgress},
    },
};

fn main() {
    // bittorrent_lib::run();

    let search_dir = std::env::var("TORRENT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let exe_path = std::env::current_exe().expect("Failed to get current exe path");
            exe_path
                .parent()
                .expect("Failed to get parent directory")
                .to_path_buf()
        });
    let pattern = search_dir.join("*.torrent");
    println!("Searching for .torrent files in: {}", pattern.display());

    let path = glob::glob(pattern.to_str().unwrap())
        .expect("Failed to read glob pattern")
        .next()
        .expect("No .torrent files found")
        .expect("Failed to read path");
    let content = std::fs::read(path).expect("Failed to read file");
    let torrent = decode::parse_metainfo(&content);

    dbg!(&torrent.trackers);

    let start_time = std::time::Instant::now();
    let peers = get_peers_from_torrent(&torrent);

    println!("Total peers collected: {}", peers.len());
    dbg!(&peers);

    if peers.is_empty() {
        println!("No peers available to connect to.");
        return;
    }

    let progress: Arc<Mutex<TorrentProgress>> = Arc::new(Mutex::new((&torrent).into()));

    // Find all files in pieces directory
    for entry in std::fs::read_dir("/pieces").unwrap_or_else(|_| {
        create_dir_all("/pieces").expect("Failed to create pieces directory");
        std::fs::read_dir("/pieces").expect("Failed to read pieces directory")
    }) {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_stem() {
                if let Some(piece_index) = filename.to_str().and_then(|s| s.parse::<u32>().ok()) {
                    let data = std::fs::read(&path).expect("Failed to read piece file");
                    // Make sure the piece data is the correct length
                    let expected_length = torrent.get_piece_length(piece_index as usize);
                    if data.len() as u32 != expected_length {
                        println!(
                            "Warning: Piece file {} has incorrect length (expected {}, got {})",
                            path.display(),
                            expected_length,
                            data.len()
                        );
                        continue;
                    }
                    progress
                        .lock()
                        .unwrap()
                        .pieces
                        .insert(piece_index, PieceProgress::Completed(data));
                }
            }
        }
    }

    println!(
        "Found existing {}/{} pieces",
        progress.lock().unwrap().pieces.len(),
        torrent.info.pieces.len()
    );

    dbg!(&peers);

    let torrent = Arc::new(torrent);
    let mut threads = vec![];
    for peer in peers {
        println!("Attempting to connect to peer: {}:{}", peer.ip, peer.port);
        let progress = Arc::clone(&progress);
        let torrent = Arc::clone(&torrent);
        threads.push(thread::spawn(move || {
            match connect_to_peer(&peer, &torrent, progress) {
                Ok(_) => println!("Successfully connected to peer: {}:{}", peer.ip, peer.port),
                Err(err) => println!(
                    "Error communicating with peer {}:{} - {}",
                    peer.ip, peer.port, err
                ),
            }
        }));
    }

    // Wait for all threads to finish
    for thread in threads {
        thread.join().expect("Failed to join thread");
    }

    let end_time = std::time::Instant::now();

    println!(
        "Download complete! Time taken: {:.2?}",
        end_time.duration_since(start_time)
    );
    let donwloads_dir = std::env::var("DOWNLOADS_DIR").unwrap_or_else(|_| "/downloads".to_string());
    create_dir_all(&donwloads_dir).expect("Failed to create downloads directory");
    println!("Saving file to downloads directory: {}", donwloads_dir);

    // Build file from pieces
    let mut output_file = std::fs::File::create(format!("{}/{}", donwloads_dir, torrent.info.name))
        .expect("Failed to create output file");
    for i in 0..torrent.info.pieces.len() {
        let progress = progress.lock().unwrap();

        let piece_data = progress
            .pieces
            .get(&(i as u32))
            .expect("Missing piece data")
            .get_final_data()
            .expect("Piece failed hash check")
            .expect("Piece data is incomplete");
        output_file
            .write_all(&piece_data)
            .expect("Failed to write piece data to output file");
    }

    println!("File saved successfully!");
}

fn get_peers_from_torrent(torrent: &Torrent) -> Vec<Peer> {
    let http_trackers = torrent
        .trackers
        .iter()
        .filter(|t| matches!(t, Tracker::Http(_)))
        .map(|t| String::from(t.clone()))
        .collect::<Vec<_>>();
    if http_trackers.is_empty() {
        return get_peers_dht(
            torrent
                .trackers
                .iter()
                .map(|t| String::from(t.clone()))
                .collect(),
        );
    }

    http_trackers
        .into_iter()
        .flat_map(|tracker| {
            let response = match get_peers_http(torrent, &tracker) {
                Ok(res) => res,
                Err(err) => {
                    println!("Error getting peers from tracker {}: {}", tracker, err);
                    return vec![];
                }
            };

            println!("Tracker Response: {:?}", response);

            if let Some(err) = response.failure {
                println!("Tracker failure reason: {:?}", err);
                return vec![];
            }

            let response = response.success.expect("No success response from tracker");
            println!("Interval: {}", response.interval);
            println!("Leechers: {}", response.incomplete.unwrap_or(0));
            println!("Seeders: {}", response.complete.unwrap_or(0));
            println!("Peers: {:?}", response.peers);

            if response.peers.is_empty() {
                println!("No peers available from tracker");
                return vec![];
            }

            response.peers
        })
        .collect()
}

fn get_peers_dht(trackers: Vec<String>) -> Vec<Peer> {
    println!("No HTTP trackers found, falling back to DHT");
    DHTNode::new(trackers).get_peers()
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
    let response = reqwest::blocking::get(&url).map_err(|_| "Failed to send request")?;
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
