/* eslint-disable @typescript-eslint/no-empty-function */
import React, { FC } from 'react';
import { Torrent } from '../types';

interface ITorrentContext {
    torrents: Torrent[];
    setTorrents: React.Dispatch<React.SetStateAction<Torrent[]>>;
}

const TorrentContext = React.createContext<ITorrentContext>(
    {} as ITorrentContext
);

type Props = { children: React.ReactNode | React.ReactNode[] };

const TorrentProvider: FC<Props> = ({ children }) => {
    const [torrents, setTorrents] = React.useState<Torrent[]>(
        JSON.parse(localStorage.getItem('torrents') ?? JSON.stringify([]))
    );

    console.log(torrents);
    console.log(setTorrents);

    React.useEffect(() => {
        localStorage.setItem('torrents', JSON.stringify(torrents));
    }, [torrents]);

    const temp: ITorrentContext = {
        torrents,
        setTorrents,
    };

    return (
        <TorrentContext.Provider value={temp}>
            {children}
        </TorrentContext.Provider>
    );
};

export { TorrentContext, TorrentProvider };
