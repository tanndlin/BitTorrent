# generate_compose.py
import sys

import yaml

n = int(sys.argv[1]) if len(sys.argv) > 1 else 2

services = {
    "opentracker": {
        "image": "wiltonsr/opentracker:open",
        "container_name": "opentracker",
        "ports": ["6969:6969", "6969:6969/udp"],
        "restart": "unless-stopped",
    },
    "bittorrent-client": {
        "build": ".",
        "container_name": "bittorrent-client",
        "depends_on": ["opentracker"],
        "environment": ["TORRENT_DIR=/torrents"],
        "volumes": [
            "./docker/torrents:/torrents",
        ],
        # RAM-backed so test downloads don't hit the disk; wiped when the container stops
        "tmpfs": ["/downloads:size=4g"],
        "develop": {
            "watch": [
                {"action": "rebuild", "path": "./src"},
                {"action": "rebuild", "path": "./Cargo.toml"},
                {"action": "rebuild", "path": "./Cargo.lock"},
            ]
        },
    },
}

for i in range(1, n + 1):
    webui_port = 8079 + i
    services[f"qbittorrent-{i}"] = {
        "image": "linuxserver/qbittorrent:latest",
        "container_name": f"qbittorrent-{i}",
        "environment": [
            "PUID=1000",
            "PGID=1000",
            f"WEBUI_PORT={webui_port}",
        ],
        "ports": [
            f"{webui_port}:{webui_port}",
            f"{6880 + i}:{6880 + i}",
            f"{6880 + i}:{6880 + i}/udp",
        ],
        "volumes": [
            f"./docker/qbittorrent{i}/config:/config",
            f"./docker/qbittorrent{i}/torrents:/torrents",
            f"./docker/qbittorrent-base/downloads:/downloads",
        ],
    }

with open("docker-compose.yml", "w") as f:
    yaml.dump({"services": services}, f, default_flow_style=False)

print(f"Generated docker-compose.yml with {n} qbittorrent instances")
print("WebUI ports:", [8080 + i for i in range(1, n + 1)])
