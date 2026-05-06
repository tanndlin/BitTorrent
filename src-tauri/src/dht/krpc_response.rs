use super::super::bencoding::decode;
use crate::{bencoding::decode::Value, connection::Peer};
use std::collections::HashMap;

#[derive(Debug)]
pub enum KRPCResponse {
    Ping(KRPCResponsePing),
    FindNode(KRPCResponseFindNode),
    GetPeers(KRPCResponseGetPeers),
}

#[derive(Debug)]
pub struct KRPCResponsePing {
    pub transaction_id: [u8; 2],
    pub node_id: [u8; 20],
}

#[derive(Debug)]
pub struct KRPCResponseFindNode {
    transaction_id: [u8; 2],
    node_id: [u8; 20],
    nodes: Vec<String>,
}

#[derive(Debug)]
pub struct KRPCResponseGetPeers {
    transaction_id: [u8; 2],
    node_id: [u8; 20],
    pub peers: Option<Vec<Peer>>,
    pub nodes: Option<Vec<String>>,
    token: Option<Vec<u8>>,
}

pub struct KRPCError {
    error_code: KRPCErrorCode,
    error_message: String,
}

pub enum KRPCErrorCode {
    GenericError = 201,
    ServerError = 202,
    ProtocolError = 203,
    MethodUnknown = 204,
}

impl TryFrom<&[u8]> for KRPCResponse {
    type Error = String;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let decoded = match decode::decode_dictionary(value, &mut 0) {
            Value::Dict(d) => d,
            _ => panic!("Invalid KRPC response: expected dictionary"),
        };

        if let Ok(get_peers_response) = KRPCResponseGetPeers::try_from(&decoded) {
            return Ok(get_peers_response.into());
        }

        if let Ok(find_node_response) = KRPCResponseFindNode::try_from(&decoded) {
            return Ok(find_node_response.into());
        }

        if let Ok(ping_response) = KRPCResponsePing::try_from(&decoded) {
            return Ok(ping_response.into());
        }

        Err("Unknown KRPC response type".to_string())
    }
}

impl TryFrom<&HashMap<String, Value>> for KRPCResponsePing {
    type Error = String;

    fn try_from(dict: &HashMap<String, Value>) -> Result<Self, Self::Error> {
        let transaction_id = match dict.get("t") {
            Some(Value::Bytes(b)) if b.len() == 2 => [b[0], b[1]],
            _ => return Err("Missing or invalid transaction ID".to_string()),
        };
        let node_id = match dict.get("r").and_then(|r| match r {
            Value::Dict(d) => d.get("id").and_then(|id| match id {
                Value::Bytes(b) if b.len() == 20 => {
                    let id: [u8; 20] = b.as_slice().try_into().unwrap();
                    Some(id)
                }
                _ => None,
            }),
            _ => None,
        }) {
            Some(id) => id,
            None => return Err("Missing or invalid node ID in response".to_string()),
        };
        Ok(KRPCResponsePing {
            transaction_id,
            node_id,
        })
    }
}

impl TryFrom<&HashMap<String, Value>> for KRPCResponseFindNode {
    type Error = String;

    fn try_from(dict: &HashMap<String, Value>) -> Result<Self, Self::Error> {
        let transaction_id = match dict.get("t") {
            Some(Value::Bytes(b)) if b.len() == 2 => [b[0], b[1]],
            _ => return Err("Missing or invalid transaction ID".to_string()),
        };

        let res = match dict.get("r") {
            Some(Value::Dict(d)) => d,
            _ => return Err("Missing or invalid 'r' dictionary in response".to_string()),
        };

        let node_id = match res.get("id") {
            Some(Value::Bytes(b)) if b.len() == 20 => {
                let id: [u8; 20] = b.as_slice().try_into().unwrap();
                id
            }
            _ => return Err("Missing or invalid node ID in response".to_string()),
        };

        let nodes = match res.get("nodes") {
            Some(Value::Bytes(b)) => {
                if !b.len().is_multiple_of(26) {
                    return Err(
                        "Invalid 'nodes' value: length must be a multiple of 26".to_string()
                    );
                }
                (0..b.len())
                    .step_by(26)
                    .map(|i| {
                        let node_info = &b[i..i + 26];
                        let ip = format!(
                            "{}.{}.{}.{}",
                            node_info[0], node_info[1], node_info[2], node_info[3]
                        );
                        let port = ((node_info[24] as u16) << 8) | (node_info[25] as u16);
                        format!("{}:{}", ip, port)
                    })
                    .collect()
            }
            _ => return Err("Missing or invalid 'nodes' value in response".to_string()),
        };

        Ok(KRPCResponseFindNode {
            transaction_id,
            node_id,
            nodes,
        })
    }
}

impl TryFrom<&HashMap<String, Value>> for KRPCResponseGetPeers {
    type Error = String;

    fn try_from(dict: &HashMap<String, Value>) -> Result<Self, Self::Error> {
        let transaction_id = match dict.get("t") {
            Some(Value::Bytes(b)) if b.len() == 2 => [b[0], b[1]],
            Some(Value::Str(s)) if s.len() == 2 => {
                let bytes = s.as_bytes();
                [bytes[0], bytes[1]]
            }
            _ => return Err("Missing or invalid transaction ID".to_string()),
        };

        let res = match dict.get("r") {
            Some(Value::Dict(d)) => d,
            _ => return Err("Missing or invalid 'r' dictionary in response".to_string()),
        };

        let node_id = match res.get("id") {
            Some(Value::Bytes(b)) if b.len() == 20 => {
                let id: [u8; 20] = b.as_slice().try_into().unwrap();
                id
            }
            _ => return Err("Missing or invalid node ID in response".to_string()),
        };

        let token = match res.get("token") {
            Some(Value::Bytes(b)) => Some(b.clone()),
            _ => None,
        };

        let peers = match res.get("values") {
            Some(Value::Peers(p)) => Some(p.iter().map(Peer::from).collect()),
            _ => None,
        };

        let nodes = match res.get("nodes") {
            Some(Value::Bytes(b)) => {
                if b.len() % 26 != 0 {
                    return Err(
                        "Invalid 'nodes' value: length must be a multiple of 26".to_string()
                    );
                }
                Some(
                    (0..b.len())
                        .step_by(26)
                        .map(|i| {
                            let node_info = &b[i..i + 26];
                            let ip = format!(
                                "{}.{}.{}.{}",
                                node_info[0], node_info[1], node_info[2], node_info[3]
                            );
                            let port = ((node_info[24] as u16) << 8) | (node_info[25] as u16);
                            format!("{}:{}", ip, port)
                        })
                        .collect(),
                )
            }
            _ => None,
        };

        if peers.is_none() && nodes.is_none() {
            return Err("Response must contain either 'values' or 'nodes'".to_string());
        }

        Ok(KRPCResponseGetPeers {
            transaction_id,
            node_id,
            peers,
            nodes,
            token,
        })
    }
}

impl From<KRPCResponsePing> for KRPCResponse {
    fn from(ping: KRPCResponsePing) -> Self {
        KRPCResponse::Ping(ping)
    }
}

impl From<KRPCResponseFindNode> for KRPCResponse {
    fn from(find_node: KRPCResponseFindNode) -> Self {
        KRPCResponse::FindNode(find_node)
    }
}

impl From<KRPCResponseGetPeers> for KRPCResponse {
    fn from(get_peers: KRPCResponseGetPeers) -> Self {
        KRPCResponse::GetPeers(get_peers)
    }
}
