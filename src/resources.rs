//! A number of resources that can be used to synchronize and trace components.

pub use self::{
    buffer::{BufferResource, ReceiveBufferResource, SentBufferResource},
    component::RegisteredComponentsResource,
    event::EventResource,
    packer::Packer,
    track::TrackResource,
};

mod buffer;
mod component;
mod event;
mod packer;
mod track;

pub mod tcp;
