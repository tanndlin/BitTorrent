import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import './App.css';
import TorrentViewer from './torrentViewer';
import { Torrent } from './types';

function App() {
    // const [greetMsg, setGreetMsg] = useState('');
    // const [name, setName] = useState('');

    // async function greet() {
    //     // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    //     setGreetMsg(await invoke('greet', { name }));
    // }

    const [torrents, setTorrents] = useState<Torrent[]>([]);

    return (
        <main>
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
            <table>
                <thead>
                    <tr>
                        <th>Name</th>
                        <th>Tracker</th>
                        <th>Size</th>
                    </tr>
                </thead>
                <tbody>
                    {torrents.map((torrent, index) => (
                        <TorrentViewer key={index} torrent={torrent} />
                    ))}
                </tbody>
            </table>
        </main>
    );
}

export default App;
