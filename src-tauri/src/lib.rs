mod bencoding;
mod connection;
mod peer;
mod util;

#[cfg(feature = "desktop")]
use crate::bencoding::decode;
#[cfg(feature = "desktop")]
use crate::bencoding::torrent::Torrent;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[cfg(feature = "desktop")]
#[tauri::command]
async fn check_tracker(url: &str) -> Result<bool, String> {
    connection::check_tracker(url)
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn parse_torrent(buffer: Vec<u8>) -> Torrent {
    // Read the file contents into the buffer
    decode::parse_metainfo(&buffer)
}

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![check_tracker, parse_torrent])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
