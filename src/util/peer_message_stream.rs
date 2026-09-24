use std::{collections::VecDeque, io::Write, net::TcpStream};

use crate::{
    peer::{PeerMessage, PeerMessageID, PeerProtocolError},
    util::greedy_tcp_stream::GreedyTcpStream,
};

pub struct PeerMessageStream {
    pub stream: GreedyTcpStream<PeerMessage>,
}

impl PeerMessageStream {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream: GreedyTcpStream::new(stream, parse_next_peer_message),
        }
    }

    pub fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.stream.stream.write_all(buf)
    }

    pub fn try_read_message(&mut self) -> Result<Option<PeerMessage>, PeerProtocolError> {
        self.stream.try_read_message()
    }
}

fn parse_next_peer_message(buf: &mut VecDeque<u8>) -> Option<PeerMessage> {
    if buf.len() < 4 {
        return None;
    }

    let length = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < 4 + length {
        return None;
    }

    if length == 0 {
        // Keep-alive message
        buf.drain(..4);
        return Some(PeerMessage {
            id: PeerMessageID::KeepAlive,
            length: 0,
            payload: vec![],
        });
    }

    let id = match buf[4] {
        0 => PeerMessageID::Choke,
        1 => PeerMessageID::Unchoke,
        2 => PeerMessageID::Interested,
        3 => PeerMessageID::NotInterested,
        4 => PeerMessageID::Have,
        5 => PeerMessageID::Bitfield,
        6 => PeerMessageID::Request,
        7 => PeerMessageID::Piece,
        8 => PeerMessageID::Cancel,
        9 => PeerMessageID::Port,
        20 => PeerMessageID::Extended,
        _ => {
            println!("Unknown message ID: {}", buf[4]);
            return None;
        }
    };
    let payload = buf.make_contiguous()[5..4 + length].to_vec();
    buf.drain(..4 + length);
    Some(PeerMessage {
        id,
        length: (length - 1) as u32,
        payload,
    })
}
