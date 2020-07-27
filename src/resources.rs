//! A number of resources that can be used to synchronize and trace components.

use std::net::{SocketAddr, TcpListener};

use legion::{systems::Resources, Entity};

use net_sync::{
    compression::CompressionStrategy,
    synchronisation::{
        ClientCommandBuffer, CommandFrameTicker, NetworkCommand, NetworkMessage, ResimulationBuffer,
    },
    tracker::TrackResource,
    transport,
    transport::{
        tcp::{TcpClientResource, TcpListenerResource},
        PostBox, PostOffice,
    },
    uid::UidAllocator,
};

pub use self::{
    buffer::BufferResource,
    component::{HashmapRegistry, RegisteredComponentsResource},
    event::EventResource,
};
use net_sync::event::NetworkEventQueue;

mod buffer;
mod component;
mod event;

pub trait ResourcesExt {
    fn insert_server_resources<
        C: CompressionStrategy + 'static,
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    >(
        &mut self,
        compression: C,
    );

    fn insert_client_resources<
        C: CompressionStrategy + 'static,
        ClientToServerCommand: NetworkCommand,
    >(
        &mut self,
        compression: C,
    );

    fn insert_required<C: CompressionStrategy + 'static>(&mut self, compression: C);

    fn insert_tcp_client_resources<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    >(
        &mut self,
        addr: SocketAddr,
    );
    fn insert_tcp_listener_resources(&mut self, listener: TcpListener);
}

impl ResourcesExt for Resources {
    fn insert_server_resources<
        C: CompressionStrategy + 'static,
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    >(
        &mut self,
        compression: C,
    ) {
        self.insert(PostOffice::<
            ServerToClientMessage,
            ClientToServerMessage,
            ClientToServerCommand,
        >::new());
        self.insert_required(compression);
    }

    fn insert_client_resources<
        C: CompressionStrategy + 'static,
        ClientToServerCommand: NetworkCommand,
    >(
        &mut self,
        compression: C,
    ) {
        self.insert(ClientCommandBuffer::<ClientToServerCommand>::with_capacity(
            10,
        ));
        self.insert(ResimulationBuffer::<ClientToServerCommand>::new());
        self.insert_required(compression);
    }

    fn insert_required<C: CompressionStrategy + 'static>(&mut self, __compression: C) {
        self.insert(BufferResource::from_capacity(5000));
        self.insert(RegisteredComponentsResource::new());
        self.insert(UidAllocator::<Entity>::new());
        self.insert(TrackResource::new());
        self.insert(CommandFrameTicker::new(30.));
        self.insert(NetworkEventQueue::new());

        let registered_components = RegisteredComponentsResource::new();
        self.insert(registered_components);
    }

    fn insert_tcp_client_resources<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    >(
        &mut self,
        addr: SocketAddr,
    ) {
        self.insert(PostBox::<
            transport::ServerToClientMessage<ServerToClientMessage>,
            transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>,
        >::new());
        self.insert(TcpClientResource::new(addr).unwrap());
    }

    fn insert_tcp_listener_resources(&mut self, listener: TcpListener) {
        self.insert(TcpListenerResource::new(Some(listener)));
    }
}
