use super::super::bencoding::decode;
use crate::{bencoding::decode::Value, connection::Peer, dht::dht_node::DhtNode};
use std::collections::HashMap;

#[derive(Debug)]
pub enum KRPCResponse {
    Ping(KRPCResponsePing),
    FindNode(KRPCResponseFindNode),
    GetPeers(KRPCResponseGetPeers),
    KRPCError(KRPCError),
}

#[derive(Debug)]
pub struct KRPCResponsePing {
    pub transaction_id: [u8; 2],
    pub node_id: [u8; 20],
}

#[derive(Debug)]
pub struct KRPCResponseFindNode {
    pub transaction_id: [u8; 2],
    node_id: [u8; 20],
    nodes: Vec<DhtNode>,
}

#[derive(Debug)]
pub struct KRPCResponseGetPeers {
    pub transaction_id: [u8; 2],
    node_id: [u8; 20],
    pub peers: Option<Vec<Peer>>,
    pub nodes: Option<Vec<DhtNode>>,
    token: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct KRPCError {
    pub transaction_id: [u8; 2],
    error_code: KRPCErrorCode,
    error_message: String,
}

#[derive(Debug)]
pub enum KRPCErrorCode {
    GenericError = 201,
    ServerError = 202,
    ProtocolError = 203,
    MethodUnknown = 204,
}

impl TryFrom<&[u8]> for KRPCResponse {
    type Error = String;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let decoded = match decode::decode_dictionary(value, &mut 0)? {
            Value::Dict(d) => d,
            _ => panic!("Invalid KRPC response: expected dictionary"),
        };

        if let Ok(err_response) = KRPCError::try_from(&decoded) {
            return Ok(err_response.into());
        }

        if let Ok(get_peers_response) = KRPCResponseGetPeers::try_from(&decoded) {
            return Ok(get_peers_response.into());
        }

        if let Ok(find_node_response) = KRPCResponseFindNode::try_from(&decoded) {
            return Ok(find_node_response.into());
        }

        if let Ok(ping_response) = KRPCResponsePing::try_from(&decoded) {
            return Ok(ping_response.into());
        }

        Err(format!(
            "Failed to parse KRPC response: no matching response type found. Decoded value: {:?}",
            decoded
        ))
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

                        let node_id: [u8; 20] = node_info[0..20].try_into().unwrap();
                        let ip = format!(
                            "{}.{}.{}.{}",
                            node_info[20], node_info[21], node_info[22], node_info[23]
                        );
                        let port = ((node_info[24] as u16) << 8) | (node_info[25] as u16);
                        let location = format!("{}:{}", ip, port);

                        DhtNode::new(Some(node_id), location)
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
            Some(Value::List(list)) => {
                let parsed: Vec<Peer> = list
                    .iter()
                    .filter_map(|v| {
                        if let Value::Bytes(b) = v {
                            b.as_slice().try_into().ok()
                        } else {
                            None
                        }
                    })
                    .collect();
                if parsed.is_empty() {
                    None
                } else {
                    Some(parsed)
                }
            }
            _ => None,
        };

        let nodes = match res.get("nodes") {
            Some(Value::Bytes(b)) if b.is_empty() => None,
            Some(Value::Str(s)) if s.is_empty() => None,
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

                            let node_id: [u8; 20] = node_info[0..20].try_into().unwrap();
                            let ip = format!(
                                "{}.{}.{}.{}",
                                node_info[20], node_info[21], node_info[22], node_info[23]
                            );
                            let port = ((node_info[24] as u16) << 8) | (node_info[25] as u16);
                            let location = format!("{}:{}", ip, port);

                            DhtNode::new(Some(node_id), location)
                        })
                        .collect(),
                )
            }
            _ => None,
        };

        Ok(KRPCResponseGetPeers {
            transaction_id,
            node_id,
            peers,
            nodes,
            token,
        })
    }
}

impl TryFrom<&HashMap<String, Value>> for KRPCError {
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

        let error_code = match dict.get("e") {
            Some(Value::List(l)) if l.len() == 2 => match &l[0] {
                Value::Number(n) => match *n as u16 {
                    201 => KRPCErrorCode::GenericError,
                    202 => KRPCErrorCode::ServerError,
                    203 => KRPCErrorCode::ProtocolError,
                    204 => KRPCErrorCode::MethodUnknown,
                    _ => return Err("Unknown error code in KRPC error response".to_string()),
                },
                _ => return Err("Invalid error code format in KRPC error response".to_string()),
            },
            _ => return Err("Missing or invalid 'e' list in KRPC error response".to_string()),
        };

        let error_message = match dict.get("e").and_then(|e| match e {
            Value::List(l) if l.len() == 2 => match &l[1] {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        }) {
            Some(msg) => msg,
            None => {
                return Err("Missing or invalid error message in KRPC error response".to_string())
            }
        };

        Ok(KRPCError {
            transaction_id,
            error_code,
            error_message,
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

impl From<KRPCError> for KRPCResponse {
    fn from(error: KRPCError) -> Self {
        KRPCResponse::KRPCError(error)
    }
}
