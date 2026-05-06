use std::{net::UdpSocket, time::Duration};

use rand::Rng;

use crate::{
    connection::Peer,
    dht::{
        krpc_request::{KRPCRequestGetPeers, KRPCRequestPing},
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
                node_id: [0u8; 20], // TODO: We should get the node ID from the tracker
                location: tracker,
            })
            .collect();

        DhtClient {
            socket,
            node_id,
            nodes,
        }
    }

    pub fn get_peers(&self) -> Result<Vec<Peer>, String> {
        let mut peers = vec![];

        for node in &self.nodes {
            if let Err(err) = self.send_ping(node) {
                println!("Error sending ping to {}: {}", node.location, err);
                continue;
            }

            let get_peers_request = KRPCRequestGetPeers::new(self.node_id, [0u8; 20]);
            let encoded: Vec<u8> = get_peers_request.into();
            let addr = node.location.to_string();
            self.socket
                .send_to(&encoded, &addr)
                .map_err(|e| format!("Failed to send get_peers to {}: {}", addr, e))
                .unwrap();

            if let Some(res) = self.recv_response() {
                match res {
                    KRPCResponse::GetPeers(get_peers_res) => {
                        if let Some(mut new_peers) = get_peers_res.peers {
                            peers.append(&mut new_peers);
                        } else {
                            println!(
                                "DhtNode sent other nodes instead of peers: {:?}",
                                get_peers_res.nodes
                            );
                        }
                    }
                    _ => println!(
                        "Expected get_peers response from {}, but got: {:?}",
                        addr, res
                    ),
                }
            }
        }

        Ok(peers)
    }

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

pub struct DhtNode {
    node_id: [u8; 20],
    location: String,
}
