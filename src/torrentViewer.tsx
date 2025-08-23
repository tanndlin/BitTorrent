import { Torrent } from './types';

type Props = {
    torrent: Torrent;
};

const torrentViewer = (props: Props) => {
    const { torrent } = props;
    return (
        <tr>
            <td>{torrent.info.name}</td>
            <td>{torrent.trackers[0]}</td>
            <td>
                {torrent.info.length
                    ? `${torrent.info.length} bytes`
                    : torrent.info.files
                    ? `${torrent.info.files.reduce(
                          (acc, file) => acc + (file.length || 0),
                          0
                      )} bytes`
                    : 'N/A'}
            </td>
        </tr>
    );
};

export default torrentViewer;
