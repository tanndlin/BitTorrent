// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{net::UdpSocket, path::Path};
// use tauri::{http, utils::config::parse};
use url::Url;

mod bencoding;
mod connection;
mod peer;
mod util;

use crate::{
    bencoding::{decode, torrent::Torrent},
    connection::{
        Action, AnnounceRequest, AnnounceResponse, Event, FromByte, HTTPResponse, Peer, ToByte,
        ToUrl, TrackerRequest, TrackerResponse,
    },
    peer::connect_to_peer,
};

fn main() {
    // bittorrent_lib::run();

    // Get abs path of main file
    let exe_path = std::env::current_exe().expect("Failed to get current exe path");
    let exe_dir = exe_path.parent().expect("Failed to get parent directory");
    let pattern = exe_dir.join("*.torrent");
    println!("Searching for .torrent files in: {}", pattern.display());

    let path = glob::glob(pattern.to_str().unwrap())
        .expect("Failed to read glob pattern")
        .next()
        .expect("No .torrent files found")
        .expect("Failed to read path");
    let content = std::fs::read(path).expect("Failed to read file");
    let parsed = decode::parse_metainfo(&content);

    let http_trackers: Vec<&String> = parsed
        .trackers
        .iter()
        .filter(|t| t.starts_with("http"))
        .collect();

    let peers: Vec<Peer> = http_trackers
        .iter()
        .flat_map(|&tracker| {
            let response = get_peers_http(&parsed, tracker).unwrap();
            println!("Tracker Response: {:?}", response);

            if let Some(err) = response.failure {
                println!("Tracker failure reason: {:?}", err);
                return vec![];
            }

            let response = response.success.expect("No success response from tracker");
            println!("Interval: {}", response.interval);
            println!("Leechers: {}", response.incomplete);
            println!("Seeders: {}", response.complete);
            println!("Peers: {:?}", response.peers);

            if response.peers.is_empty() {
                println!("No peers available from tracker");
                return vec![];
            }

            response.peers
        })
        .collect();

    println!("Total peers collected: {}", peers.len());
    dbg!(&peers);

    if peers.is_empty() {
        println!("No peers available to connect to.");
        return;
    }

    let peer = &peers[0];
    println!("First peer IP: {}, Port: {}", peer.ip, peer.port);
    connect_to_peer(peer, &parsed);
}

fn get_peers_http(torrent: &Torrent, tracker: &str) -> Result<TrackerResponse, String> {
    println!("Testing HTTP tracker: {}", tracker);

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
    println!("Request URL: {}", url);
    let response = reqwest::blocking::get(&url).expect("Failed to send request");
    let status = response.status();
    println!("Response Status: {}", status);

    let bytes = response.bytes().expect("Failed to read bytes");
    let text = String::from_utf8_lossy(&bytes);
    println!("Response Body: {:?}", text);

    if !status.is_success() {
        return Err("Failed to get a successful response from the tracker".to_string());
    }

    let tracker_response = TrackerResponse::from_http_response(bytes.as_ref());
    dbg!(&tracker_response);

    Ok(tracker_response)
}

#[allow(dead_code)]
fn get_peers_udp(torrent: &Torrent, tracker: &str) {
    // send a connect request
    let mut buf = [0; 16];
    buf[0..8].copy_from_slice(&0x41727101980u64.to_be_bytes());
    buf[8..12].copy_from_slice(&0u32.to_be_bytes());
    buf[12..16].copy_from_slice(&rand::random::<u32>().to_be_bytes());
    println!("Connect request: {:?}", &buf);

    let socket = UdpSocket::bind("0.0.0.0:6969").expect("Failed to bind socket");
    let url = Url::parse(tracker).expect("Invalid URL");

    // 3. Resolve the target URL to a SocketAddr.
    let host = url.host_str().expect("No host in URL");
    let port = url.port().unwrap_or(80);
    let remote_addr = format!("{}:{}", host, port);

    socket
        .send_to(&buf, &remote_addr)
        .expect("Failed to send data");
    println!("UDP datagram sent to {}", remote_addr);
    let mut buf = [0; 1024];
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("Failed to set read timeout");
    // Receive repoonse
    let res = socket.recv_from(&mut buf);
    if res.is_err() {
        println!("No response received (timeout or error)");
        return;
    }

    let (amt, src) = res.unwrap();
    println!("Received {} bytes from {}: {:?}", amt, src, &buf[..amt]);

    let action = u32::from_be_bytes(buf[0..4].try_into().unwrap());
    let transaction_id = u32::from_be_bytes(buf[4..8].try_into().unwrap());
    let connection_id = u64::from_be_bytes(buf[8..16].try_into().unwrap());
    println!(
        "Action: {}, Transaction ID: {}, Connection ID: {}",
        action, transaction_id, connection_id
    );

    // Send announce request
    let peer_id_str = "-TR2940-fuckmek6wWLc";
    let peer_id: [u8; 20] = {
        let bytes = peer_id_str.as_bytes();
        let mut arr = [0u8; 20];
        arr[..bytes.len().min(20)].copy_from_slice(&bytes[..bytes.len().min(20)]);
        arr
    };

    let left = if let Some(length) = torrent.info.length {
        length as u64
    } else {
        torrent.info.files.as_ref().unwrap()[0].length as u64
    };

    let announce_request = AnnounceRequest {
        action: Action::AnnounceRequest,
        connection_id,
        downloaded: 0,
        transaction_id,
        info_hash: torrent.info_hash,
        event: Event::Empty,
        ip: None,
        key: 69420,
        peer_id,
        left,
        uploaded: 0,
        port: 6969,
        num_want: -1,
    };

    let buf = announce_request.to_be_bytes();
    println!("Announce request: {:?}", &buf);
    socket
        .send_to(&buf, &remote_addr)
        .expect("Failed to send data");

    let mut buf = [0; 1024];
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("Failed to set read timeout");
    // Receive repoonse
    let res = socket.recv_from(&mut buf);
    if res.is_err() {
        println!("No response received (timeout or error)");
        return;
    }

    let (amt, src) = res.unwrap();
    let num_peers = (amt - 20) / 6;
    println!("Received {amt} bytes, with {num_peers} peers");
    println!("Received {} bytes from {}: {:?}", amt, src, &buf[..amt]);
    let announce_response = AnnounceResponse::from_be_bytes(&buf[..amt]);
    dbg!(announce_response);
}
