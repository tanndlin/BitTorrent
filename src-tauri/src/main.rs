// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{ToSocketAddrs, UdpSocket};

mod bencoding;
mod connection;
use crate::{
    bencoding::{decode, util::Torrent},
    connection::{Action, AnnounceRequest, AnnounceResponse, Event, FromByte, ToByte},
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
    let peer_id_str = "-TR2940-6wfG2wk6wWLc";
    let peer_id: [u8; 20] = {
        let bytes = peer_id_str.as_bytes();
        let mut arr = [0u8; 20];
        arr[..bytes.len().min(20)].copy_from_slice(&bytes[..bytes.len().min(20)]);
        arr
    };

    let announce_request = AnnounceRequest {
        action: Action::ConnectRequest,
        connection_id,
        downloaded: 0,
        transaction_id,
        info_hash: torrent.info.pieces[0],
        event: Event::None,
        ip: None,
        key: 0,
        peer_id,
        left: torrent.info.files.as_ref().unwrap()[0].length as u64,
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
    println!("Received {} bytes from {}: {:?}", amt, src, &buf[..amt]);
    let announce_response = AnnounceResponse::from_be_bytes(&buf[..amt]);
    dbg!(announce_response);
}
