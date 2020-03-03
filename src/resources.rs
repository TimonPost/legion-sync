//! A number of resources that can be used to synchronize and trace components.

pub use self::{
    buffer::{BufferResource, ReceiveBufferResource, SentBufferResource},
    component::RegisteredComponentsResource,
    event::EventResource,
    packer::Packer,
    track::TrackResource,
};
use crate::resources::tcp::{TcpClientResource, TcpListenerResource};
use crate::tracking::SerializationStrategy;
use legion::prelude::Resources;
use net_sync::compression::CompressionStrategy;
use std::net::{SocketAddr, TcpListener};

mod buffer;
mod component;
mod event;
mod packer;
mod track;

pub mod tcp;

pub trait ResourcesExt {
    fn insert_server_resources<
        S: SerializationStrategy + 'static,
        C: CompressionStrategy + 'static,
    >(
        &mut self,
        serialization: S,
        compression: C,
    );
    fn insert_client_resources<
        S: SerializationStrategy + 'static,
        C: CompressionStrategy + 'static,
    >(
        &mut self,
        serialization: S,
        compression: C,
    );
    fn insert_required<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        &mut self,
        serialization: S,
        compression: C,
    );

    fn insert_tcp_client_resources(&mut self, addr: SocketAddr);
    fn insert_tcp_listener_resources(&mut self, listener: TcpListener);
}

impl ResourcesExt for Resources {
    fn insert_server_resources<
        S: SerializationStrategy + 'static,
        C: CompressionStrategy + 'static,
    >(
        &mut self,
        serialization: S,
        compression: C,
    ) {
        self.insert(TrackResource::new());
        self.insert(ReceiveBufferResource::default());
        self.insert(RegisteredComponentsResource::new());
        self.insert(BufferResource::from_capacity(1500));
        self.insert_required(serialization, compression);
    }

    fn insert_client_resources<
        S: SerializationStrategy + 'static,
        C: CompressionStrategy + 'static,
    >(
        &mut self,
        serialization: S,
        compression: C,
    ) {
        self.insert(SentBufferResource::new());
        self.insert_required(serialization, compression);
    }

    fn insert_required<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        &mut self,
        serialization: S,
        compression: C,
    ) {
        self.insert(RegisteredComponentsResource::new());
        self.insert(Packer::<S, C>::default());
    }

    fn insert_tcp_client_resources(&mut self, addr: SocketAddr) {
        self.insert(TcpClientResource::new(addr).unwrap());
    }

    fn insert_tcp_listener_resources(&mut self, listener: TcpListener) {
        self.insert(TcpListenerResource::new(Some(listener)));
    }
}
