use std::{io::Read, net::TcpStream};

use crate::peer::PeerProtocolError;

pub struct GreedyTcpStream<T> {
    pub stream: TcpStream,
    bytes_left: Vec<u8>,
    parser: fn(&[u8]) -> Option<(T, usize)>,
    buf: Box<[u8; 32768]>,
}

impl<T> GreedyTcpStream<T> {
    pub fn new(stream: TcpStream, parser: fn(&[u8]) -> Option<(T, usize)>) -> Self {
        Self {
            stream,
            bytes_left: vec![],
            parser,
            buf: Box::new([0u8; 32768]),
        }
    }

    pub fn try_read_message(&mut self) -> Result<Option<T>, PeerProtocolError> {
        // First check already-buffered bytes
        if let Some((message, bytes_used)) = (self.parser)(&self.bytes_left) {
            self.bytes_left.drain(0..bytes_used);
            return Ok(Some(message));
        }

        // Try a non-blocking read
        self.stream
            .set_read_timeout(Some(std::time::Duration::from_millis(1)))
            .unwrap();

        match self.stream.read(&mut *self.buf) {
            Ok(0) => Err(PeerProtocolError::ConnectionClosed),
            Ok(n) => {
                self.bytes_left.extend_from_slice(&self.buf[..n]);
                if let Some((message, bytes_used)) = (self.parser)(&self.bytes_left) {
                    self.bytes_left.drain(0..bytes_used);
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
