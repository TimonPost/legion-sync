use std::{
    io,
    io::{Read, Write},
};

use legion::prelude::{Schedulable, SystemBuilder};
use log::{debug, error};

use net_sync::{
    compression::CompressionStrategy,
    packer::Packer,
    serialization::SerializationStrategy,
    synchronisation::{CommandFrameTicker, NetworkCommand, NetworkMessage},
    transport,
    transport::{
        tcp::{TcpClientResource, TcpListenerResource},
        PostBox, PostOffice,
    },
};

use crate::resources::BufferResource;
use net_sync::{event::NetworkEventQueue, synchronisation::CommandFrame};
use std::{io::ErrorKind, net::Shutdown};

pub fn tcp_connection_listener<
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_connection_listener")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>()
        .write_resource::<NetworkEventQueue>()
        .build(|_, _, resources, _| {
            net_sync::transport::tcp::tcp_connection_listener(&mut resources.0, &mut resources.1, &mut resources.2);
        })
}

pub fn tcp_client_receive_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_client_receive_system")
        .write_resource::<TcpClientResource>()
        .write_resource::<PostBox<
            transport::ServerToClientMessage<ServerToClientMessage>,
            transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>,
        >>()
        .write_resource::<BufferResource>()
        .read_resource::<Packer<S, C>>()
        .write_resource::<NetworkEventQueue>()
        .build(|_, _, resources, _| {
            net_sync::transport::tcp::tcp_client_receive_system(
                &mut resources.0,
                &mut resources.1,
                &resources.3,
                &mut resources.4,
                &mut resources.2.recv_buffer,
            )
        })
}

pub fn tcp_client_sent_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_client_sent_system")
        .write_resource::<TcpClientResource>()
        .write_resource::<PostBox<
            transport::ServerToClientMessage<ServerToClientMessage>,
            transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>,
        >>()
        .read_resource::<Packer<S, C>>()
        .write_resource::<NetworkEventQueue>()
        .build(|_, _, resources, _| {
            net_sync::transport::tcp::tcp_client_sent_system(
                &mut resources.0,
                &mut resources.1,
                &resources.2,
                &mut resources.3,
            )
        })
}

pub fn tcp_server_receive_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_server_receive_system")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>()
        .read_resource::<Packer<S, C>>()
        .write_resource::<BufferResource>()
        .write_resource::<NetworkEventQueue>()
        .read_resource::<CommandFrameTicker>()
        .build(|_, _, resources, _| {
            net_sync::transport::tcp::tcp_server_receive_system(&mut resources.0, &mut resources.1, &resources.2, resources.5.command_frame(), &mut resources.4, &mut resources.3.recv_buffer)
        })
}

pub fn tcp_server_sent_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_server_sent_system")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>()
        .write_resource::<Packer<S, C>>()
        .write_resource::<NetworkEventQueue>()
        .build(|_, _, resources, _| {
            net_sync::transport::tcp::tcp_server_sent_system(&mut resources.0, &mut resources.1, &resources.2, &mut resources.3);
        })
}
