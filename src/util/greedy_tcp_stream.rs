use std::{io::Read, net::TcpStream};

use crate::peer::peer_protocol::PeerProtocolError;

pub type MessageParser<T> = Box<dyn FnMut(&[u8]) -> Option<(T, usize)>>;

pub struct GreedyTcpStream<T> {
    pub stream: TcpStream,
    pub bytes_left: Vec<u8>,
    pub parser: MessageParser<T>,
}

impl<T> GreedyTcpStream<T> {
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

        let mut buf = [0u8; 32768];
        match self.stream.read(&mut buf) {
            Ok(0) => Err(PeerProtocolError::ConnectionClosed),
            Ok(n) => {
                self.bytes_left.extend_from_slice(&buf[..n]);
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
