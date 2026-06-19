export type Torrent = {
    trackers: Tracker[];
    info: Info;
    info_hash: number[];
};

type Info = {
    name: string;
    piece_length: number;
    pieces: number[][];
    length?: number;
    files?: TorrentFile[];
};

type TorrentFile = {
    length: number;
    path: string[];
};

export type Tracker = HTTPTracker | UDPTracker | DHTTracker;

export type HTTPTracker = {
    http: string;
};

export type UDPTracker = {
    udp: string;
};

export type DHTTracker = {
    dht: string;
};