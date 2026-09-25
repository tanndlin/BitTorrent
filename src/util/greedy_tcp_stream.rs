use std::{collections::VecDeque, io::Read, net::TcpStream};

use crate::peer::PeerProtocolError;

pub struct GreedyTcpStream<T> {
    pub stream: TcpStream,
    bytes_left: VecDeque<u8>,
    parser: fn(&mut VecDeque<u8>) -> Option<T>,
    buf: Box<[u8; 32768]>,
}

impl<T> GreedyTcpStream<T> {
    pub fn new(stream: TcpStream, parser: fn(&mut VecDeque<u8>) -> Option<T>) -> Self {
        Self {
            stream,
            bytes_left: VecDeque::new(),
            parser,
            buf: Box::new([0u8; 32768]),
        }
    }

    // Tries a non-blocking read
    pub fn try_read_message(&mut self) -> Result<Option<T>, PeerProtocolError> {
        // First check already-buffered bytes
        if let Some(message) = (self.parser)(&mut self.bytes_left) {
            return Ok(Some(message));
        }

        match self.stream.read(&mut *self.buf) {
            Ok(0) => Err(PeerProtocolError::ConnectionClosed),
            Ok(n) => {
                self.bytes_left.extend(&self.buf[..n]);
                if let Some(message) = (self.parser)(&mut self.bytes_left) {
                    return Ok(Some(message));
                }
                Ok(None)
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                Ok(None)
            }
            Err(_) => Err(PeerProtocolError::ConnectionClosed),
        }
    }
}
