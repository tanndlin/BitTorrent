import './App.scss';
import { TorrentProvider } from './contexts/TorrentContext';
import TorrentsContainer from './TorrentsContainer';

function App() {
    return (
        <TorrentProvider>
            <TorrentsContainer />
        </TorrentProvider>
    );
}

export default App;
