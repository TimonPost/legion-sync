use std::collections::HashMap;
use std::cell::RefCell;
use legion_sync::register::ComponentRegistration;
use legion::{Universe, any, World, EntityStore, Read, IntoQuery};

use legion_sync::tracking::*;
use legion_sync::resources::RegisteredComponentsResource;
use std::iter::FromIterator;

use type_uuid::TypeUuid;
use std::any::TypeId;
use legion::serialize::SerializableTypeId;
use serde::de::DeserializeSeed;
use bincode::Options;
use serde::Serializer;

#[sync]
#[derive(TypeUuid, Debug, PartialEq)]
#[uuid = "d4b83227-d3f8-47f5-b026-db615fb41d31"]
struct Pos{x: f32, y: f32,  z: f32}

impl Default for Pos {
    fn default() -> Self {
        Pos { x: 0., y: 0., z:0. }
    }
}

#[sync]
#[derive(TypeUuid, Debug, PartialEq)]
#[uuid = "14dec17f-ae14-40a3-8e44-e487fc423287"]
struct Vel{x: f32, y: f32, z: f32}

impl Default for Vel {
    fn default() -> Self {
        Vel { x: 0., y: 0., z:0. }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Unregistered(f32, f32, f32);

fn main() {
    // create world
    let universe = Universe::new();
    let mut world = universe.create_world();

    // Pos and Vel are both serializable, so all components in this chunkset will be serialized
    let entities = world.extend(
        vec![
            (Pos{x: 1., y: 2., z: 3.}, Vel{x: 1., y: 2., z: 3.0}),
            (Pos{x: 1., y: 2., z: 3.}, Vel{x: 1., y: 2., z: 3.1}),
            (Pos{x: 1., y: 2., z: 3.}, Vel{x: 1., y: 2., z: 3.2}),
            (Pos{x: 1., y: 2., z: 3.}, Vel{x: 1., y: 2., z: 3.3}),
        ],
    );

    let mut registry = legion::Registry::<SerializableTypeId>::new();
    registry.register_auto_mapped::<Pos>();
    registry.register_auto_mapped::<Vel>();

    let serialized = bincode::serialize(&world.as_serializable(any(), &registry)).unwrap();
    println!("{:?}", serialized);

    let mut world_new: World = registry
        .as_deserialize(&universe)
        .deserialize( &mut bincode::Deserializer::from_slice(&serialized, bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .allow_trailing_bytes()))
        .unwrap();

    println!("Number of entities in deserialized world {}", world_new.len());

    let mut query = <(Read<Pos>, Read<Vel>)>::query();

    for (pos, vel) in query.iter_mut(&mut world_new) {
        println!("deserialized {:?} {:?}", pos, vel)
    }

    let mut registry = legion::world::Duplicate::new();
    registry.register_clone::<Pos>();
    registry.register_clone::<Vel>();

    let mut new_world: World = universe.create_world();
    new_world.clone_from(&world, &any(), &mut registry);

    for (pos, vel) in query.iter_mut(&mut world_new) {
        println!("after merge {:?} {:?}", pos, vel)
    }
}