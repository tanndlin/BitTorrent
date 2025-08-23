import prettyBytes from 'pretty-bytes';
import { Torrent } from './types';

type Props = {
    torrent: Torrent;
};

const torrentViewer = (props: Props) => {
    const { torrent } = props;
    return (
        <tr>
            <td>{torrent.info.name}</td>
            <td>{torrent.trackers.length}</td>
            <td>
                {torrent.info.length
                    ? `${torrent.info.length} bytes`
                    : torrent.info.files
                    ? `${prettyBytes(
                          torrent.info.files.reduce(
                              (acc, file) => acc + (file.length || 0),
                              0
                          )
                      )}`
                    : 'N/A'}
            </td>
        </tr>
    );
};

export default torrentViewer;
