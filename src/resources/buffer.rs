use std::collections::VecDeque;

use net_sync::{
    transport::{PostBox, PostOffice},
    uid::Uid,
};
use std::{
    net::SocketAddr,
    ops::{Deref, DerefMut},
};

pub struct BufferResource {
    pub(crate) recv_buffer: Vec<u8>,
}

impl BufferResource {
    pub fn from_capacity(size: usize) -> BufferResource {
        BufferResource {
            recv_buffer: vec![0; size],
        }
    }

    pub fn buffer(&self) -> &[u8] {
        &self.recv_buffer
    }
}

/// Resource serving as the owner of the queue of messages to be sent. This resource also serves
/// as the interface for other systems to send messages.
pub struct PostOfficeResource {
    postoffice: PostOffice,
    frame_budget_bytes: i32,
}

impl PostOfficeResource {
    /// Creates a new `TransportResource`.
    pub fn new() -> Self {
        Self {
            postoffice: PostOffice::new(),
            frame_budget_bytes: 0,
        }
    }
}

impl DerefMut for PostOfficeResource {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.postoffice
    }
}

impl Deref for PostOfficeResource {
    type Target = PostOffice;

    fn deref(&self) -> &Self::Target {
        &self.postoffice
    }
}

impl Default for PostOfficeResource {
    fn default() -> Self {
        Self {
            postoffice: PostOffice::new(),
            frame_budget_bytes: 0,
        }
    }
}

pub struct PostBoxResource {
    postbox: PostBox,
}

impl PostBoxResource {
    /// Creates a new `TransportResource`.
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            postbox: PostBox::new(addr),
        }
    }
}

impl DerefMut for PostBoxResource {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.postbox
    }
}

impl Deref for PostBoxResource {
    type Target = PostBox;

    fn deref(&self) -> &Self::Target {
        &self.postbox
    }
}
