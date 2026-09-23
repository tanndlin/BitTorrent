# BitTorrent

A BitTorrent client written in Rust, run from the CLI.

## Running

```sh
cargo run --release -- path/to/file.torrent
```

If no path is given, the first `*.torrent` file in `TORRENT_DIR` is used (falling
back to the directory containing the executable).

## Configuration

Environment variables, read from `.env` if present:

| Variable | Description |
| --- | --- |
| `TORRENT_DIR` | Directory searched for a `.torrent` file when none is passed on the command line |
| `PIECES_DIR` | Where downloaded pieces are cached (required) |
| `DOWNLOADS_DIR` | Where the assembled file is written (default `/downloads`) |

## Docker

`docker-compose.yml` brings up the client alongside an `opentracker` instance and
a few qBittorrent peers for testing:

```sh
docker compose up --build
```

Regenerate the compose file with a different number of qBittorrent peers via
`python gen_compose.py <n>`.
