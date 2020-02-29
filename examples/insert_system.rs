use legion::prelude::{IntoQuery, Read, Resources, Schedulable, Schedule, SystemBuilder, Universe};
use legion_sync::{
    components::UidComponent,
    resources::{EventResource, RegisteredComponentsResource, SentBufferResource},
    systems::track_modifications_system,
};
use net_sync::uid::{Uid, UidAllocator};
use std::{thread, time::Duration};
use track::preclude::*;

#[track]
#[derive(Debug)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Position {
    pub fn set(&mut self, pos: (u16, u16)) {
        self.x = pos.0;
        self.y = pos.1;
    }
}

fn main() {
    let universe = Universe::new();
    let mut world = universe.create_world();

    let mut resources = Resources::default();

    let mut scheduler = Schedule::builder()
        .add_system(insert())
        .add_system(retreive())
        .build();

    loop {
        scheduler.execute(&mut world, &mut resources);

        thread::sleep(Duration::from_millis(500));
    }
}

/// Basic example of reading received entity synchronization data.
pub fn insert() -> Box<dyn Schedulable> {
    SystemBuilder::new("read_received_system")
        .with_query(<(legion::prelude::Write<Position>, Read<UidComponent>)>::query())
        .build(|command_buffer, mut world, resources, query| {
            command_buffer.insert(
                (),
                vec![(Position { x: 5, y: 5 }, UidComponent::new(Uid(1)))],
            );
        })
}

/// Basic example of reading received entity synchronization data.
pub fn retreive() -> Box<dyn Schedulable> {
    SystemBuilder::new("read_received_system")
        .with_query(<(legion::prelude::Write<Position>, Read<UidComponent>)>::query())
        .build(|command_buffer, mut world, resources, query| {
            /// 1) Filter and query modified components and retrieve the packets for those.
            //            let filter = query.clone().filter(modified(&resources.1));
            for (mut pos, identifier) in query.iter_mut(&mut world) {
                println!("a");
            }
        })
}
