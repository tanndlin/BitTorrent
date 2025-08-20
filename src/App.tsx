import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import './App.css';
import TorrentViewer from './torrentViewer';

function App() {
    const [greetMsg, setGreetMsg] = useState('');
    const [name, setName] = useState('');

    async function greet() {
        // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
        setGreetMsg(await invoke('greet', { name }));
    }

    const [torrents, setTorrents] = useState([]);

    return (
        <main>
            <button>Add</button>
            <table>
                <thead>
                    <tr>
                        <th>Name</th>
                        <th>Progress</th>
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
