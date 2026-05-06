use std::{collections::HashSet, net::UdpSocket, time::Duration};

use rand::Rng;

use crate::{
    connection::Peer,
    dht::{
        krpc_request::{KRPCRequestGetPeers, KRPCRequestPing},
        krpc_response::{KRPCResponse, KRPCResponseGetPeers},
    },
};

pub struct DhtClient {
    socket: UdpSocket,
    node_id: [u8; 20],
    pub nodes: Vec<DhtNode>,
}

impl DhtClient {
    pub fn new(trackers: Vec<String>) -> Self {
        let socket = UdpSocket::bind("0.0.0.0:6881").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();

        let mut node_id = [0u8; 20];
        rand::rng().fill(&mut node_id);

        let nodes = trackers
            .into_iter()
            .map(|tracker| DhtNode {
                node_id: None,
                location: tracker,
            })
            .collect();

        DhtClient {
            socket,
            node_id,
            nodes,
        }
    }

    pub fn get_peers(&self, info_hash: &[u8; 20]) -> Result<Vec<Peer>, String> {
        let mut peers = vec![];
        let mut queue: Vec<DhtNode> = self.nodes.clone();
        let mut visited = HashSet::new();

        loop {
            queue.sort_by_key(|n| n.node_id.map(|id| xor_distance(&id, info_hash)));

            let candidates: Vec<_> = queue
                .drain(..)
                .filter(|n| visited.insert(n.location.clone()))
                .take(8)
                .collect();

            if candidates.is_empty() {
                break; // only stop when we truly have nothing left to try
            }

            for node in &candidates {
                if let Some(res) = self.query(node, info_hash) {
                    if let Some(mut new_peers) = res.peers {
                        peers.append(&mut new_peers);
                        return Ok(peers);
                    }
                    if let Some(new_nodes) = res.nodes {
                        for new_node in new_nodes {
                            if !visited.contains(&new_node.location) {
                                queue.push(new_node);
                            }
                        }
                    }
                }
            }
        }

        Ok(peers)
    }

    fn query(&self, node: &DhtNode, info_hash: &[u8; 20]) -> Option<KRPCResponseGetPeers> {
        println!("Querying DHT node {} for peers", node.location);
        if let Some(node_id) = node.node_id {
            let dist = xor_distance(&node_id, info_hash);
            println!("Querying {} distance: {}", node.location, hex::encode(dist));
        }

        let get_peers_request = KRPCRequestGetPeers::new(self.node_id, *info_hash);
        let encoded: Vec<u8> = get_peers_request.into();
        let addr = node.location.to_string();
        if self.socket.send_to(&encoded, &addr).is_err() {
            println!("Failed to send get_peers request to {}", addr);
            return None;
        }

        if let Some(res) = self.recv_response() {
            match res {
                KRPCResponse::GetPeers(get_peers_res) => Some(get_peers_res),
                other => {
                    println!("Unexpected response type from {}: {:?}", addr, other);
                    None
                }
            }
        } else {
            println!("No response received from {}", addr);
            None
        }
    }

    #[allow(dead_code)]
    fn send_ping(&self, node: &DhtNode) -> Result<(), String> {
        let ping_request = KRPCRequestPing::new(self.node_id);
        let encoded: Vec<u8> = ping_request.into();

        let addr = node.location.to_string();
        self.socket
            .send_to(&encoded, &addr)
            .map_err(|e| format!("Failed to send ping to {}: {}", addr, e))?;

        if let Some(res) = self.recv_response() {
            println!("Received response from {}: {:?}", &addr, res);
            match res {
                KRPCResponse::Ping(_) => Ok(()),
                _ => Err(format!(
                    "Expected ping response from {}, but got: {:?}",
                    addr, res
                )),
            }
        } else {
            Err(format!("No response received from {}", addr))
        }
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

#[derive(Debug, Clone)]
pub struct DhtNode {
    node_id: Option<[u8; 20]>,
    location: String,
}

impl DhtNode {
    pub fn new(node_id: Option<[u8; 20]>, location: String) -> Self {
        DhtNode { node_id, location }
    }
}

fn xor_distance(a: &[u8; 20], b: &[u8; 20]) -> [u8; 20] {
    let mut dist = [0u8; 20];
    for i in 0..20 {
        dist[i] = a[i] ^ b[i];
    }
    dist
}
