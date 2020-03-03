use legion_sync::register::ComponentRegister;
use legion::prelude::{Universe, Read, Query, IntoQuery, World, Write};
use legion::filter::{EntityFilterTuple, Passthrough, ComponentFilter};
use legion_sync::tracking::*;
use legion_sync::components::UidComponent;
use net_sync::uid::Uid;
use std::collections::HashMap;
use std::ptr::replace;
use legion_sync::network_universe::NetworkUniverse;

#[sync]
#[derive(Debug)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Default for Position {
    fn default() -> Self {
        Position {
            x: 5,
            y: 5
        }
    }
}

fn main() {
    let registered_components = ComponentRegister::by_component_id();

    // We want to do plain copies of all the data
    let clone_impl = legion_sync::create_copy_clone_impl();

    let mut universe = NetworkUniverse::new();
    let mut local = universe.create_world();
    let mut remote = universe.create_world();

    let query = <(Read<Position>)>::query();

    // insert into remote world
    remote.insert(
        (),
        vec![(
            Position { x: 1, y: 1 },
            UidComponent::new(Uid(1)),
        )],
    );

    let mut counter =0;

    loop {
        increment(&mut remote);

        universe.merge_into(&mut local, &remote);
        counter += 1;

        if counter == 5 {
            remote.insert(
                (),
                vec![(
                    Position { x: 2, y: 2 },
                    UidComponent::new(Uid(2)),
                )],
            );
        }

        if counter == 10 {
            break;
        }
    }

    print_world(&query, &remote);
    print_world(&query, &local);
}

fn print_world(query: &Query<Read<Position>, EntityFilterTuple<ComponentFilter<Position>, Passthrough, Passthrough>>, world: &World) {
    println!("====World====");
    for (entity, component) in query.iter_entities(&world) {
        println!("{:?} => {:?}", entity, *component)
    }
}

fn increment(world: &mut World) {
    for mut pos in <(Write<Position>)>::query().iter_mut(world) {
        pos.x += 1;
    }
}

fn component_count(query: &Query<Read<Position>, EntityFilterTuple<ComponentFilter<Position>, Passthrough, Passthrough>>, world: &World) -> u32 {
    let mut counter = 0;

    for component in query.iter(&world) {
        counter += 1;
    }

    counter
}
