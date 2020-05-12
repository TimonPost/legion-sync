//! A number of resources that can be used to synchronize and trace components.

use std::net::{SocketAddr, TcpListener};

use legion::prelude::{Entity, Resources};

use net_sync::packer::Packer;
use net_sync::track::TrackResource;
use net_sync::transport::tcp::{TcpClientResource, TcpListenerResource};
use net_sync::transport::{PostBox, PostOffice, NetworkMessage, NetworkCommand};
use net_sync::{compression::CompressionStrategy, uid::UidAllocator, transport};

use crate::tracking::SerializationStrategy;

pub use self::{
    buffer::BufferResource,
    component::{HashmapRegistry, RegisteredComponentsResource},
    event::EventResource,
};
use net_sync::synchronisation::{CommandFrameTicker, ClientCommandBuffer, ResimulationBuffer};

mod buffer;
mod component;
mod event;

pub trait ResourcesExt {
    fn insert_server_resources<
        S: SerializationStrategy + 'static,
        C: CompressionStrategy + 'static,
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    >(
        &mut self,
        serialization: S,
        compression: C,
    );

    fn insert_client_resources<
        S: SerializationStrategy + 'static,
        C: CompressionStrategy + 'static,
        ClientToServerCommand:  NetworkCommand
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

    fn insert_tcp_client_resources<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand>(&mut self, addr: SocketAddr);
    fn insert_tcp_listener_resources(&mut self, listener: TcpListener);
}

impl ResourcesExt for Resources {
    fn insert_server_resources<
        S: SerializationStrategy + 'static,
        C: CompressionStrategy + 'static,
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    >(
        &mut self,
        serialization: S,
        compression: C,
    ) {
        self.insert(PostOffice::<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>::new());
        self.insert_required(serialization, compression);
    }

    fn insert_client_resources<
        S: SerializationStrategy + 'static,
        C: CompressionStrategy + 'static,
        ClientToServerCommand:  NetworkCommand
    >(
        &mut self,
        serialization: S,
        compression: C,
    ) {
        self.insert(ClientCommandBuffer::<ClientToServerCommand>::with_capacity(10));
        self.insert(ResimulationBuffer::<ClientToServerCommand>::new());
        self.insert_required(serialization, compression);

    }

    fn insert_required<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        &mut self,
        __serialization: S,
        __compression: C,
    ) {
        self.insert(BufferResource::from_capacity(5000));
        self.insert(RegisteredComponentsResource::new());
        self.insert(Packer::<S, C>::default());
        self.insert(UidAllocator::<Entity>::new());
        self.insert(TrackResource::new());
        self.insert(CommandFrameTicker::new(200.))
    }

    fn insert_tcp_client_resources<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand>(&mut self, addr: SocketAddr) {
        self.insert( PostBox::<transport::ServerToClientMessage<ServerToClientMessage>, transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>>::new());
        self.insert(TcpClientResource::new(addr).unwrap());
    }

    fn insert_tcp_listener_resources(&mut self, listener: TcpListener) {
        self.insert(TcpListenerResource::new(Some(listener)));
    }
}
