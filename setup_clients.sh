N=${1:-2}
for i in $(seq 1 $N); do
    mkdir -p ./docker/qbittorrent$i/config/qBittorrent
    mkdir -p ./docker/qbittorrent$i/torrents
    cp ./docker/qbittorrent-base/qBittorrent.conf ./docker/qbittorrent$i/config/qBittorrent/qBittorrent.conf
    cp ./docker/qbittorrent-base/watched_folders.json ./docker/qbittorrent$i/config/qBittorrent/watched_folders.json
    cp ./docker/torrents/*.torrent ./docker/qbittorrent$i/torrents/
done