use std::io::Write;
use std::net::{SocketAddr, TcpStream};

pub struct ClientResource {
    socket: TcpStream,
}

impl ClientResource {
    pub fn new(addr: SocketAddr) -> ClientResource {
        ClientResource {
            socket: TcpStream::connect(addr).unwrap(),
        }
    }

    pub fn sent(&mut self, data: &[u8]) {
        self.socket.write(data).unwrap();
    }
}
