set shell := ["powershell.exe", "-c"]

# Profile against the docker swarm, e.g. `just profile GH010048.MP4.torrent`; Ctrl+C to stop
profile torrent:
    docker compose run --rm --build bittorrent-profiler --torrent /torrents/{{file_name(replace(torrent, '\', '/'))}}
    samply load docker/profiles/profile.json.gz
