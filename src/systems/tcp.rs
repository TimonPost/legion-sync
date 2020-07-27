use legion::systems::{Builder, SystemBuilder};

use net_sync::{
    synchronisation::{CommandFrameTicker, NetworkCommand, NetworkMessage},
    transport,
    transport::{
        tcp::{TcpClientResource, TcpListenerResource},
        PostBox, PostOffice,
    },
};

use crate::resources::BufferResource;
use net_sync::event::NetworkEventQueue;

pub fn tcp_connection_listener<
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>(
    builder: Builder,
) -> Builder {
    builder.add_system(SystemBuilder::new("tcp_connection_listener")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>()
        .write_resource::<NetworkEventQueue>()
        .build(|_, _, resources, _| {
            net_sync::transport::tcp::tcp_connection_listener(&mut resources.0, &mut resources.1, &mut resources.2);
        }))
}

pub fn tcp_client_receive_system<
    //    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>(
    builder: Builder,
) -> Builder {
    builder.add_system(
        SystemBuilder::new("tcp_client_receive_system")
            .write_resource::<TcpClientResource>()
            .write_resource::<PostBox<
                transport::ServerToClientMessage<ServerToClientMessage>,
                transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>,
            >>()
            .write_resource::<BufferResource>()
            .write_resource::<NetworkEventQueue>()
            .build(|_, _, resources, _| {
                net_sync::transport::tcp::tcp_client_receive_system(
                    &mut resources.0,
                    &mut resources.1,
                    &mut resources.3,
                    &mut resources.2.recv_buffer,
                )
            }),
    )
}

pub fn tcp_client_sent_system<
    //    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>(
    builder: Builder,
) -> Builder {
    builder.add_system(
        SystemBuilder::new("tcp_client_sent_system")
            .write_resource::<TcpClientResource>()
            .write_resource::<PostBox<
                transport::ServerToClientMessage<ServerToClientMessage>,
                transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>,
            >>()
            .write_resource::<NetworkEventQueue>()
            .build(|_, _, resources, _| {
                net_sync::transport::tcp::tcp_client_sent_system(
                    &mut resources.0,
                    &mut resources.1,
                    &mut resources.2,
                )
            }),
    )
}

pub fn tcp_server_receive_system<
    //    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>(
    builder: Builder,
) -> Builder {
    builder.add_system(SystemBuilder::new("tcp_server_receive_system")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>()
        .write_resource::<BufferResource>()
        .write_resource::<NetworkEventQueue>()
        .read_resource::<CommandFrameTicker>()
        .build(|_, _, resources, _| {
            net_sync::transport::tcp::tcp_server_receive_system(&mut resources.0, &mut resources.1, resources.4.command_frame(), &mut resources.3, &mut resources.2.recv_buffer)
        }))
}

pub fn tcp_server_sent_system<
    //    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>(
    builder: Builder,
) -> Builder {
    builder.add_system(SystemBuilder::new("tcp_server_sent_system")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>()
        .write_resource::<NetworkEventQueue>()
        .build(|_, _, resources, _| {
            net_sync::transport::tcp::tcp_server_sent_system(&mut resources.0, &mut resources.1, &mut resources.2);
        }))
}
