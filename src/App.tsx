import './App.scss';
import { TorrentProvider } from './contexts/TorrentContext';
import TorrentsContainer from './TorrentsContainer';

function App() {
    return (
        <TorrentProvider>
            <main>
                <TorrentsContainer />
            </main>
        </TorrentProvider>
    );
}

export default App;
