use net_sync::compression::{CompressionStrategy, ModificationCompressor};
use track::serialization::{ModificationSerializer, SerializationStrategy};

pub struct Packer<S: SerializationStrategy, C: CompressionStrategy> {
    compression: ModificationCompressor<C>,
    serialization: ModificationSerializer<S>,
}

impl<S: SerializationStrategy, C: CompressionStrategy> Packer<S, C> {
    pub fn new(serialization: S, compression: C) -> Packer<S, C> {
        Packer {
            serialization: ModificationSerializer::new(serialization),
            compression: ModificationCompressor::new(compression),
        }
    }

    pub fn compression(&self) -> &ModificationCompressor<C> {
        &self.compression
    }

    pub fn serialization(&self) -> &ModificationSerializer<S> {
        &self.serialization
    }
}

impl<S: SerializationStrategy, C: CompressionStrategy> Default for Packer<S, C> {
    fn default() -> Self {
        Packer {
            serialization: Default::default(),
            compression: Default::default(),
        }
    }
}
