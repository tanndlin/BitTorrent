use super::super::bencoding::encode;
use crate::bencoding::decode::Value;
use rand::Rng;
use std::collections::HashMap;

pub enum KRPCRequest {
    Ping(KRPCRequestPing),
    FindNode(KRPCRequestFindNode),
    // GetPeers(KRPCRequestGetPeers),
    // AnnouncePeer(KRPCRequestAnnouncePeer),
}

pub struct KRPCRequestPing {
    transaction_id: [u8; 2],
    node_id: [u8; 20],
}

impl KRPCRequestPing {
    pub fn new(node_id: [u8; 20]) -> Self {
        let transaction_id = rand::rng().random::<[u8; 2]>();
        KRPCRequestPing {
            transaction_id,
            node_id,
        }
    }
}

pub struct KRPCRequestFindNode {
    transaction_id: [u8; 2],
    node_id: [u8; 20],
    target_id: [u8; 20],
}

impl KRPCRequestFindNode {
    pub fn new(node_id: [u8; 20], target_id: [u8; 20]) -> Self {
        let transaction_id = rand::rng().random::<[u8; 2]>();
        KRPCRequestFindNode {
            transaction_id,
            node_id,
            target_id,
        }
    }
}

impl From<KRPCRequest> for Vec<u8> {
    fn from(request: KRPCRequest) -> Self {
        match request {
            KRPCRequest::Ping(ping) => ping.into(),
            KRPCRequest::FindNode(find_node) => find_node.into(),
            // KRPCRequest::GetPeers(get_peers) => get_peers.into(),
            // KRPCRequest::AnnouncePeer(announce_peer) => announce_peer.into(),
        }
    }
}

impl From<KRPCRequestPing> for Vec<u8> {
    fn from(ping: KRPCRequestPing) -> Self {
        let mut dict = HashMap::new();
        dict.insert("t".to_string(), Value::Bytes(ping.transaction_id.to_vec()));
        dict.insert("y".to_string(), Value::Bytes(b"q".to_vec()));
        dict.insert("q".to_string(), Value::Bytes(b"ping".to_vec()));
        dict.insert(
            "a".to_string(),
            Value::Dict({
                let mut args = HashMap::new();
                args.insert("id".to_string(), Value::Bytes(ping.node_id.to_vec()));
                args
            }),
        );

        encode::encode_dictionary(&dict)
    }
}

impl From<KRPCRequestFindNode> for Vec<u8> {
    fn from(find_node: KRPCRequestFindNode) -> Self {
        let mut dict = HashMap::new();
        dict.insert(
            "t".to_string(),
            Value::Bytes(find_node.transaction_id.to_vec()),
        );
        dict.insert("y".to_string(), Value::Bytes(b"q".to_vec()));
        dict.insert("q".to_string(), Value::Bytes(b"find_node".to_vec()));
        dict.insert(
            "a".to_string(),
            Value::Dict({
                let mut args = HashMap::new();
                args.insert("id".to_string(), Value::Bytes(find_node.node_id.to_vec()));
                args.insert(
                    "target".to_string(),
                    Value::Bytes(find_node.target_id.to_vec()),
                );
                args
            }),
        );

        encode::encode_dictionary(&dict)
    }
}
