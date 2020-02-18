use crate::error::ErrorKind;
use std::{
    io::Write,
    net::{SocketAddr, TcpStream},
};

pub struct ClientResource {
    socket: TcpStream,
}

impl ClientResource {
    pub fn new(addr: SocketAddr) -> Result<ClientResource, ErrorKind> {
        Ok(ClientResource {
            socket: TcpStream::connect(addr)?,
        })
    }

    pub fn sent(&mut self, data: &[u8]) -> Result<usize, ErrorKind> {
        Ok(self.socket.write(data)?)
    }
}
