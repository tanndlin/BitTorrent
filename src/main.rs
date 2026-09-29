use bittorrent::download::download_torrent_from_path;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the .torrent file to download
    #[arg(short, long)]
    torrent: String,
}

fn main() {
    let args = Args::parse();
    let path = PathBuf::from(&args.torrent);
    println!("Using .torrent file: {}", path.display());
    download_torrent_from_path(path.to_str().expect("Path is not valid UTF-8"));
}
