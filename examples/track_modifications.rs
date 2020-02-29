use legion::prelude::{
    any, IntoQuery, Read, Resources, Schedulable, Schedule, SystemBuilder, Universe,
};
use legion_sync::{
    components::UidComponent,
    resources::{EventResource, RegisteredComponentsResource, SentBufferResource},
    systems::track_modifications_system,
};
use net_sync::uid::Uid;
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

    let mut event_resource = EventResource::new();

    world.subscribe(event_resource.legion_subscriber().clone(), any());

    let mut resources = Resources::default();
    resources.insert(SentBufferResource::new());
    resources.insert(event_resource);
    resources.insert(RegisteredComponentsResource::new());

    world.insert(
        (),
        vec![(Position { x: 1, y: 1 }, UidComponent::new(Uid(1)))],
    );

    let mut scheduler = Schedule::builder()
        .add_system(track_modifications_system())
        .add_system(make_modification_system())
        .add_system(watch_modifications_system())
        .build();

    loop {
        scheduler.execute(&mut world, &mut resources);

        thread::sleep(Duration::from_millis(10));
    }
}

pub fn make_modification_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("move player")
        .read_resource::<EventResource>()
        .with_query(<(legion::prelude::Write<Position>, Read<UidComponent>)>::query())
        .build(|_, mut world, resource, query| {
            for (mut pos, identifier) in query.iter_mut(&mut world) {
                let mut pos = pos.track(resource.notifier(), identifier.uid());
                let new_pos = (pos.x + 1, pos.x + 1);
                pos.set(new_pos);
            }
        })
}

pub fn watch_modifications_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("read_received_system")
        .write_resource::<SentBufferResource>()
        .build(|_, _, sent_buffer, _| {
            for message in sent_buffer.drain_messages(|_| true).into_iter() {
                print!("identifier: {:?} \t| ", message.identifier());
                print!("urgency: {:?} \t| ", message.urgency());
                println!("event: {:?}", message.event());

                // - sent data over to other endpoint (see tcp_sent_system)
                // - apply changed data to struct (see `Apply`)
            }
        })
}
