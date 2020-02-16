use std::net::UdpSocket;

use net_sync::compression::{lz4::Lz4Compression, ModificationCompressor};
use track::{preclude::Bincode, serialisation::ModificationSerializer};

use crate::packet::Message;

pub struct ClientUniverseResource {
    socket: UdpSocket,
    compression: ModificationCompressor<Lz4Compression>,
    serialisation: ModificationSerializer<Bincode>,
}

impl ClientUniverseResource {
    pub fn new() -> ClientUniverseResource {
        ClientUniverseResource {
            socket: UdpSocket::bind("127.0.0.1:1111").unwrap(),
            compression: ModificationCompressor::new(Lz4Compression),
            serialisation: ModificationSerializer::new(Bincode),
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
