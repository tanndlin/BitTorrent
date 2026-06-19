import { invoke } from '@tauri-apps/api/core';
import React, { useRef } from 'react';
import { TorrentContext } from './contexts/TorrentContext';
import TorrentViewer from './torrentViewer';
import { Torrent } from './types';

const TorrentsContainer = () => {
    const { torrents, setTorrents } = React.useContext(TorrentContext);
    const fileInputRef = useRef<HTMLInputElement>(null);

    const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        const buffer = Array.from(new Uint8Array(await file.arrayBuffer()));
        const data = await invoke<Torrent>('parse_torrent', { buffer });
        setTorrents((prev) => {
            const exists = prev.some(
                (t) => t.info_hash.toString() === data.info_hash.toString()
            );
            return exists ? prev : [...prev, data];
        });
        e.target.value = '';
    };

    return (
        <div className="min-h-screen bg-gray-950 text-gray-100 p-6">
            <div className="flex items-center justify-between mb-6">
                <h1 className="text-xl font-semibold tracking-tight">BitTorrent</h1>
                <button
                    onClick={() => fileInputRef.current?.click()}
                    className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-md text-sm font-medium transition-colors cursor-pointer"
                >
                    + Open .torrent
                </button>
                <input
                    ref={fileInputRef}
                    type="file"
                    accept=".torrent"
                    className="hidden"
                    onChange={handleFile}
                />
            </div>

            {torrents.length === 0 ? (
                <div className="text-center text-gray-500 py-24 text-sm">
                    No torrents. Open a .torrent file to get started.
                </div>
            ) : (
                <table className="w-full border-collapse text-sm">
                    <thead>
                        <tr className="border-b border-gray-800 text-gray-400 text-left">
                            <th className="py-2 pr-4 font-medium">Name</th>
                            <th className="py-2 pr-4 font-medium">Size</th>
                            <th className="py-2 pr-4 font-medium">Trackers</th>
                            <th className="py-2 pr-4 font-medium">Status</th>
                            <th className="py-2 font-medium">Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {torrents.map((torrent, index) => (
                            <TorrentViewer key={index} torrent={torrent} />
                        ))}
                    </tbody>
                </table>
            )}
        </div>
    );
};

export default TorrentsContainer;
