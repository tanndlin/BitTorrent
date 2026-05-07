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

use dotenvy::dotenv;

use crate::{
    bencoding::{
        decode,
        torrent::{Torrent, Tracker},
    },
    connection::{Event, HTTPResponse, Peer, ToUrl, TrackerRequest, TrackerResponse},
    dht::dht_node::DhtClient,
    peer::{
        peer_protocol::{connect_to_peer, PeerProtocolError},
        types::{PieceProgress, TorrentProgress},
    },
};

fn main() {
    // bittorrent_lib::run();
    dotenv().ok();

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
    let peers = get_peers_from_torrent(&torrent).expect("Failed to get peers from torrent");

    println!("Total peers collected: {}", peers.len());
    // let peers = peers.into_iter().take(250).collect::<Vec<_>>();
    // println!("Using {} peers for downloading", peers.len());

    if peers.is_empty() {
        println!("No peers available to connect to.");
        return;
    }

    let progress: Arc<Mutex<TorrentProgress>> = Arc::new(Mutex::new((&torrent).into()));

    // Find all files in pieces directory
    let pieces_dir = std::env::var("PIECES_DIR").expect("Env var PIECES_DIR not set");
    for entry in std::fs::read_dir(&pieces_dir).unwrap_or_else(|_| {
        create_dir_all(&pieces_dir).expect("Failed to create pieces directory");
        std::fs::read_dir(&pieces_dir).expect("Failed to read pieces directory")
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
        progress
            .lock()
            .unwrap()
            .pieces
            .iter()
            .filter(|p| matches!(p.1, PieceProgress::Completed(_)))
            .count(),
        torrent.info.pieces.len()
    );

    let torrent = Arc::new(torrent);
    let mut threads = vec![];
    for peer in peers {
        // println!("Attempting to connect to peer: {}:{}", peer.ip, peer.port);
        let progress = Arc::clone(&progress);
        let torrent = Arc::clone(&torrent);
        threads.push(thread::spawn(move || {
            progress
                .lock()
                .unwrap()
                .connected_peers
                .insert(peer.clone());
            match connect_to_peer(&peer, &torrent, progress.clone()) {
                Ok(_) => println!("Successfully connected to peer: {}:{}", peer.ip, peer.port),
                Err(err) => match err {
                    PeerProtocolError::ReceivedError(e) => {
                        println!("Receive error with peer {}:{} - {}", peer.ip, peer.port, e);
                    }
                    PeerProtocolError::Unknown(e) => {
                        println!("Unknown error with peer {}:{} - {}", peer.ip, peer.port, e);
                    }
                    _ => {}
                },
            }

            // Delete peer from list
            progress.lock().unwrap().connected_peers.remove(&peer);
        }));
    }

    // Every second, print progress until all pieces are complete
    let total_blocks: u64 = progress
        .lock()
        .unwrap()
        .pieces
        .iter()
        .map(|p| match p.1 {
            PieceProgress::InProgress(piece_progress_data) => piece_progress_data.data.len() as u64,
            // Estimate completed pieces as full length, even if we don't have all the blocks yet, since we want to show progress as we download blocks for a piece
            PieceProgress::Completed(_) => {
                (torrent.get_piece_length(*p.0 as usize) / (torrent.info.piece_length as u32))
                    as u64
            }
        })
        .sum();
    loop {
        let completed_blocks: u64 = progress
            .lock()
            .unwrap()
            .pieces
            .iter()
            .map(|p| match p.1 {
                PieceProgress::InProgress(piece_progress_data) => piece_progress_data
                    .data
                    .values()
                    .filter(|b| b.data.is_some())
                    .count()
                    as u64,
                PieceProgress::Completed(_) => {
                    (torrent.get_piece_length(*p.0 as usize) / (torrent.info.piece_length as u32))
                        as u64
                }
            })
            .sum();

        let percent = (completed_blocks as f64 / total_blocks as f64) * 100.0;
        let connected_peers = progress.lock().unwrap().connected_peers.len();
        print!(
            "\rProgress - {}/{} blocks ({:.2}%) - Connected Peers: {}",
            completed_blocks, total_blocks, percent, connected_peers
        );
        std::io::stdout().flush().unwrap();

        // Check if all pieces are complete
        if progress
            .lock()
            .unwrap()
            .pieces
            .iter()
            .all(|p| matches!(p.1, PieceProgress::Completed(_)))
        {
            break;
        }

        if connected_peers < 100 {
            let peers = get_peers_from_torrent(&torrent).expect("Failed to get peers from torrent");
            let peers = peers
                .into_iter()
                .filter(|p| !progress.lock().unwrap().connected_peers.contains(p))
                .collect::<Vec<_>>();

            println!("Added {} new peers", peers.len());
            for peer in peers {
                if !progress.lock().unwrap().connected_peers.contains(&peer) {
                    let progress = Arc::clone(&progress);
                    let torrent = Arc::clone(&torrent);
                    threads.push(thread::spawn(move || {
                        progress
                            .lock()
                            .unwrap()
                            .connected_peers
                            .insert(peer.clone());
                        match connect_to_peer(&peer, &torrent, progress.clone()) {
                            Ok(_) => println!(
                                "Successfully connected to peer: {}:{}",
                                peer.ip, peer.port
                            ),
                            Err(err) => match err {
                                PeerProtocolError::ReceivedError(e) => {
                                    println!(
                                        "Receive error with peer {}:{} - {}",
                                        peer.ip, peer.port, e
                                    );
                                }
                                PeerProtocolError::Unknown(e) => {
                                    println!(
                                        "Unknown error with peer {}:{} - {}",
                                        peer.ip, peer.port, e
                                    );
                                }
                                _ => {}
                            },
                        }

                        // Delete peer from list
                        progress.lock().unwrap().connected_peers.remove(&peer);
                    }));
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
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

fn get_peers_from_torrent(torrent: &Torrent) -> Result<Vec<Peer>, String> {
    let http_trackers = torrent
        .trackers
        .iter()
        .filter(|t| matches!(t, Tracker::Http(_)))
        .map(|t| String::from(t.clone()))
        .collect::<Vec<_>>();
    let dht_trackers: Vec<_> = torrent
        .trackers
        .iter()
        .filter_map(|t| {
            if let Tracker::Dht(addr) = t {
                Some(addr.clone())
            } else {
                None
            }
        })
        .chain([
            "router.bittorrent.com:6881".to_string(),
            "dht.transmissionbt.com:6881".to_string(),
            "router.utorrent.com:6881".to_string(),
        ])
        .collect();

    return get_peers_dht(&torrent.info_hash, dht_trackers);

    if http_trackers.is_empty() {
        return get_peers_dht(&torrent.info_hash, dht_trackers);
    }

    Ok(http_trackers
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
            println!("Peers: {}", response.peers.len());

            if response.peers.is_empty() {
                println!("No peers available from tracker");
                return vec![];
            }

            response.peers
        })
        .collect())
}

fn get_peers_dht(info_hash: &[u8; 20], trackers: Vec<String>) -> Result<Vec<Peer>, String> {
    println!("No HTTP trackers found, falling back to DHT");
    DhtClient::new(trackers).get_peers(info_hash)
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
