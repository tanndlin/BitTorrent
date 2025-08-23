// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod parse;

fn main() {
    bittorrent_lib::run();

    // let content = std::fs::read("C:/Users/Tanner/Documents/torrents/Inglourious Basterds 2009 Inglorious Bastards DVDRip x264.torrent").expect("Failed to read file");
    // let parsed = parse::parse_metainfo(&content);
    // parse::print_map(&parsed);
}
