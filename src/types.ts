export type Torrent = {
    trackers: string[];
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
