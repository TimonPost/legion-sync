use std::net::UdpSocket;

use net_sync::compression::{lz4::Lz4Compression, ModificationCompressor, CompressionStrategy};
use track::{preclude::Bincode, serialisation::{SerialisationStrategy,  ModificationSerializer}};

use crate::packet::Message;

pub struct ClientUniverseResource<S: SerialisationStrategy, C: CompressionStrategy> {
    socket: UdpSocket,
    compression: ModificationCompressor<C>,
    serialisation: ModificationSerializer<S>,
}

impl<S: SerialisationStrategy, C: CompressionStrategy> ClientUniverseResource<C,S> {
    pub fn new(serialisation: S, compression: C) -> ClientUniverseResource<S,C> {
        ClientUniverseResource {
            socket: UdpSocket::bind("127.0.0.1:1111").unwrap(),
            compression: ModificationCompressor::new(compression),
            serialisation: ModificationSerializer::new(serialisation),
        }
    }

    pub fn sent(&self, messages: Vec<Message>) {
        let data = messages
            .into_iter()
            .map(|message| message.payload)
            .collect::<Vec<Vec<u8>>>();

        let compressed = self
            .compression
            .compress(&self.serialisation.serialize(&data));

        self.socket.send_to(&compressed.data, "127.0.0.1:1119");
    }
}

impl<S: SerialisationStrategy, C: CompressionStrategy> Default for ClientUniverseResource<S,C> {
    fn default() -> Self {
        ClientUniverseResource {
            socket: UdpSocket::bind("127.0.0.1:0").unwrap(),
            compression: ModificationCompressor::new(C::default()),
            serialisation: ModificationSerializer::new(S::default()),
        }
    }
}