use legion::prelude::{IntoQuery, Read, Resources, Schedulable, SystemBuilder, Universe};
use legion_sync::{
    components::UidComponent,
    filters::filter_fns::{all, modified, removed},
    resources::{ReceiveBufferResource, TrackResource},
};
use net_sync::uid::UidAllocator;

struct Position;

fn main() {
    let universe = Universe::new();
    let mut world = universe.create_world();

    let mut resources = Resources::default();
    resources.insert(ReceiveBufferResource::default());
    resources.insert(TrackResource::new());

    world.insert(
        (),
        vec![(
            Position,
            UidComponent::new(UidAllocator::new().allocate(Some(1))),
        )],
    );
}
pub fn receive_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("read_received_system")
        .write_resource::<ReceiveBufferResource>()
        .write_resource::<TrackResource>()
        .with_query(<(legion::prelude::Write<Position>, Read<UidComponent>)>::query())
        .build(|command_buffer, mut world, resources, query| {
            let buffer = &mut resources.0;
            let tracker = &mut resources.1;

            let modified_filter = query.clone().filter(modified(&tracker));
            let removed_filter = query.clone().filter(removed(&tracker));
            let all_filter = query.clone().filter(all(&tracker));

            for (pos, identifier) in modified_filter.iter_mut(&mut world) {}

            for (pos, identifier) in removed_filter.iter_mut(&mut world) {}

            for (pos, identifier) in all_filter.iter_mut(&mut world) {}
        })
}
