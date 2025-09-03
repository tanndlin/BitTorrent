use std::{
    io::{Read, Write},
    net::TcpStream,
};

pub type MessageParser<T> = Box<dyn FnMut(&[u8]) -> Option<(T, usize)>>;

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
            self.stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .expect("Failed to set read timeout");

            match self.stream.read(&mut buf) {
                Ok(bytes_read) => {
                    if bytes_read == 0 {
                        panic!("Connection closed");
                    }
                    self.bytes_left.extend_from_slice(&buf[..bytes_read]);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Timeout occurred, send keep-alive message
                    self.stream
                        .write_all(&[0, 0, 0, 0])
                        .expect("Failed to send keep-alive");
                    // println!("Sent keep-alive message");
                    continue;
                }
                Err(e) => panic!("Failed to read from TCP stream: {}", e),
            }
        }
    }
}
