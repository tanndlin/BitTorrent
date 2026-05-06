use super::super::bencoding::decode;
use crate::bencoding::decode::Value;
use std::collections::HashMap;

#[derive(Debug)]
pub enum KRPCResponse {
    Ping(KRPCResponsePing),
    FindNode(KRPCResponseFindNode),
    // GetPeers(Vec<Peer>),
}

#[derive(Debug)]
pub struct KRPCResponsePing {
    transaction_id: [u8; 2],
    node_id: [u8; 20],
}

#[derive(Debug)]
pub struct KRPCResponseFindNode {
    transaction_id: [u8; 2],
    node_id: [u8; 20],
    nodes: Vec<String>,
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
                Value::Bytes(b) if b.len() == 20 => Some([
                    b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11],
                    b[12], b[13], b[14], b[15], b[16], b[17], b[18], b[19],
                ]),
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

impl From<KRPCResponsePing> for KRPCResponse {
    fn from(ping: KRPCResponsePing) -> Self {
        KRPCResponse::Ping(ping)
    }
}
