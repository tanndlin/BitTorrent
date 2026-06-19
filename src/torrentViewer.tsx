import { invoke } from '@tauri-apps/api/core';
import prettyBytes from 'pretty-bytes';
import { useState } from 'react';
import { Torrent } from './types';
import { getTrackerURL } from './util';

type Props = {
    torrent: Torrent;
};

const torrentViewer = (props: Props) => {
    const { torrent } = props;
    const [trackerStatuses, setTrackerStatuses] = useState<boolean[]>([]);

    const checkTracker = () => {
        console.log('Checking trackers...');
        const results = torrent.trackers
            .map(getTrackerURL)
            .map((tracker) =>
            invoke('check_tracker', { url:tracker })
        );

        console.log(torrent.trackers.map(getTrackerURL));

        Promise.all(results).then((statuses) => {
            setTrackerStatuses(statuses as boolean[]);
            console.log(statuses);
        });
    };

        console.log(torrent.trackers);

    return (
        <tr>
            <td>{torrent.info.name}</td>
            <td>
                <select>
                    {torrent.trackers.map((tracker, index) => (
                        <option key={index} value={getTrackerURL(tracker)}>
                            {getTrackerURL(tracker)}
                        </option>
                    ))}
                </select>
            </td>
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
            <td>
                <button onClick={checkTracker}>Check Trackers</button>
            </td>
            <td>
                {trackerStatuses.length > 0
                    ? trackerStatuses.map((status, index) => (
                          <div key={index}>{status ? 'good' : 'bad'}</div>
                      ))
                    : 'No statuses yet'}
            </td>
        </tr>
    );
};

export default torrentViewer;
