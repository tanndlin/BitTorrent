import { invoke } from '@tauri-apps/api/core';
import React from 'react';
import ResizeableTableHeader from './common/ResizeableTableHeader';
import { TorrentContext } from './contexts/TorrentContext';
import TorrentViewer from './torrentViewer';
import { Torrent } from './types';

const TorrentsContainer = () => {
    const { torrents, setTorrents } = React.useContext(TorrentContext);

    return (
        <div>
            <input
                type="file"
                id="torrentFile"
                onChange={async (e) => {
                    const file = e.target.files?.[0];
                    if (file) {
                        invoke<Torrent>('parse_torrent', {
                            buffer: await file.arrayBuffer(),
                        }).then((data: Torrent) => {
                            console.log(data);
                            setTorrents([data]);
                        });
                    }
                }}
            />
            <table id="torrentsTable">
                <thead>
                    <tr>
                        <ResizeableTableHeader title="Name" />
                        <ResizeableTableHeader title="Trackers" />
                        <ResizeableTableHeader title="Size" />
                        <ResizeableTableHeader title="Actions" />
                        <ResizeableTableHeader title="Tracker Statuses" />
                    </tr>
                </thead>
                <tbody>
                    {torrents.map((torrent, index) => (
                        <TorrentViewer key={index} torrent={torrent} />
                    ))}
                </tbody>
            </table>
        </div>
    );
};

export default TorrentsContainer;
