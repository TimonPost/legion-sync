use std::io::Write;
use std::{io, io::Read};

use legion::prelude::{Schedulable, SystemBuilder};
use log::{debug, error};

use net_sync::packer::Packer;
use net_sync::transport;
use net_sync::transport::tcp::{TcpClientResource, TcpListenerResource};
use net_sync::transport::{NetworkCommand, NetworkMessage};
use net_sync::{
    compression::CompressionStrategy,
    transport::{PostBox, PostOffice},
};
use track::serialization::SerializationStrategy;

use crate::resources::BufferResource;

pub fn tcp_connection_listener<
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_connection_listener")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>()
        .build(|_, _, resources, _| {
            if !resources.0.get().is_some() {
                return;
            }

            loop {
                let (stream, addr) = match resources.0.get().unwrap().accept() {
                    Ok((stream, addr)) => {
                        stream
                            .set_nonblocking(true)
                            .expect("Setting nonblocking mode");
                        stream.set_nodelay(true).expect("Setting nodelay");

                        debug!("Incoming TCP connection: {:?}", addr);

                        (stream, addr)
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(e) => {
                        error!("Error while handling TCP connection: {:?}", e);
                        // TODO: handle error
                        break;
                    }
                };

                resources.0.register_stream(addr, stream);
                resources.1.add_client(addr);
            }
        })
}

pub fn tcp_client_receive_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_receive_system")
        .write_resource::<TcpClientResource>()
        .write_resource::<BufferResource>()
        .write_resource::<PostBox<transport::ServerToClientMessage<ServerToClientMessage>, transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>>>()
        .read_resource::<Packer<S, C>>()
        .build(|_, _, resources, _|
    {
        let tcp: &mut TcpClientResource = &mut resources.0;
        let recv_buffer: &mut [u8] = &mut resources.1.recv_buffer;
        let postbox: &mut PostBox<transport::ServerToClientMessage<ServerToClientMessage>, transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>> = &mut resources.2;
        let unpacker: &Packer<S,C> = &resources.3;

        let result = tcp.stream().read(recv_buffer);

        match result {
            Ok(recv_len) => {
                if recv_len < 5 {
                    return;
                }

                // match unpacker
                //     .compression()
                //     .decompress(&recv_buffer[..recv_len]) {
                //     Ok(decompressed) => {
                        match unpacker
                            .serialization()
                            .deserialize::<Vec<transport::ServerToClientMessage<ServerToClientMessage>>>(&recv_buffer[..recv_len])
                        {
                            Ok(deserialized) => {
                                debug!("Received packet");
                                for packet in deserialized.into_iter() {
                                    postbox.add_to_inbox(packet);
                                }
                            }
                            Err(e) => {
                                error!("Error occurred when deserializing TCP-packet. Reason: {:?}", e);
                            }
                        }
                //     }
                //     Err(e) => {
                //         error!("Error occurred when decompressing TCP-packet. Reason: {:?}", e);
                //     }
                // }
            }
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::ConnectionReset => { }
                    io::ErrorKind::WouldBlock => { }
                    _ => {}
                };
            }
        }
    })
}

pub fn tcp_client_sent_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_sent_system")
        .write_resource::<TcpClientResource>()
        .write_resource::<PostBox<transport::ServerToClientMessage<ServerToClientMessage>, transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>>>()
        .read_resource::<Packer<S, C>>()
        .build(|_, _, resources, _| {
            let tcp_client: &mut TcpClientResource = &mut resources.0;
            let postbox: &mut PostBox<transport::ServerToClientMessage<ServerToClientMessage>, transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>> =
                &mut resources.1;
            let packer = &resources.2;

            if postbox.empty_outgoing() {
                return;
            }
            let packets =
                postbox
                    .drain_outgoing(|_| true)
                    .into_iter()
                    .collect::<Vec<
                        transport::ClientToServerMessage<
                            ClientToServerMessage,
                            ClientToServerCommand,
                        >,
                    >>();

            if packets.len() == 0 {
                return;
            }

            match &packer.serialization().serialize(&packets) {
                Ok(serialized) => {
                    // let compressed = packer.compression().compress(&serialized);
                    //
                    // if let Err(e) = tcp_client.sent(&compressed) {
                    //     error!("Error occurred when sending TCP-packet. Reason: {:?}", e);
                    // }

                    if let Err(e) = tcp_client.sent(&serialized) {
                        error!("Error occurred when sending TCP-packet. Reason: {:?}", e);
                    }
                }
                Err(e) => {
                    error!(
                        "Error occurred when serializing TCP-packet. Reason: {:?}",
                        e
                    );
                }
            }
        })
}

pub fn tcp_server_receive_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_receive_system")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>()
        .read_resource::<Packer<S, C>>()
        .write_resource::<BufferResource>()
        .build(|_, _, resources, _| {
            let tcp: &mut TcpListenerResource = &mut resources.0;
            let postoffice: &mut PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand> = &mut resources.1;
            let unpacker: &Packer<S,C> = &resources.2;
            let recv_buffer: &mut [u8] = &mut resources.3.recv_buffer;

            for (_, (active, stream)) in tcp.iter_mut() {
                // If we can't get a peer_addr, there is likely something pretty wrong with the
                // connection so we'll mark it inactive.
                let peer_addr = match stream.peer_addr() {
                    Ok(addr) => addr,
                    Err(_e) => {
                        *active = false;
                        continue;
                    }
                };

                loop {
                    let result = stream.read(recv_buffer);

                    match result {
                        Ok(recv_len) => {
                            if recv_len < 5 {
                                *active = false;
                                break;
                            }

                            debug!(
                                "Received {} bytes from TCP stream: {:?}.",
                                recv_len, peer_addr
                            );

                            let client = postoffice
                                .client_by_addr_mut(&peer_addr)
                                .expect("Client should exist");

                            let buffer = &recv_buffer[..recv_len];

                            // match unpacker
                            //     .compression()
                            //     .decompress(buffer) {
                            //     Ok(decompressed) => {
                                    match unpacker
                                        .serialization()
                                        .deserialize::<Vec<transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>>>(buffer) {
                                        Ok(mut deserialized) => {
                                            for packet in deserialized.iter_mut() {
                                                client
                                                    .add_received_message(packet.clone())
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error occurred when deserializing TCP-packet. Reason: {:?}", e);
                                        }
                                    // };
                            //     }
                            //     Err(e) => {
                            //         error!("Error occurred when decompressing TCP-packet. Reason: {:?}", e);
                            //     }
                            }
                        }
                        Err(e) => {
                            match e.kind() {
                                io::ErrorKind::ConnectionReset => {
                                    *active = false;
                                }
                                io::ErrorKind::WouldBlock => {}
                                _ => { }
                            };

                            break;
                        }
                    };
                }
            }
        })
}

pub fn tcp_server_sent_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_receive_system")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>()
        .write_resource::<Packer<S, C>>()
        .build(|_, _, resources, _| {
            let tcp: &mut TcpListenerResource = &mut resources.0;
            let postoffice: &mut PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand> = &mut resources.1;
            let packer: &Packer<S, C> = &resources.2;

            for client in postoffice.clients_mut() {
                let addr = client.1.addr();

                let postbox = client.1.postbox_mut();
                let client_stream = tcp
                    .get_stream(addr)
                    .expect("TCP didn't exist while it is supposed to.");

                let packets = postbox
                    .drain_outgoing(|_| true)
                    .into_iter()
                    .collect::<Vec<transport::ServerToClientMessage<ServerToClientMessage>>>();

                if packets.len() == 0 {
                    continue;
                }

                match &packer.serialization().serialize(&packets) {
                    Ok(serialized) => {
                        // let compressed = packer.compression().compress(&serialized);
                        //
                        // if let Err(e) = client_stream.1.write(&compressed) {
                        //     error!("Error occurred when sending TCP-packet. Reason: {:?}", e);
                        // }

                        if let Err(e) = client_stream.1.write(&serialized) {
                            error!("Error occurred when sending TCP-packet. Reason: {:?}", e);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Error occurred when serializing TCP-packet. Reason: {:?}",
                            e
                        );
                    }
                }
            }
        })
}
