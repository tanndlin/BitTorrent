mod bencoding;
mod connection;
mod dht;
mod download;
mod peer;
mod util;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering::SeqCst},
        Arc, RwLock,
    },
};

#[cfg(feature = "desktop")]
use crate::bencoding::{decode, torrent::Torrent};

#[cfg(feature = "desktop")]
#[tauri::command]
async fn check_tracker(url: &str) -> Result<bool, String> {
    connection::check_tracker(url)
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn parse_torrent(buffer: Vec<u8>, state: tauri::State<AppState>) -> Torrent {
    let torrent = decode::parse_metainfo(&buffer);
    let info_hash = torrent.info_hash;
    state
        .torrents
        .write()
        .unwrap()
        .insert(info_hash, torrent.clone());
    torrent
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn download_torrent(torrent: Torrent, state: tauri::State<AppState>) -> Result<(), String> {
    let info_hash = torrent.info_hash;
    let total = torrent.info.pieces.len() as u64;
    let completed = Arc::new(AtomicU64::new(0));

    state
        .progress
        .write()
        .unwrap()
        .insert(info_hash, (Arc::clone(&completed), total));

    std::thread::spawn(move || {
        download::download_torrent(torrent, completed);
    });
    Ok(())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn download_progress(
    info_hash: [u8; 20],
    state: tauri::State<AppState>,
) -> Result<(u64, u64), String> {
    let progress = state.progress.read().unwrap();
    progress
        .get(&info_hash)
        .map(|(completed, total)| (completed.load(SeqCst), *total))
        .ok_or_else(|| "No progress tracked for this torrent".to_string())
}

struct AppState {
    pub torrents: RwLock<HashMap<[u8; 20], Torrent>>,
    pub progress: RwLock<HashMap<[u8; 20], (Arc<AtomicU64>, u64)>>,
}

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            torrents: RwLock::new(HashMap::new()),
            progress: RwLock::new(HashMap::new()),
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_tracker,
            parse_torrent,
            download_torrent,
            download_progress,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
