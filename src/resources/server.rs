use std::net::{SocketAddr, UdpSocket};

use net_sync::compression::{CompressionStrategy, ModificationCompressor};
use track::{preclude::SerialisationStrategy, serialisation::ModificationSerializer};

use crate::{NetworkPacket, NetworkPacketReader};

pub struct ServerUniverseResource<S: SerialisationStrategy, C: CompressionStrategy> {
    socket: UdpSocket,
    compression: ModificationCompressor<C>,
    serialisation: ModificationSerializer<S>,
    buffer: Vec<u8>,
}

impl<S: SerialisationStrategy, C: CompressionStrategy> ServerUniverseResource<S, C> {
    pub fn new(serialisation: S, compression: C, host: SocketAddr) -> ServerUniverseResource<S, C> {
        let host = UdpSocket::bind(host).unwrap();

        ServerUniverseResource {
            socket: host,
            compression: ModificationCompressor::<C>::new(compression),
            serialisation: ModificationSerializer::<S>::new(serialisation),
            buffer: vec![0; 1500],
        }
    }

    pub fn try_receive(&mut self) -> Option<Vec<NetworkPacket>> {
        if let Ok(size) = self.socket.recv(&mut self.buffer) {
            let decompressed = self
                .compression
                .decompress(self.buffer[0..size].to_vec())
                .unwrap();
            let deserialized = self
                .serialisation
                .deserialize::<Vec<Vec<u8>>>(&decompressed)
                .unwrap();

            return Some(
                deserialized
                    .iter()
                    .map(|d| NetworkPacketReader::new(&d).read().unwrap())
                    .collect(),
            );
        } else {
            None
        }
    }
}
