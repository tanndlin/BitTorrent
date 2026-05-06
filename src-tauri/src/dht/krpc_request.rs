use crate::bencoding::{decode::Value, encode};
use rand::Rng;
use std::collections::HashMap;

pub enum KRPCRequest {
    Ping(KRPCRequestPing),
    FindNode(KRPCRequestFindNode),
    GetPeers(KRPCRequestGetPeers),
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

pub struct KRPCRequestGetPeers {
    transaction_id: [u8; 2],
    node_id: [u8; 20],
    info_hash: [u8; 20],
}

impl KRPCRequestGetPeers {
    pub fn new(node_id: [u8; 20], info_hash: [u8; 20]) -> Self {
        let transaction_id = rand::rng().random::<[u8; 2]>();
        KRPCRequestGetPeers {
            transaction_id,
            node_id,
            info_hash,
        }
    }
}

impl From<KRPCRequest> for Vec<u8> {
    fn from(req: KRPCRequest) -> Self {
        match req {
            KRPCRequest::Ping(ping) => ping.into(),
            KRPCRequest::FindNode(find_node) => find_node.into(),
            KRPCRequest::GetPeers(get_peers) => get_peers.into(),
            // KRPCRequest::AnnouncePeer(announce_peer) => announce_peer.into(),
        }
    }
}

impl From<KRPCRequestPing> for Vec<u8> {
    fn from(req: KRPCRequestPing) -> Self {
        let mut dict = get_transaction_dict(req.transaction_id);
        dict.insert("y".to_string(), Value::Bytes(b"q".to_vec()));
        dict.insert("q".to_string(), Value::Bytes(b"ping".to_vec()));
        dict.insert(
            "a".to_string(),
            Value::Dict({
                let mut args = HashMap::new();
                args.insert("id".to_string(), Value::Bytes(req.node_id.to_vec()));
                args
            }),
        );

        encode::encode_dictionary(&dict)
    }
}

impl From<KRPCRequestFindNode> for Vec<u8> {
    fn from(req: KRPCRequestFindNode) -> Self {
        let mut dict = get_transaction_dict(req.transaction_id);
        dict.insert("y".to_string(), Value::Bytes(b"q".to_vec()));
        dict.insert("q".to_string(), Value::Bytes(b"find_node".to_vec()));
        dict.insert(
            "a".to_string(),
            Value::Dict({
                let mut args = HashMap::new();
                args.insert("id".to_string(), Value::Bytes(req.node_id.to_vec()));
                args.insert("target".to_string(), Value::Bytes(req.target_id.to_vec()));
                args
            }),
        );

        encode::encode_dictionary(&dict)
    }
}

impl From<KRPCRequestGetPeers> for Vec<u8> {
    fn from(req: KRPCRequestGetPeers) -> Self {
        let mut dict = get_transaction_dict(req.transaction_id);
        dict.insert("y".to_string(), Value::Bytes(b"q".to_vec()));
        dict.insert("q".to_string(), Value::Bytes(b"get_peers".to_vec()));
        dict.insert(
            "a".to_string(),
            Value::Dict({
                let mut args = HashMap::new();
                args.insert("id".to_string(), Value::Bytes(req.node_id.to_vec()));
                args.insert(
                    "info_hash".to_string(),
                    Value::Bytes(req.info_hash.to_vec()),
                );
                args
            }),
        );

        encode::encode_dictionary(&dict)
    }
}

fn get_transaction_dict(transaction_id: [u8; 2]) -> HashMap<String, Value> {
    let mut dict = HashMap::new();
    dict.insert("t".to_string(), Value::Bytes(transaction_id.to_vec()));
    dict
}
