use crate::peer::{PeerMessage, PeerMessageID};
use std::{
    io::{Read, Write},
    net::TcpStream,
};

type MessageParser<T> = Box<dyn FnMut(&[u8]) -> Option<(T, usize)>>;

pub struct GreedyTcpStream<T> {
    pub stream: TcpStream,
    pub bytes_left: Vec<u8>,
    pub parser: MessageParser<T>,
}

impl<T> GreedyTcpStream<T> {
    pub fn get_next_message(&mut self) -> T {
        loop {
            if let Some((message, bytes_used)) = (self.parser)(&self.bytes_left) {
                self.bytes_left.drain(0..bytes_used);
                return message;
            }

            let mut buf = [0; 32768];
            let bytes_read = self
                .stream
                .read(&mut buf)
                .expect("Failed to read from TCP stream");
            if bytes_read == 0 {
                panic!("Connection closed");
            }

            self.bytes_left.extend_from_slice(&buf[..bytes_read]);
        }
    }
}

impl<T> Drop for GreedyTcpStream<T> {
    fn drop(&mut self) {
        println!("Closing GreedyTcpStream connection");
        if let Err(e) = self.stream.shutdown(std::net::Shutdown::Both) {
            println!("Failed to shutdown TCP stream: {}", e);
        }
    }
}

fn parse_next_peer_message(buf: &[u8]) -> Option<(PeerMessage, usize)> {
    if buf.len() < 4 {
        return None;
    }

    let length = u32::from_be_bytes(buf[0..4].try_into().unwrap()) as usize;
    if buf.len() < 4 + length {
        return None;
    }

    if length == 0 {
        // Keep-alive message
        return Some((
            PeerMessage {
                id: PeerMessageID::KeepAlive,
                length: 0,
                payload: vec![],
            },
            4,
        ));
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
        _ => {
            println!("Unknown message ID: {}", buf[4]);
            return None;
        }
    };
    let payload = buf[5..4 + length].to_vec();
    Some((
        PeerMessage {
            id,
            length: (length - 1) as u32,
            payload,
        },
        4 + length,
    ))
}

pub struct PeerMessageStream {
    pub stream: GreedyTcpStream<PeerMessage>,
}

impl PeerMessageStream {
    pub fn new(stream: TcpStream) -> Self {
        let parser: MessageParser<PeerMessage> = Box::new(parse_next_peer_message);
        Self {
            stream: GreedyTcpStream {
                stream,
                bytes_left: Vec::new(),
                parser,
            },
        }
    }

    // Add a convenience method to get the next peer message
    pub fn get_next_message(&mut self) -> PeerMessage {
        self.stream.get_next_message()
    }

    pub fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.stream.stream.write_all(buf)
    }
}

impl Drop for PeerMessageStream {
    fn drop(&mut self) {
        println!("Closing PeerMessageStream connection");
        // The GreedyTcpStream will handle the actual TCP stream cleanup
    }
}
