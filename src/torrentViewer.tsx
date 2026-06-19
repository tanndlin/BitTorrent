import { invoke } from '@tauri-apps/api/core';
import prettyBytes from 'pretty-bytes';
import { useEffect, useRef, useState } from 'react';
import { Torrent } from './types';
import { getTrackerURL } from './util';

type DownloadState = 'idle' | 'downloading' | 'done' | 'error';

type Props = {
    torrent: Torrent;
};

const TorrentViewer = ({ torrent }: Props) => {
    const [downloadState, setDownloadState] = useState<DownloadState>('idle');
    const [progress, setProgress] = useState<{ completed: number; total: number } | null>(null);
    const [trackerStatuses, setTrackerStatuses] = useState<boolean[] | null>(null);
    const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

    const totalSize = torrent.info.length
        ? torrent.info.length
        : (torrent.info.files?.reduce((acc, f) => acc + f.length, 0) ?? 0);

    const pct =
        progress && progress.total > 0
            ? Math.floor((progress.completed / progress.total) * 100)
            : 0;

    const pollProgress = () => {
        invoke<[number, number]>('download_progress', { infoHash: torrent.info_hash })
            .then(([completed, total]) => {
                setProgress({ completed, total });
                if (completed >= total && total > 0) {
                    setDownloadState('done');
                    if (pollRef.current) clearInterval(pollRef.current);
                }
            })
            .catch(() => {});
    };

    useEffect(() => {
        if (downloadState === 'downloading') {
            pollRef.current = setInterval(pollProgress, 1000);
        } else {
            if (pollRef.current) clearInterval(pollRef.current);
        }
        return () => {
            if (pollRef.current) clearInterval(pollRef.current);
        };
    }, [downloadState]);

    const handleDownload = async () => {
        setDownloadState('downloading');
        setProgress(null);
        try {
            await invoke('download_torrent', { torrent });
        } catch {
            setDownloadState('error');
        }
    };

    const checkTrackers = () => {
        setTrackerStatuses(null);
        const results = torrent.trackers
            .map(getTrackerURL)
            .map((url) => invoke<boolean>('check_tracker', { url }));
        Promise.all(results).then((statuses) =>
            setTrackerStatuses(statuses as boolean[])
        );
    };

    const statusCell = () => {
        switch (downloadState) {
            case 'downloading':
                return (
                    <div className="flex flex-col gap-1 min-w-32">
                        <div className="flex justify-between text-xs text-gray-400">
                            <span>Downloading</span>
                            <span>{pct}%</span>
                        </div>
                        <div className="h-1.5 bg-gray-800 rounded-full overflow-hidden">
                            <div
                                className="h-full bg-blue-500 rounded-full transition-all duration-500"
                                style={{ width: `${pct}%` }}
                            />
                        </div>
                        {progress && (
                            <span className="text-xs text-gray-500">
                                {progress.completed} / {progress.total} pieces
                            </span>
                        )}
                    </div>
                );
            case 'done':
                return (
                    <span className="px-2 py-0.5 rounded text-xs bg-green-900 text-green-300">
                        Done
                    </span>
                );
            case 'error':
                return (
                    <span className="px-2 py-0.5 rounded text-xs bg-red-900 text-red-300">
                        Error
                    </span>
                );
            default:
                return (
                    <span className="px-2 py-0.5 rounded text-xs bg-gray-800 text-gray-400">
                        Idle
                    </span>
                );
        }
    };

    return (
        <tr className="border-b border-gray-800 hover:bg-gray-900 transition-colors align-top">
            <td className="py-3 pr-4 font-medium max-w-xs truncate">
                {torrent.info.name}
            </td>
            <td className="py-3 pr-4 text-gray-400 whitespace-nowrap">
                {totalSize > 0 ? prettyBytes(totalSize) : 'N/A'}
            </td>
            <td className="py-3 pr-4">
                <select className="bg-gray-800 text-gray-300 text-xs rounded px-2 py-1 border border-gray-700 max-w-xs">
                    {torrent.trackers.map((tracker, i) => (
                        <option key={i} value={getTrackerURL(tracker)}>
                            {getTrackerURL(tracker)}
                        </option>
                    ))}
                </select>
                {trackerStatuses && (
                    <div className="flex gap-1 mt-1.5">
                        {trackerStatuses.map((ok, i) => (
                            <span
                                key={i}
                                title={ok ? 'Online' : 'Offline'}
                                className={`inline-block w-2 h-2 rounded-full ${ok ? 'bg-green-500' : 'bg-red-500'}`}
                            />
                        ))}
                    </div>
                )}
            </td>
            <td className="py-3 pr-4">{statusCell()}</td>
            <td className="py-3">
                <div className="flex gap-2">
                    <button
                        onClick={handleDownload}
                        disabled={downloadState === 'downloading' || downloadState === 'done'}
                        className="px-3 py-1 bg-blue-600 hover:bg-blue-500 disabled:bg-gray-700 disabled:text-gray-500 disabled:cursor-not-allowed text-white rounded text-xs font-medium transition-colors cursor-pointer"
                    >
                        Download
                    </button>
                    <button
                        onClick={checkTrackers}
                        className="px-3 py-1 bg-gray-700 hover:bg-gray-600 text-gray-300 rounded text-xs font-medium transition-colors cursor-pointer"
                    >
                        Check Trackers
                    </button>
                </div>
            </td>
        </tr>
    );
};

export default TorrentViewer;
