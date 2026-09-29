use std::env::args;

use bittorrent::bencoding::decode;

fn main() {
    let args = args().collect::<Vec<_>>();
    let path = args.get(1).unwrap();
    dbg!(&path);

    let content = std::fs::read(path).expect("Failed to read file");
    let torrent = decode::parse_metainfo(&content);
    dbg!(torrent);
}
