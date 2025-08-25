use std::net::{IpAddr, Ipv4Addr};

pub trait ToByte {
    fn to_be_bytes(&self) -> Vec<u8>;
}

pub trait FromByte {
    fn from_be_bytes(bytes: &[u8]) -> Self;
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
    None = 0,
    Completed = 1,
    Started = 2,
    Stopped = 3,
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
        buf[8..12].copy_from_slice(&(self.action as u32).to_be_bytes()); // action
        buf[12..16].copy_from_slice(&self.transaction_id.to_be_bytes());
        buf[16..36].copy_from_slice(&self.info_hash); // info hash
        buf[36..56].copy_from_slice(&self.peer_id); // peer id
        buf[56..64].copy_from_slice(&self.downloaded.to_be_bytes());
        buf[64..72].copy_from_slice(&self.left.to_be_bytes());
        buf[72..80].copy_from_slice(&self.uploaded.to_be_bytes());
        buf[80..84].copy_from_slice(&(self.event as u32).to_be_bytes()); // event
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
struct Peer {
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
            .map(|chunk| Peer::from_be_bytes(chunk.into()))
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

struct ScrapeSubresponse {
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
