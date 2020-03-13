use crate::{
    resources::{EventResource, PostBoxResource, PostOfficeResource, RegisteredComponentsResource},
    systems::SystemBuilderExt,
};
use legion::prelude::{Entity, Schedulable, SystemBuilder};
use net_sync::{transport::PostOffice, uid::UidAllocator};

/// This system picks up all the changes since the last tick.
///
/// The modifications are retrieved from [EventListenerResource](LINK) and written to [TransportResource](LINK).
pub fn track_modifications_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("track_modifications_system")
        .read_registered_components()
        .write_resource::<PostBoxResource>()
        .read_resource::<EventResource>()
        .read_resource::<RegisteredComponentsResource>()
        .write_resource::<UidAllocator<Entity>>()
        .build(|_, world, resources, _| {
            resources
                .1
                .gather_events(&mut resources.0, &resources.2, &mut resources.3, world);
        })
}
