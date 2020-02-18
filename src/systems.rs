//! A number of systems that can be used to synchronize and trace components.

use legion::prelude::{Schedulable, SystemBuilder};

use crate::{
    components::UuidComponent,
    resources::{EventListenerResource, SentBufferResource},
};

pub mod tcp;

/// This system picks up all the changes since the last tick.
///
/// The modifications are retrieved from [EventListenerResource](LINK) and written to [TransportResource](LINK).
pub fn track_modifications_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("track_modifications_system")
        .write_resource::<SentBufferResource>()
        .read_resource::<EventListenerResource>()
        .read_component::<UuidComponent>()
        .build(|_, world, resources, _| {
            resources.1.gather_events(&mut resources.0, world);
        })
}
