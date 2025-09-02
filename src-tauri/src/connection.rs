use core::panic;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use reqwest::Response;
use url::Url;
use urlencoding::decode;

use crate::bencoding::decode::{parse_dictionary, Value};

pub trait ToByte {
    fn to_be_bytes(&self) -> Vec<u8>;
}

pub trait FromByte {
    fn from_be_bytes(bytes: &[u8]) -> Self;
}

pub trait ToUrl {
    fn to_url_params(&self) -> String;
}

pub trait HTTPResponse {
    fn from_http_response(response: &[u8]) -> Self;
}

#[derive(Copy, Clone)]
pub enum Action {
    ConnectRequest = 0,
    ConnectResponse = 1,
    AnnounceRequest = 2,
    AnnounceResponse = 3,
}

#[derive(Copy, Clone)]
pub enum Event {
    Empty = 0,
    Completed = 1,
    Started = 2,
    Stopped = 3,
}

impl Event {
    fn to_string(&self) -> &str {
        match self {
            Event::Empty => "empty",
            Event::Completed => "completed",
            Event::Started => "started",
            Event::Stopped => "stopped",
        }
    }
}

pub struct TrackerRequest {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
    pub port: u16,
    pub uploaded: u64,
    pub downloaded: u64,
    pub left: u64,
    pub compact: u8,
    pub no_peer_id: bool,
    pub event: Event,
    pub ip: Option<IpAddr>,
    pub num_want: Option<i32>,
    pub key: Option<u32>,
    pub tracker_id: Option<String>,
}

impl ToUrl for TrackerRequest {
    fn to_url_params(&self) -> String {
        let info_hash_encoded = self
            .info_hash
            .iter()
            .map(|b| format!("%{:02x}", b))
            .collect::<String>();

        let peer_id_encoded = self
            .peer_id
            .iter()
            .map(|b| format!("%{:02x}", b))
            .collect::<String>();

        format!(
            "?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact={}&no_peer_id={}&event={}&ip={}&num_want={}&key={}&tracker_id={}",
            info_hash_encoded,
            peer_id_encoded,
            self.port,
            self.uploaded,
            self.downloaded,
            self.left,
            self.compact,
            self.no_peer_id as u8,
            self.event.to_string(),
            self.ip.map_or("".to_string(), |ip| ip.to_string()),
            self.num_want.map_or(-1, |n| n),
            self.key.map_or(0, |k| k),
            self.tracker_id.as_deref().unwrap_or(""),
        )
    }
}

#[derive(Debug)]
pub struct TrackerResponse {
    pub failure: Option<TrackerResponseError>,
    pub success: Option<TrackerResponseGood>,
}

#[derive(Debug)]
pub struct TrackerResponseError {
    pub failure_reason: String,
}

#[derive(Debug)]
pub struct TrackerResponseGood {
    pub warning_message: Option<String>,
    pub interval: u32,
    pub min_interval: Option<u32>,
    pub tracker_id: Option<String>,
    pub complete: u32,
    pub incomplete: u32,
    pub peers: Vec<Peer>,
}

impl HTTPResponse for TrackerResponse {
    fn from_http_response(response: &[u8]) -> Self {
        let map = match parse_dictionary(response, &mut 0) {
            Value::Dict(d) => d,
            _ => panic!("Expected a dictionary at the top level"),
        };
        dbg!(&map);

        if map.contains_key("failure reason") {
            let reason = if let Value::Str(s) = &map["failure reason"] {
                s.clone()
            } else {
                "".to_string()
            };
            TrackerResponse {
                failure: Some(TrackerResponseError {
                    failure_reason: reason,
                }),
                success: None,
            }
        } else {
            let warning_message = if let Some(Value::Str(s)) = map.get("warning message") {
                Some(s.clone())
            } else {
                None
            };

            let interval = if let Value::Number(n) = &map["interval"] {
                *n as u32
            } else {
                0
            };

            let min_interval = if let Some(Value::Number(n)) = map.get("min interval") {
                Some(*n as u32)
            } else {
                None
            };

            // let tracker_id = if let Some(Value::Str(s)) = map.get("tracker id") {
            //     s.clone()
            // } else {
            //     panic!("No tracker id in response");
            // };

            let tracker_id = if let Some(Value::Str(s)) = map.get("tracker id") {
                Some(s.clone())
            } else {
                None
            };

            let complete = if let Some(Value::Number(n)) = map.get("complete") {
                *n as u32
            } else {
                panic!("No complete in response");
            };

            let incomplete = if let Some(Value::Number(n)) = map.get("incomplete") {
                *n as u32
            } else {
                panic!("No incomplete in response");
            };

            let peers = if let Value::Peers(s) = &map["peers"] {
                s.iter().map(|x| Peer::from_be_bytes(x)).collect()
            } else {
                vec![]
            };

            TrackerResponse {
                failure: None,
                success: Some(TrackerResponseGood {
                    warning_message,
                    interval,
                    min_interval,
                    tracker_id,
                    complete,
                    incomplete,
                    peers,
                }),
            }
        }
    }
}

pub struct AnnounceRequest {
    pub connection_id: u64,
    pub action: Action,
    pub transaction_id: u32,
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
    pub downloaded: u64,
    pub left: u64,
    pub uploaded: u64,
    pub event: Event,
    pub ip: Option<IpAddr>,
    pub key: u32,
    pub num_want: i32,
    pub port: u16,
}

impl ToByte for AnnounceRequest {
    fn to_be_bytes(&self) -> Vec<u8> {
        let mut buf = [0; 98];
        buf[0..8].copy_from_slice(&self.connection_id.to_be_bytes());
        buf[8..12].copy_from_slice(&(self.action as u32).to_be_bytes());
        buf[12..16].copy_from_slice(&self.transaction_id.to_be_bytes());
        buf[16..36].copy_from_slice(&self.info_hash);
        buf[36..56].copy_from_slice(&self.peer_id);
        buf[56..64].copy_from_slice(&self.downloaded.to_be_bytes());
        buf[64..72].copy_from_slice(&self.left.to_be_bytes());
        buf[72..80].copy_from_slice(&self.uploaded.to_be_bytes());
        buf[80..84].copy_from_slice(&(self.event as u32).to_be_bytes());
        if let Some(ip) = &self.ip {
            match ip {
                IpAddr::V4(ipv4) => buf[84..88].copy_from_slice(&ipv4.octets()),
                IpAddr::V6(_) => buf[84..88].copy_from_slice(&[0, 0, 0, 0]), // or handle IPv6 as needed
            }
        }
        buf[88..92].copy_from_slice(&self.key.to_be_bytes());
        buf[92..96].copy_from_slice(&self.num_want.to_be_bytes());
        buf[96..98].copy_from_slice(&self.port.to_be_bytes());

        buf.to_vec()
    }
}

#[derive(Debug)]
pub struct Peer {
    pub ip: IpAddr,
    pub port: u16,
}

impl FromByte for Peer {
    fn from_be_bytes(bytes: &[u8]) -> Self {
        Peer {
            ip: IpAddr::V4(Ipv4Addr::from(<[u8; 4]>::try_from(&bytes[0..4]).unwrap())),
            port: u16::from_be_bytes(bytes[4..6].try_into().unwrap()),
        }
    }
}

#[derive(Debug)]
pub struct AnnounceResponse {
    pub action: u32,
    pub transaction_id: u32,
    pub interval: u32,
    pub leechers: u32,
    pub seeders: u32,
    pub peers: Vec<Peer>,
}

impl FromByte for AnnounceResponse {
    fn from_be_bytes(bytes: &[u8]) -> Self {
        let action = u32::from_be_bytes(bytes[0..4].try_into().unwrap());
        let transaction_id = u32::from_be_bytes(bytes[4..8].try_into().unwrap());
        let interval = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
        let leechers = u32::from_be_bytes(bytes[12..16].try_into().unwrap());
        let seeders = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let peers: Vec<Peer> = bytes[20..]
            .chunks_exact(6)
            .map(Peer::from_be_bytes)
            .collect();

        AnnounceResponse {
            action,
            transaction_id,
            interval,
            leechers,
            seeders,
            peers,
        }
    }
}

pub struct ScrapeRequest {
    pub connection_id: u64,
    pub action: u32,
    pub transaction_id: u32,
    pub hashes: Vec<[u8; 20]>,
}

impl ToByte for ScrapeRequest {
    fn to_be_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::<u8>::new();
        buf.extend_from_slice(&self.connection_id.to_be_bytes());
        buf.extend_from_slice(&self.action.to_be_bytes());
        buf.extend_from_slice(&self.transaction_id.to_be_bytes());
        for hash in &self.hashes {
            buf.extend_from_slice(hash);
        }

        buf
    }
}

pub struct ScrapeSubresponse {
    pub seeders: u32,
    pub completed: u32,
    pub leechers: u32,
}

impl FromByte for ScrapeSubresponse {
    fn from_be_bytes(bytes: &[u8]) -> ScrapeSubresponse {
        ScrapeSubresponse {
            seeders: u32::from_be_bytes(bytes[0..4].try_into().unwrap()),
            completed: u32::from_be_bytes(bytes[4..8].try_into().unwrap()),
            leechers: u32::from_be_bytes(bytes[8..12].try_into().unwrap()),
        }
    }
}

pub struct ScrapeResponse {
    pub action: u32,
    pub transaction_id: u32,
    pub sub_response: Vec<ScrapeSubresponse>,
}

impl FromByte for ScrapeResponse {
    fn from_be_bytes(bytes: &[u8]) -> ScrapeResponse {
        let action = u32::from_be_bytes(bytes[0..4].try_into().unwrap());
        let transaction_id = u32::from_be_bytes(bytes[4..8].try_into().unwrap());
        let mut sub_response = Vec::<ScrapeSubresponse>::new();

        let mut index = 8;
        while index < bytes.len() {
            sub_response.push(ScrapeSubresponse::from_be_bytes(&bytes[index..index + 12]));
            index += 12;
        }

        ScrapeResponse {
            action,
            transaction_id,
            sub_response,
        }
    }
}

pub fn check_tracker(url: &str) -> Result<bool, String> {
    if url.starts_with("udp://") {
        check_udp_tracker(url)
    } else if url.starts_with("http://") || url.starts_with("https://") {
        Ok(true)
    } else {
        Err("Unsupported tracker protocol".to_string())
    }
}

fn check_udp_tracker(url: &str) -> Result<bool, String> {
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind socket: {}", e))?;

    // 2. Define the target URL (hostname and port).
    let url = Url::parse(url).expect("Invalid URL");

    // 3. Resolve the target URL to a SocketAddr.
    let host = url.host_str().expect("No host in URL");
    let port = url.port().unwrap_or(80);
    let remote_addr = format!("{}:{}", host, port);

    // 4. Prepare the data to send.
    let data = b"Hello, UDP!";

    // 5. Send the datagram.
    socket
        .send_to(data, &remote_addr)
        .map_err(|e| format!("Failed to send data: {}", e))?;

    println!("UDP datagram sent to {}", remote_addr);

    Ok(true)
}
