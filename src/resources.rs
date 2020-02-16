//! A number of resources that can be used to synchronize and trace components.

pub use self::{
    client::ClientUniverseResource, event_listener::EventListenerResource,
    server::ServerUniverseResource, transport::TransportResource,
};

mod client;
mod event_listener;
mod server;
mod transport;
