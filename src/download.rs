use std::{
    collections::HashMap,
    io::Write,
    sync::{
        atomic::{AtomicU64, Ordering::SeqCst},
        Arc, RwLock,
    },
    thread,
    time::{Duration, Instant},
};

use crate::{
    bencoding::{
        decode,
        torrent::{Torrent, Tracker},
    },
    connection::{Event, HTTPResponse, Peer, ToUrl, TrackerRequest, TrackerResponse},
    dht::dht_node::DhtClient,
    peer::{
        peer_protocol::{connect_to_peer, PeerProtocolError},
        types::TorrentProgress,
    },
};

const PEER_RETRY_DELAY: Duration = Duration::from_secs(30);
const TRACKER_RETRY_DELAY: Duration = Duration::from_secs(60);

pub fn download_torrent_from_path(path: &str) {
    let content = std::fs::read(path).expect("Failed to read file");
    let torrent = decode::parse_metainfo(&content);
    download_torrent(torrent, Arc::new(AtomicU64::new(0)));
}

pub fn download_torrent(torrent: Torrent, completed_pieces: Arc<AtomicU64>) {
    dbg!(&torrent.trackers);

    let start_time = std::time::Instant::now();
    let progress: Arc<RwLock<TorrentProgress>> = Arc::new(RwLock::new((&torrent).into()));
    let total_pieces = torrent.info.pieces.len() as u64;

    let loaded_pieces = progress.read().unwrap().journal.num_written_pieces();
    println!("{loaded_pieces} pieces already downloaded");
    completed_pieces.fetch_add(loaded_pieces as u64, SeqCst);

    println!(
        "Found existing {}/{} pieces",
        loaded_pieces,
        torrent.info.pieces.len()
    );

    let torrent = Arc::new(torrent);
    let mut last_attempt: HashMap<Peer, Instant> = HashMap::new();
    let mut failed_trackers: HashMap<String, Instant> = HashMap::new();

    println!();

    // Every second, print progress until all pieces are complete
    loop {
        let completed = completed_pieces.load(SeqCst);
        let percent = (completed as f64 / total_pieces as f64) * 100.0;
        let connected_peers = progress.read().unwrap().connected_peers.len();
        print!(
            "\rProgress - {}/{} peices ({:.2}%) - Connected Peers: {}",
            completed, total_pieces, percent, connected_peers
        );
        std::io::stdout().flush().unwrap();

        // Check if all pieces are complete
        if completed >= total_pieces {
            println!();
            break;
        }

        if connected_peers < 100 {
            let peers = get_peers_from_torrent(&torrent, &mut failed_trackers)
                .expect("Failed to get peers from torrent");
            // Skip peers tried recently, otherwise a peer that refuses connections
            // (like our own announced address) gets retried every second
            let now = Instant::now();
            let peers = peers
                .into_iter()
                .filter(|p| !progress.read().unwrap().connected_peers.contains(p))
                .filter(|p| {
                    last_attempt
                        .get(p)
                        .is_none_or(|t| now.duration_since(*t) >= PEER_RETRY_DELAY)
                })
                .collect::<Vec<_>>();
            for peer in &peers {
                last_attempt.insert(peer.clone(), now);
            }

            if !peers.is_empty() {
                println!("Added {} new peers", peers.len());

                dbg!(&progress.read().unwrap().connected_peers);
                dbg!(&peers);
            }

            for peer in peers {
                // Register before spawning so a peer can't get two threads, and so
                // one thread's cleanup can't remove another thread's entry
                if progress
                    .write()
                    .unwrap()
                    .connected_peers
                    .insert(peer.clone())
                {
                    let progress = Arc::clone(&progress);
                    let torrent = Arc::clone(&torrent);
                    let completed_pieces = Arc::clone(&completed_pieces);
                    thread::spawn(move || {
                        match connect_to_peer(
                            &peer,
                            &torrent,
                            progress.clone(),
                            completed_pieces.clone(),
                        ) {
                            Ok(_) => {}
                            Err(err) => match err {
                                PeerProtocolError::ReceivedError(e) => {
                                    println!(
                                        "Receive error with peer {}:{} - {}",
                                        peer.ip, peer.port, e
                                    );
                                }
                                PeerProtocolError::Unknown(e) => {
                                    println!(
                                        "Unknown error with peer {}:{} - {}",
                                        peer.ip, peer.port, e
                                    );
                                }
                                _ => {}
                            },
                        }

                        // Delete peer from list
                        progress.write().unwrap().connected_peers.remove(&peer);
                    });
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Peer threads aren't joined: every piece is already in `progress`, and a peer
    // thread stuck on a slow or dead connection shouldn't hold up saving the file
    let end_time = std::time::Instant::now();

    println!(
        "Download complete! Time taken: {:.2?}",
        end_time.duration_since(start_time)
    );
}

fn get_peers_from_torrent(
    torrent: &Torrent,
    failed_trackers: &mut HashMap<String, Instant>,
) -> Result<Vec<Peer>, String> {
    let http_trackers = torrent
        .trackers
        .iter()
        .filter(|t| matches!(t, Tracker::Http(_)))
        .map(|t| String::from(t.clone()))
        .collect::<Vec<_>>();
    let dht_trackers: Vec<_> = torrent
        .trackers
        .iter()
        .filter_map(|t| {
            if let Tracker::Dht(addr) = t {
                Some(addr.clone())
            } else {
                None
            }
        })
        .chain([
            "router.bittorrent.com:6881".to_string(),
            "dht.transmissionbt.com:6881".to_string(),
            "router.utorrent.com:6881".to_string(),
        ])
        .collect();

    if http_trackers.is_empty() {
        return get_peers_dht(&torrent.info_hash, dht_trackers);
    }

    let now = Instant::now();
    Ok(http_trackers
        .into_iter()
        .filter(|tracker| {
            failed_trackers
                .get(tracker)
                .is_none_or(|t| now.duration_since(*t) >= TRACKER_RETRY_DELAY)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(|tracker| {
            let response = match get_peers_http(torrent, &tracker) {
                Ok(res) => {
                    failed_trackers.remove(&tracker);
                    res
                }
                Err(err) => {
                    // Only report the first failure, not every retry
                    if failed_trackers.insert(tracker.clone(), now).is_none() {
                        println!("Error getting peers from tracker {}: {}", tracker, err);
                    }
                    return vec![];
                }
            };

            // println!("Tracker Response: {:?}", response);

            if let Some(err) = response.failure {
                println!("Tracker failure reason: {:?}", err);
                return vec![];
            }

            let response = response.success.expect("No success response from tracker");
            // println!("Interval: {}", response.interval);
            // println!("Leechers: {}", response.incomplete.unwrap_or(0));
            // println!("Seeders: {}", response.complete.unwrap_or(0));
            // println!("Peers: {}", response.peers.len());

            if response.peers.is_empty() {
                println!("No peers available from tracker");
                return vec![];
            }

            response.peers
        })
        .collect())
}

fn get_peers_dht(info_hash: &[u8; 20], trackers: Vec<String>) -> Result<Vec<Peer>, String> {
    println!("No HTTP trackers found, falling back to DHT");
    DhtClient::new(trackers).get_peers(info_hash)
}

fn get_peers_http(torrent: &Torrent, tracker: &str) -> Result<TrackerResponse, String> {
    // println!("Testing HTTP tracker: {}", tracker);

    let left = if let Some(length) = torrent.info.length {
        length as u64
    } else {
        torrent.info.files.as_ref().unwrap()[0].length as u64
    };

    // send a connect request
    let connection_request = TrackerRequest {
        info_hash: torrent.info_hash,
        peer_id: *b"-TR2940-fuckmek6wWLc",
        downloaded: 0,
        left,
        uploaded: 0,
        event: Event::Started,
        ip: None,
        key: None,
        num_want: Some(100),
        port: 6969,
        compact: 1,
        no_peer_id: false,
        tracker_id: None,
    };

    let url = format!("{}{}", tracker, connection_request.to_url_params());
    // println!("Request URL: {}", url);
    let response = reqwest::blocking::get(&url).map_err(|e| e.without_url().to_string())?;
    let status = response.status();
    // println!("Response Status: {}", status);

    if !status.is_success() {
        return Err("Failed to get a successful response from the tracker".to_string());
    }

    let bytes = response.bytes().expect("Failed to read bytes");
    let tracker_response = TrackerResponse::from_http_response(bytes.as_ref());
    // dbg!(&tracker_response);

    Ok(tracker_response)
}
