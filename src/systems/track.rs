use crate::resources::{EventResource, RegisteredComponentsResource, SentBufferResource};
use legion::prelude::{Entity, Schedulable, SystemBuilder};
use net_sync::uid::UidAllocator;

/// This system picks up all the changes since the last tick.
///
/// The modifications are retrieved from [EventListenerResource](LINK) and written to [TransportResource](LINK).
pub fn track_modifications_system() -> Box<dyn Schedulable> {
    let mut builder = SystemBuilder::new("track_modifications_system");

    for component in RegisteredComponentsResource::new().slice_with_uid().iter() {
        builder = component.1.add_to_system(builder);
    }

    builder
        .write_resource::<SentBufferResource>()
        .read_resource::<EventResource>()
        .read_resource::<RegisteredComponentsResource>()
        .write_resource::<UidAllocator<Entity>>()
        .build(|_, world, resources, _| {
            resources
                .1
                .gather_events(&mut resources.0, &resources.2, &mut resources.3, world);
        })
}
