//! A number of resources that can be used to synchronize and trace components.

pub use self::{
    buffer::{BufferResource, ReceiveBufferResource, SentBufferResource},
    event::EventResource,
    packer::Packer,
};

use legion::prelude::BitSet;

mod buffer;
mod event;
mod packer;

pub mod tcp;

#[derive(Debug)]
pub struct TrackResource {
    pub inserted: BitSet<u32>,
    pub modified: BitSet<u32>,
    pub removed: BitSet<u32>,
}

impl TrackResource {
    pub fn new() -> TrackResource {
        TrackResource {
            inserted: BitSet::new(),
            modified: BitSet::new(),
            removed: BitSet::new(),
        }
    }

    pub fn insert(&mut self, set: usize) {
        // If previously removed/modified we don't need to know that anymore.
        self.removed.remove(set);
        self.modified.remove(set);
        self.inserted.insert(set);
    }

    pub fn remove(&mut self, set: usize) {
        // Don't need to know that it was inserted/modified if it was subsequently
        // removed.
        self.inserted.remove(set);
        self.modified.remove(set);
        self.removed.remove(set);
    }

    pub fn modify(&mut self, set: usize) {
        self.modified.insert(set);
    }
}

impl Clone for TrackResource {
    fn clone(&self) -> Self {
        TrackResource {
            inserted: self.inserted.clone(),
            removed: self.removed.clone(),
            modified: self.removed.clone(),
        }
    }
}
