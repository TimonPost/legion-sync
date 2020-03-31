use crate::resources::{
    tcp::{TcpClientResource, TcpListenerResource},
    BufferResource, Packer, PostBoxResource, PostOfficeResource, TrackResource,
};
use legion::prelude::{Entity, Schedulable, SystemBuilder};
use log::{debug, error};
use net_sync::transport::Client;
use net_sync::uid::UidAllocator;
use net_sync::{
    compression::CompressionStrategy,
    transport::{PostBox, PostOffice, SentPacket},
    ClientMessage, EntityId, ServerMessage,
};
use std::io::Write;
use std::{io, io::Read};
use track::serialization::SerializationStrategy;

pub fn tcp_connection_listener() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_connection_listener")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOfficeResource>()
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
                resources.1.register_client(addr);
            }
        })
}

pub fn tcp_client_receive_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_receive_system")
        .write_resource::<TcpClientResource>()
        .write_resource::<BufferResource>()
        .write_resource::<PostBoxResource>()
        .read_resource::<Packer<S, C>>()
        .build(|_, _, resources, _|
    {
        let tcp: &mut TcpClientResource = &mut resources.0;
        let recv_buffer: &mut [u8] = &mut resources.1.recv_buffer;
        let postbox: &mut PostBox<ServerMessage, ClientMessage> = &mut resources.2;
        let unpacker: &Packer<S,C> = &resources.3;

        let result = tcp.stream().read(recv_buffer);

        match result {
            Ok(recv_len) => {
                if recv_len < 5 {
                    return;
                }

                match unpacker
                    .compression()
                    .decompress(&recv_buffer[0..recv_len]) {
                    Ok(decompressed) => {
                        match unpacker
                            .serialization()
                            .deserialize::<Vec<ServerMessage>>(&decompressed) {
                            Ok(deserialized) => {
                                debug!("Received: {:?}", deserialized);
                                for packet in deserialized.into_iter() {
                                    postbox.add_acknowledge_to_inbox(packet);
                                }
                            }
                            Err(e) => {
                                error!("Error occurred when deserializing TCP-packet. Reason: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error occurred when decompressing TCP-packet. Reason: {:?}", e);
                    }
                }
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

pub fn tcp_server_receive_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_receive_system")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOfficeResource>()
        .read_resource::<Packer<S, C>>()
        .write_resource::<BufferResource>()
        .write_resource::<TrackResource>()
        .write_resource::<UidAllocator<Entity>>()
        .build(|_, _, resources, _| {
            let tcp: &mut TcpListenerResource = &mut resources.0;
            let postoffice: &mut PostOffice = &mut resources.1;
            let unpacker: &Packer<S,C> = &resources.2;
            let recv_buffer: &mut [u8] = &mut resources.3.recv_buffer;
            let tracker: &mut TrackResource = &mut resources.4;
            let allocator: &mut UidAllocator<Entity> = &mut resources.5;

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
                                .clients_mut()
                                .by_addr_mut(&peer_addr)
                                .expect("Client should exist");

                            match unpacker
                                .compression()
                                .decompress(&recv_buffer[..recv_len]) {
                                Ok(decompressed) => {
                                    match unpacker
                                        .serialization()
                                        .deserialize::<Vec<ClientMessage>>(&decompressed) {
                                        Ok(mut deserialized) => {
                                            for packet in deserialized.iter_mut() {
                                                match packet {
                                                    ClientMessage::EntityInserted(mut entity_id, _) => {
                                                        debug!("Received Entity Inserted {:?}", entity_id);
                                                        let reserved = allocator.reserve_for(entity_id);
                                                        debug!("Reserved client id: {} server id {}", entity_id, reserved);
                                                        tracker.insert(entity_id as usize);
                                                    }
                                                    ClientMessage::ComponentModified(mut entity_id, _) => {
                                                        debug!("Received Entity Modified {:?}", entity_id);
                                                        replace_with_server_id(&mut entity_id, &allocator, &client);
                                                        tracker.modify(entity_id as usize);
                                                    }
                                                    ClientMessage::EntityRemoved(mut entity_id) => {
                                                        debug!("Received Entity Removed {:?}", entity_id);
                                                        replace_with_server_id(&mut entity_id, &allocator, &client);
                                                        tracker.remove(entity_id as usize);
                                                    }
                                                    ClientMessage::ComponentRemoved(mut entity_id) => {
                                                        debug!("Received Component Removed {:?}", entity_id);
                                                        replace_with_server_id(&mut entity_id, &allocator, &client);
                                                    }
                                                    ClientMessage::ComponentAdd(mut entity_id, _) => {
                                                        debug!("Received Component Add {:?}", entity_id);
                                                        replace_with_server_id(&mut entity_id, &allocator, &client);
                                                    }
                                                }

                                                client
                                                    .postbox_mut()
                                                    .add_to_inbox(packet.clone());
                                            }

                                        }
                                        Err(e) => {
                                            error!("Error occurred when deserializing TCP-packet. Reason: {:?}", e);
                                        }
                                    };
                                }
                                Err(e) => {
                                    error!("Error occurred when decompressing TCP-packet. Reason: {:?}", e);
                                }
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

pub fn tcp_client_sent_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_sent_system")
        .write_resource::<TcpClientResource>()
        .write_resource::<PostBoxResource>()
        .read_resource::<Packer<S, C>>()
        .build(|_, _, resources, _| {
            let tcp_client: &mut TcpClientResource = &mut resources.0;
            let postbox: &mut PostBox<ServerMessage, ClientMessage> = &mut resources.1;
            let packer = &resources.2;

            if postbox.empty_outgoing() {
                return;
            }
            let packets = postbox
                .drain_outgoing_with_priority(|_| true)
                .into_iter()
                .map(|message| message.event())
                .collect::<Vec<ClientMessage>>();

            if packets.len() == 0 {
                return;
            }

            match &packer.serialization().serialize(&packets) {
                Ok(serialized) => {
                    let compressed = packer.compression().compress(&serialized);

                    if let Err(e) = tcp_client.sent(&compressed) {
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

pub fn tcp_server_sent_system<
    S: SerializationStrategy + 'static,
    C: CompressionStrategy + 'static,
>() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_receive_system")
        .write_resource::<TcpListenerResource>()
        .write_resource::<PostOfficeResource>()
        .write_resource::<Packer<S, C>>()
        .build(|_, _, resources, _| {
            let tcp: &mut TcpListenerResource = &mut resources.0;
            let mut postoffice: &mut PostOffice = &mut resources.1;
            let packer: &Packer<S, C> = &resources.2;

            for client in postoffice.clients_mut().iter_mut() {
                let postbox = client.1.postbox_mut();
                let client_stream = tcp
                    .get_stream(postbox.addr())
                    .expect("TCP didn't exist while it is supposed to.");

                let packets = postbox
                    .drain_outgoing_with_priority(|_| true)
                    .into_iter()
                    .map(|message| message.event())
                    .collect::<Vec<ServerMessage>>();

                if packets.len() == 0 {
                    continue;
                }

                match &packer.serialization().serialize(&packets) {
                    Ok(serialized) => {
                        let compressed = packer.compression().compress(&serialized);

                        if let Err(e) = client_stream.1.write(&compressed) {
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

fn replace_with_server_id(client_id: &mut u32, allocator: &UidAllocator<Entity>, client: &Client) {
    if let Some(server_id) = client.is_accepted(*client_id) {
        debug!("Apply server id {} to client id {}", client_id, server_id);
        *client_id = server_id;
    } else {
        let server_id = allocator
            .reserved(*client_id)
            .expect(&format!("On insert should have reserve id for client id: {}", client_id));
        debug!("Apply server id {} to client id {}", client_id, server_id);
        *client_id = *server_id;
    }
}
