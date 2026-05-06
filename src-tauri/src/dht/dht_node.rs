use std::{
    collections::{HashMap, HashSet},
    net::UdpSocket,
    time::Duration,
};

use rand::Rng;

use crate::{
    connection::Peer,
    dht::{
        krpc_request::{KRPCRequest, KRPCRequestGetPeers, KRPCRequestPing},
        krpc_response::KRPCResponse,
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
        let mut pending = HashMap::new();

        while !queue.is_empty() || !pending.is_empty() {
            while let Some(res) = self.recv_response() {
                let transaction_id = match &res {
                    KRPCResponse::Ping(p) => p.transaction_id,
                    KRPCResponse::GetPeers(gp) => gp.transaction_id,
                    KRPCResponse::FindNode(fn_) => fn_.transaction_id,
                    KRPCResponse::KRPCError(krpcerror) => krpcerror.transaction_id,
                };

                match pending.remove(&transaction_id) {
                    Some(KRPCRequest::Ping(_)) if matches!(res, KRPCResponse::Ping(_)) => {}
                    Some(KRPCRequest::GetPeers(_)) if matches!(res, KRPCResponse::GetPeers(_)) => {
                        let res = match res {
                            KRPCResponse::GetPeers(gp) => gp,
                            _ => unreachable!(),
                        };

                        if let Some(peer_list) = res.peers {
                            peers.extend(peer_list);
                        } else if let Some(nodes) = res.nodes {
                            for node in nodes {
                                // println!("Adding node {} to queue", node.location);
                                queue.push(node);
                            }
                        }
                    }
                    Some(KRPCRequest::FindNode(_)) if matches!(res, KRPCResponse::FindNode(_)) => {}
                    None => {}
                    Some(_) => {
                        println!(
                            "Unexpected response for transaction {}: {:?}",
                            hex::encode(transaction_id),
                            res
                        );
                    }
                }
            }

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
                match self.send_query(node, info_hash) {
                    Ok(req) => {
                        pending.insert(req.transaction_id, req.into());
                    }
                    Err(_err) => {
                        // println!("Error querying node {}: {}", node.location, err);
                        continue;
                    }
                }
            }
        }

        Ok(peers)
    }

    fn send_query(
        &self,
        node: &DhtNode,
        info_hash: &[u8; 20],
    ) -> Result<KRPCRequestGetPeers, String> {
        println!("Querying DHT node {} for peers", node.location);
        if let Some(node_id) = node.node_id {
            let dist = xor_distance(&node_id, info_hash);
            println!("Querying {} distance: {}", node.location, hex::encode(dist));
        }

        let req = KRPCRequestGetPeers::new(self.node_id, *info_hash);
        let encoded: Vec<u8> = req.clone().into();
        let addr = node.location.to_string();
        self.socket
            .send_to(&encoded, &addr)
            .map_err(|e| format!("Failed to send get_peers to {}: {}", addr, e))?;

        Ok(req)
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
        let mut buf = [0u8; 65536];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((size, src)) => match KRPCResponse::try_from(&buf[..size]) {
                    Ok(res) => return Some(res),
                    Err(err) => {
                        println!("Failed to parse from {}: {}", src, err);
                        continue; // skip garbage, keep waiting
                    }
                },
                Err(_) => return None, // timeout = genuinely no response
            }
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
