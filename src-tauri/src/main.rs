// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{ToSocketAddrs, UdpSocket};

mod bencoding;
use crate::bencoding::{
    decode,
    encode::encode_tracker_get_request,
    util::{Download, Torrent},
};

fn check_tracker(url: &str) -> Result<bool, String> {
    // 1. Bind the UdpSocket to a local address.
    //    "0.0.0.0:0" allows the OS to choose an available port.
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind socket: {}", e))?;

    // 2. Define the target URL (hostname and port).
    let target_url: String = url.trim_start_matches("udp://").to_string(); // Replace with your target URL and port
    println!("{target_url}");

    // 3. Resolve the target URL to a SocketAddr.
    let remote_addr = target_url
        .to_socket_addrs()
        .map_err(|e| format!("Failed to resolve address: {}", e))?
        .next()
        .ok_or_else(|| "Could not resolve address".to_string())?;

    // 4. Prepare the data to send.
    let data = b"Hello, UDP!";

    // 5. Send the datagram.
    socket
        .send_to(data, &remote_addr)
        .map_err(|e| format!("Failed to send data: {}", e))?;

    println!("UDP datagram sent to {}", remote_addr);

    Ok(true)
}

fn main() {
    // bittorrent_lib::run();

    let content = std::fs::read("C:/Users/Tanner/Documents/torrents/Inglourious Basterds 2009 Inglorious Bastards DVDRip x264.torrent").expect("Failed to read file");
    let parsed = decode::parse_metainfo(&content);

    for tracker in &parsed.trackers {
        match check_tracker(&tracker.as_str()) {
            Ok(res) => {
                if res {
                    test(&parsed, tracker);
                }
            }
            Err(e) => println!("Error: {e}"),
        }
    }
}

fn test(torrent: &Torrent, tracker: &String) {
    // let peer_id = "-TR2940-6wfG2wk6wWLc".to_string();
    // let download = Download {
    //     torrent,
    //     peer_id,
    //     port: 6969,
    //     downloaded: 0,
    //     uploaded: 0,
    //     left: torrent.info.files.as_ref().unwrap()[0].length,
    //     ip: None,
    //     event: None,
    // };

    // let bytes = encode_tracker_get_request(&download, torrent.info.pieces[0]);
    // println!("Sending {} bytes to {tracker}", bytes.len());

    // let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
    // let target_url: String = tracker.trim_start_matches("udp://").to_string();
    // let remote_addr = target_url
    //     .to_socket_addrs()
    //     .expect("Failed to resolve address")
    //     .next()
    //     .expect("Could not resolve address");

    // socket
    //     .send_to(&bytes, &remote_addr)
    //     .expect("Failed to send data");
    // println!("UDP datagram sent to {}", remote_addr);
    // let mut buf = [0; 1024];
    // socket
    //     .set_read_timeout(Some(std::time::Duration::from_secs(5)))
    //     .expect("Failed to set read timeout");

    // // Receive repoonse
    // let res = socket.recv_from(&mut buf).expect("Failed to receive data");
    // let (amt, src) = res;
    // println!("Received {} bytes from {}: {:?}", amt, src, &buf[..amt]);

    // send a connect request
    let mut buf = [0; 16];
    buf[0..8].copy_from_slice(&0x41727101980u64.to_be_bytes());
    buf[8..12].copy_from_slice(&0u32.to_be_bytes());
    buf[12..16].copy_from_slice(&69u32.to_be_bytes());
    println!("Connect request: {:?}", &buf);

    let socket = UdpSocket::bind("0.0.0.0:6881").expect("Failed to bind socket");
    let target_url: String = tracker.trim_start_matches("udp://").to_string();
    let remote_addr = target_url
        .to_socket_addrs()
        .expect("Failed to resolve address")
        .next()
        .expect("Could not resolve address");

    socket
        .send_to(&buf, &remote_addr)
        .expect("Failed to send data");
    println!("UDP datagram sent to {}", remote_addr);
    let mut buf = [0; 1024];
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("Failed to set read timeout");
    // Receive repoonse
    let res = socket.recv_from(&mut buf).expect("Failed to receive data");
    let (amt, src) = res;
    println!("Received {} bytes from {}: {:?}", amt, src, &buf[..amt]);
}
