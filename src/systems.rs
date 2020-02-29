//! A number of systems that can be used to synchronize and trace components.

use legion::prelude::{Schedulable, SystemBuilder};

use crate::resources::{EventResource, RegisteredComponentsResource, SentBufferResource};

pub mod tcp;

/// This system picks up all the changes since the last tick.
///
/// The modifications are retrieved from [EventListenerResource](LINK) and written to [TransportResource](LINK).
pub fn track_modifications_system() -> Box<dyn Schedulable> {
    let mut builder = SystemBuilder::new("track_modifications_system");

    for component in RegisteredComponentsResource::new().slice().iter() {
        builder = component.1.add_to_system(builder);
    }

    builder
        .write_resource::<SentBufferResource>()
        .read_resource::<EventResource>()
        .read_resource::<RegisteredComponentsResource>()
        .build(|_, world, resources, _| {
            resources
                .1
                .gather_events(&mut resources.0, &resources.2, world);
        })
}
