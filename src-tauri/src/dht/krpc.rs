use std::collections::HashMap;

use crate::bencoding::decode::Value;

use super::super::bencoding::{decode, encode};

pub enum KRPCMessageType {
    Query,
    Response,
    Error,
}

pub enum KRPCMessage {
    Query(KRPCRequest),
    Response(KRPCResponse),
    Error(KRPCError),
}

pub struct KRPCRequest {
    transaction_id: String,
    query_type: KRPCQueryType,
    arguments: HashMap<String, Vec<u8>>,
}

pub enum KRPCQueryType {
    Ping,
    FindNode,
    GetPeers,
    AnnouncePeer,
}

impl From<KRPCQueryType> for String {
    fn from(query_type: KRPCQueryType) -> Self {
        match query_type {
            KRPCQueryType::Ping => "ping".to_string(),
            KRPCQueryType::FindNode => "find_node".to_string(),
            KRPCQueryType::GetPeers => "get_peers".to_string(),
            KRPCQueryType::AnnouncePeer => "announce_peer".to_string(),
        }
    }
}

pub struct KRPCResponse {
    transaction_id: String,
    response: HashMap<String, Vec<u8>>,
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

impl From<KRPCRequest> for Vec<u8> {
    fn from(request: KRPCRequest) -> Self {
        let mut map = HashMap::new();
        map.insert("t".to_string(), Value::Str(request.transaction_id));
        map.insert("y".to_string(), Value::Str("q".to_string()));
        map.insert("q".to_string(), Value::Str(request.query_type.into()));
        map.insert(
            "a".to_string(),
            Value::Dict(
                request
                    .arguments
                    .into_iter()
                    .map(|(k, v)| (k, Value::Bytes(v)))
                    .collect(),
            ),
        );
        encode::encode_dictionary(map)
    }
}

impl From<&[u8]> for KRPCResponse {
    fn from(value: &[u8]) -> Self {
        let decoded = match decode::decode_dictionary(value, &mut 0) {
            Value::Dict(d) => d,
            _ => panic!("Invalid KRPC response: expected dictionary"),
        };
        let transaction_id = match decoded.get("t") {
            Some(Value::Str(t)) => t.clone(),
            _ => panic!("Invalid KRPC response: missing transaction ID"),
        };
        let response = match decoded.get("r") {
            Some(Value::Dict(r)) => r
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        match v {
                            Value::Bytes(b) => b.clone(),
                            _ => panic!("Invalid KRPC response: expected bytes in response dict"),
                        },
                    )
                })
                .collect(),
            _ => panic!("Invalid KRPC response: missing response dict"),
        };
        KRPCResponse {
            transaction_id,
            response,
        }
    }
}
