use std::{net::UdpSocket, time::Duration};

use rand::Rng;

use crate::{
    connection::Peer,
    dht::{krpc_request::KRPCRequestPing, krpc_response::KRPCResponse},
};

pub struct DHTNode {
    external_nodes: Vec<String>,
}

impl DHTNode {
    pub fn new(trackers: Vec<String>) -> Self {
        let mut external_nodes = trackers;
        if external_nodes.is_empty() {
            println!("No nodes found in DHT, adding default nodes");
            external_nodes = Self::get_default_nodes();
        }

        DHTNode { external_nodes }
    }

    pub fn get_peers(&self) -> Vec<Peer> {
        println!("Found {} nodes in DHT", self.external_nodes.len());
        // Send ping to each node
        self.external_nodes.iter().for_each(|node| {
            println!("Pinging node: {}", node);
            // Simulate ping response with a dummy peer
            // let peer = Peer {
            //     ip: IpAddr::V4(
            //         node.split(':')
            //             .nth(0)
            //             .and_then(|ip| ip.parse::<std::net::Ipv4Addr>().ok())
            //             .unwrap_or(std::net::Ipv4Addr::new(0, 0, 0, 0)),
            //     ),
            //     port: node
            //         .split(':')
            //         .nth(1)
            //         .and_then(|p| p.parse::<u16>().ok())
            //         .unwrap_or(0),
            // };

            DhtClient::new()
                .send_ping(node)
                .unwrap_or_else(|e| println!("Failed to ping {}: {}", node, e));
        });

        vec![]
    }

    fn get_default_nodes() -> Vec<String> {
        vec![
            "router.bittorrent.com:6881".to_string(),
            "dht.transmissionbt.com:6881".to_string(),
            "router.utorrent.com:6881".to_string(),
        ]
    }
}

pub struct DhtClient {
    socket: UdpSocket,
    node_id: [u8; 20],
}

impl DhtClient {
    pub fn new() -> Self {
        let socket = UdpSocket::bind("0.0.0.0:6881").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();

        let mut node_id = [0u8; 20];
        rand::rng().fill(&mut node_id);

        DhtClient { socket, node_id }
    }

    pub fn send_ping(&self, addr: &str) -> std::io::Result<()> {
        let ping_request = KRPCRequestPing::new(self.node_id);
        let encoded: Vec<u8> = ping_request.into();
        self.socket.send_to(&encoded, addr)?;

        if let Some(res) = self.recv_response() {
            println!("Received response from {}: {:?}", addr, res);
        } else {
            println!("No response received from {}", addr);
        }

        Ok(())
    }

    pub fn recv_response(&self) -> Option<KRPCResponse> {
        let mut buf = [0u8; 65536]; // max UDP packet size
        match self.socket.recv_from(&mut buf) {
            Ok((size, _)) => match KRPCResponse::try_from(&buf[..size]) {
                Ok(res) => Some(res),
                Err(err) => {
                    println!("Failed to parse KRPC response: {}", err);
                    None
                }
            },
            Err(_) => None,
        }
    }
}
