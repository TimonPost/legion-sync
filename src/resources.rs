//! A number of resources that can be used to synchronize and trace components.

pub use self::{
    client::ClientResource, event_listener::EventListenerResource,
    transport::ReceiveBufferResource, transport::SentBufferResource, unpacker::Packer,
};

mod client;
mod event_listener;
mod transport;
mod unpacker;

pub mod tcp;

pub struct BufferResource {
    pub recv_buffer: Vec<u8>,
}

impl BufferResource {
    pub fn from_capacity(size: usize) -> BufferResource {
        BufferResource {
            recv_buffer: vec![0; size],
        }
    }
}
