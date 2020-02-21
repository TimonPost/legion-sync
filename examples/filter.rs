use legion::prelude::{IntoQuery, Read, Resources, Schedulable, SystemBuilder, Universe};
use legion_sync::{
    components::UidComponent,
    filters::filter_fns::{all, modified, removed},
    resources::{ReceiveBufferResource, TrackResource},
};

struct Position;

fn main() {
    let universe = Universe::new();
    let mut world = universe.create_world();

    let mut resources = Resources::default();
    resources.insert(ReceiveBufferResource::default());

    world.insert(
        (),
        vec![(
            Position { x: 10, y: 10 },
            UidComponent::new(uid_allocator.allocate(Some(1))),
        )],
    );
}
pub fn receive_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("read_received_system")
        .write_resource::<ReceiveBufferResource>()
        .write_resource::<TrackResource>()
        .with_query(<(legion::prelude::Write<Position>, Read<UidComponent>)>::query())
        .build(|command_buffer, mut world, receive, query| {
            let modified_filter = query.clone().filter(modified(receive.1));
            let removed_filter = query.clone().filter(removed(receive.1));
            let all_filter = query.clone().filter(all(receive.1));

            for (pos, identifier) in modified_filter.iter_mut(&mut world) {}

            for (pos, identifier) in removed_filter.iter(&mut world) {}

            for (pos, identifier) in all_filter.iter(&mut world) {}
        })
}
