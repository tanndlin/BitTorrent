mod bencoding;
mod connection;
mod dht;
mod download;
mod peer;
mod util;

use std::path::PathBuf;

use dotenvy::dotenv;

use crate::download::download_torrent_from_path;

fn main() {
    dotenv().ok();

    let path = match std::env::args().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => find_torrent_file(),
    };

    println!("Using .torrent file: {}", path.display());
    download_torrent_from_path(path.to_str().expect("Path is not valid UTF-8"));
}

/// Find the first *.torrent file in TORRENT_DIR, falling back to the directory
/// holding the executable.
fn find_torrent_file() -> PathBuf {
    let search_dir = std::env::var("TORRENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let exe_path = std::env::current_exe().expect("Failed to get current exe path");
            exe_path
                .parent()
                .expect("Failed to get parent directory")
                .to_path_buf()
        });

    let pattern = search_dir.join("*.torrent");
    println!("Searching for .torrent files in: {}", pattern.display());

    glob::glob(pattern.to_str().expect("Path is not valid UTF-8"))
        .expect("Failed to read glob pattern")
        .next()
        .expect("No .torrent files found")
        .expect("Failed to read path")
}
