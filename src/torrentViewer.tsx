import { Torrent } from './types';

type Props = {
    torrent: Torrent;
};

const torrentViewer = (props: Props) => {
    const { torrent } = props;
    return (
        <tr>
            <td>{torrent.name}</td>
            <td>{torrent.tracker}</td>
        </tr>
    );
};

export default torrentViewer;
