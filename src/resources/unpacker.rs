use net_sync::compression::{CompressionStrategy, ModificationCompressor};
use track::serialisation::{ModificationSerializer, SerialisationStrategy};

pub struct Packer<S: SerialisationStrategy, C: CompressionStrategy> {
    compression: ModificationCompressor<C>,
    serialisation: ModificationSerializer<S>,
}

impl<S: SerialisationStrategy, C: CompressionStrategy> Packer<S, C> {
    pub fn new(serialisation: S, compression: C) -> Packer<S, C> {
        Packer {
            serialisation: ModificationSerializer::new(serialisation),
            compression: ModificationCompressor::new(compression),
        }
    }

    pub fn compression(&self) -> &ModificationCompressor<C> {
        &self.compression
    }

    pub fn serialisation(&self) -> &ModificationSerializer<S> {
        &self.serialisation
    }
}

impl<S: SerialisationStrategy, C: CompressionStrategy> Default for Packer<S, C> {
    fn default() -> Self {
        Packer {
            serialisation: Default::default(),
            compression: Default::default(),
        }
    }
}
