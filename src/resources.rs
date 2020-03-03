//! A number of resources that can be used to synchronize and trace components.

pub use self::{
    buffer::{BufferResource, ReceiveBufferResource, SentBufferResource},
    component::RegisteredComponentsResource,
    event::EventResource,
    packer::Packer,
    track::TrackResource,
};
use crate::tracking::SerializationStrategy;
use net_sync::compression::CompressionStrategy;
use legion::prelude::Resources;

mod buffer;
mod component;
mod event;
mod packer;
mod track;

pub mod tcp;

pub trait ResourcesExt<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static> {
    fn insert_server_resources(&mut  self, serialization: S, compression: C);
    fn insert_client_resources(&mut self, serialization: S, compression: C);
    fn insert_required(&mut self, serialization: S, compression: C);
}

impl<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static> ResourcesExt<S,C> for Resources {
    fn insert_server_resources(&mut self, serialization: S, compression: C) {
        self.insert(TrackResource::new());
        self.insert(ReceiveBufferResource::default());
        self.insert(RegisteredComponentsResource::new());
        self.insert(BufferResource::from_capacity(1500));
        self.insert_required(serialization, compression);
    }

    fn insert_client_resources(&mut self, serialization: S, compression: C) {
        self.insert(SentBufferResource::new());
        self.insert_required(serialization, compression);
    }

    fn insert_required(&mut self, serialization: S, compression: C) {
        self.insert(RegisteredComponentsResource::new());
        self.insert(Packer::<S, C>::default());
    }
}