export type Torrent = {
    name: string;
    tracker: string;
    hashes: Uint8Array[];
};
