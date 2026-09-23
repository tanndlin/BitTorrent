# BitTorrent

A BitTorrent client written in Rust, run from the CLI.

## Running

```sh
cargo run --release -- --torrent path/to/file.torrent
```

The downloaded file is written to the current working directory under the name
from the torrent's metadata.

While downloading, pieces are written into `<name>.tmp` and recorded in
`<name>.journal`. Re-running the same torrent from the same directory resumes
from the journal. Once every piece is written, `<name>.tmp` is renamed to
`<name>` and the journal is deleted.

## Docker

`docker-compose.yml` defines the client alongside an `opentracker` instance and
five qBittorrent peers for testing. Put `.torrent` files in `docker/torrents`,
then seed the peers' config and torrents with:

```sh
./setup_clients.sh 5
```

Start the tracker and peers, then run the client against one of the torrents:

```sh
docker compose up -d opentracker qbittorrent-1 qbittorrent-2 qbittorrent-3 qbittorrent-4 qbittorrent-5
docker compose run --rm --build bittorrent-client --torrent /torrents/<file>.torrent
```

Downloads land in `docker/client/downloads`.

## Profiling

Build with the `profiling` profile (release optimizations plus debug symbols),
then record with [samply](https://github.com/mstange/samply)
(`cargo install samply`). On Windows, samply must run from an Administrator
terminal.

```sh
cargo build --profile profiling
samply record -- target/profiling/bittorrent --torrent path/to/file.torrent
```

Stop the client with Ctrl+C when you have enough samples; samply then opens the
profile in the Firefox Profiler.
