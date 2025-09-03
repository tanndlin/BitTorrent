use std::net::{ToSocketAddrs, UdpSocket};

use crate::bencoding::decode;
use crate::bencoding::torrent::Torrent;

mod bencoding;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
async fn check_tracker(url: &str) -> Result<bool, String> {
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
        .send_to(data, remote_addr)
        .map_err(|e| format!("Failed to send data: {}", e))?;

    println!("UDP datagram sent to {}", remote_addr);

    // Receive a response with a timeout
    let mut buf = [0; 1024];
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| format!("Failed to set read timeout: {}", e))?;
    let res = socket
        .recv_from(&mut buf)
        .map_err(|e| format!("Failed to receive data: {}", e));
    match res {
        Ok((amt, src)) => {
            println!("Received {} bytes from {}: {:?}", amt, src, &buf[..amt]);
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

#[tauri::command]
fn parse_torrent(buffer: Vec<u8>) -> Torrent {
    // Read the file contents into the buffer
    decode::parse_metainfo(&buffer)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![check_tracker, parse_torrent])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
