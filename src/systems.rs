//! A number of systems that can be used to synchronize and trace components.

use legion::prelude::{Schedulable, SystemBuilder};

use crate::{
    components::UuidComponent,
    resources::{ClientUniverseResource, EventListenerResource, TransportResource},
};

/// This system picks up all the changes since the last tick.
///
/// The modifications are retrieved from [EventListenerResource](LINK) and written to [TransportResource](LINK).
pub fn track_modifications_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("track modifications")
        .write_resource::<TransportResource>()
        .read_resource::<EventListenerResource>()
        .read_component::<UuidComponent>()
        .build(|_, mut world, mut resources, _| {
            resources.1.gather_events(&mut resources.0, world);
        })
}

/// This system retrieves all packets with modified data. And compresses them before they are sent.
///
/// The packets are retrieved from [TransportResource](LINK) and sent to the endpoint with [ClientResource](LINK).
pub fn sent_updates_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("sent updates to server")
        .write_resource::<TransportResource>()
        .read_resource::<ClientUniverseResource>()
        .build(|_, mut world, mut resources, _| {
            if resources.0.has_messages() {
                let messages = resources.0.drain_messages(|x| true);
                resources.1.sent(messages);
            }
        })
}
