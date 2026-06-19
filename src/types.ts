export type Torrent = {
    trackers: Tracker[];
    info: Info;
};

type Info = {
    name: string;
    piece_length: number;
    pieces: string[];
    length?: number;
    files?: File[];
};

type File = {
    length: number;
    path: string;
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