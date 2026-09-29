use bittorrent::{download::download_torrent_from_path, util::CancellationToken};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the .torrent file to download
    #[arg(short, long)]
    torrent: String,
    #[arg(long)]
    no_seed: bool,
}

fn main() {
    let args = Args::parse();
    let path = PathBuf::from(&args.torrent);
    println!("Using .torrent file: {}", path.display());

    let cancel = CancellationToken::default();
    let handler_cancel = cancel.clone();
    ctrlc::set_handler(move || handler_cancel.cancel()).expect("Failed to set Ctrl+C handler");

    download_torrent_from_path(path, args.no_seed, &cancel);
}
