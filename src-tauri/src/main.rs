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
    sync::{
        atomic::{AtomicU64, Ordering::SeqCst},
        Arc, RwLock,
    },
    thread,
};

use dotenvy::dotenv;
use rayon::prelude::*;
use sha1::{Digest, Sha1};

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
    dotenv().ok();
    bittorrent_lib::run();

    // let search_dir = std::env::var("TORRENT_DIR")
    //     .map(std::path::PathBuf::from)
    //     .unwrap_or_else(|_| {
    //         let exe_path = std::env::current_exe().expect("Failed to get current exe path");
    //         exe_path
    //             .parent()
    //             .expect("Failed to get parent directory")
    //             .to_path_buf()
    //     });
    // let pattern = search_dir.join("*.torrent");
    // println!("Searching for .torrent files in: {}", pattern.display());

    // let path = glob::glob(pattern.to_str().unwrap())
    //     .expect("Failed to read glob pattern")
    //     .next()
    //     .expect("No .torrent files found")
    //     .expect("Failed to read path");
    // println!("Found .torrent file: {}", path.display());
    // download_torrent_from_path(path.to_str().unwrap());
}
