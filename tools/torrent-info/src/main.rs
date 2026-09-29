use std::env::args;

use bittorrent::bencoding::torrent::Torrent;

fn main() {
    let args = args().collect::<Vec<_>>();
    let path = args.get(1).unwrap();

    let torrent = Torrent::read(path).unwrap();
    dbg!(torrent);
}
