#[derive(Debug)]
pub struct PeerHandshake {
    pub pstr: String,
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl From<&PeerHandshake> for Vec<u8> {
    fn from(handshake: &PeerHandshake) -> Vec<u8> {
        let mut buf = vec![];
        buf.push(handshake.pstr.len() as u8);
        buf.extend_from_slice(handshake.pstr.as_bytes());
        buf.extend_from_slice(&handshake.reserved);
        buf.extend_from_slice(&handshake.info_hash);
        buf.extend_from_slice(&handshake.peer_id);

        buf
    }
}

impl From<[u8; 68]> for PeerHandshake {
    fn from(bytes: [u8; 68]) -> Self {
        let pstr_len = bytes[0] as usize;
        let pstr = String::from_utf8(bytes[1..1 + pstr_len].to_vec()).unwrap();
        let mut reserved = [0; 8];
        reserved.copy_from_slice(&bytes[1 + pstr_len..1 + pstr_len + 8]);
        let mut info_hash = [0; 20];
        info_hash.copy_from_slice(&bytes[1 + pstr_len + 8..1 + pstr_len + 28]);
        let mut peer_id = [0; 20];
        peer_id.copy_from_slice(&bytes[1 + pstr_len + 28..1 + pstr_len + 48]);

        PeerHandshake {
            pstr,
            reserved,
            info_hash,
            peer_id,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PeerMessageID {
    KeepAlive = -1,
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
    Port = 9,
}

#[derive(Debug)]
pub struct PeerMessage {
    pub id: PeerMessageID,
    pub length: u32,
    pub payload: Vec<u8>,
}

impl From<&PeerMessage> for Vec<u8> {
    fn from(message: &PeerMessage) -> Self {
        let mut buf = vec![];
        buf.extend_from_slice(&(message.length).to_be_bytes());
        buf.push(message.id as u8);
        buf.extend_from_slice(&message.payload);

        buf
    }
}

impl PeerMessage {
    pub fn create_request(index: u32, begin: u32, length: u32) -> Self {
        let mut payload = Vec::<u8>::new();
        payload.extend_from_slice(&index.to_be_bytes());
        payload.extend_from_slice(&begin.to_be_bytes());
        payload.extend_from_slice(&length.to_be_bytes());
        PeerMessage {
            id: PeerMessageID::Request,
            length: 13,
            payload,
        }
    }

    pub fn create_interested() -> Self {
        PeerMessage {
            id: PeerMessageID::Interested,
            length: 1,
            payload: vec![],
        }
    }
}
