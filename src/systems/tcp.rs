use crate::resources::tcp::TcpListenerResource;
use crate::resources::{
    BufferResource, ClientResource, Packer, ReceiveBufferResource, SentBufferResource,
};
use crate::{NetworkPacket, ReceivedPacket};
use legion::prelude::{Schedulable, SystemBuilder};
use log::debug;
use net_sync::compression::CompressionStrategy;
use std::io;
use std::io::Read;
use track::serialisation::SerialisationStrategy;

pub fn tcp_connection_listener() -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_connection_listener")
        .write_resource::<TcpListenerResource>()
        .build(|_, _, resources, _| {
            if !resources.get().is_some() {
                return;
            }

            loop {
                let (stream, addr) = match resources.get().unwrap().accept() {
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
                        debug!("Error while handling TCP connection: {:?}", e);
                        // TODO: handle error
                        break;
                    }
                };

                resources.add_stream(addr, stream);
            }

            debug!("tcp_connection_listener");
        })
}

pub fn tcp_receive_system<S: SerialisationStrategy + 'static, C: CompressionStrategy + 'static>(
) -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_receive_system")
        .write_resource::<TcpListenerResource>()
        .write_resource::<ReceiveBufferResource>()
        .read_resource::<Packer<S, C>>()
        .write_resource::<BufferResource>()
        .build(|_, _, resources, _| {
            let tcp = &mut resources.0;
            let recv_buffer = &mut resources.3.recv_buffer;
            let receive_queue = &mut resources.1;
            let unpacker = &resources.2;

            for (_, (active, stream)) in tcp.iter_mut() {
                // If we can't get a peer_addr, there is likely something pretty wrong with the
                // connection so we'll mark it inactive.
                let peer_addr = match stream.peer_addr() {
                    Ok(addr) => addr,
                    Err(e) => {
                        *active = false;
                        continue;
                    }
                };

                loop {
                    let result = stream.read(recv_buffer);

                    match result {
                        Ok(recv_len) => {
                            if recv_len > 0 {
                                debug!(
                                    "Received {} bytes from TCP stream: {:?}.",
                                    recv_len, peer_addr
                                );

                                let decompressed = unpacker
                                    .compression()
                                    .decompress(recv_buffer[..recv_len].to_vec())
                                    .unwrap();

                                let _ = unpacker
                                    .serialisation()
                                    .deserialize::<Vec<NetworkPacket>>(&decompressed)
                                    .unwrap()
                                    .into_iter()
                                    .map(|p| receive_queue.push(ReceivedPacket::new(peer_addr, p)))
                                    .collect::<()>();
                            } else {
                                *active = false;
                                break;
                            }
                        }
                        Err(e) => {
                            match e.kind() {
                                io::ErrorKind::ConnectionReset => {
                                    *active = false;
                                }
                                io::ErrorKind::WouldBlock => {}
                                _ => {}
                            }
                            break;
                        }
                    }
                }
            }
        })
}

pub fn tcp_sent_system<S: SerialisationStrategy + 'static, C: CompressionStrategy + 'static>(
) -> Box<dyn Schedulable> {
    SystemBuilder::new("tcp_sent_system")
        .write_resource::<ClientResource>()
        .write_resource::<SentBufferResource>()
        .read_resource::<Packer<S, C>>()
        .build(|_, _, resources, _| {
            let client = &mut resources.0;
            let sent_buffer = &mut resources.1;
            let packer = &resources.2;

            let data = sent_buffer
                .drain_messages(|_| true)
                .into_iter()
                .map(|message| NetworkPacket::new(message.uuid, message.event))
                .collect::<Vec<NetworkPacket>>();

            let compressed = packer
                .compression()
                .compress(&packer.serialisation().serialize(&data));

            client.sent(&compressed.data);
        })
}