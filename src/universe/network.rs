use crate::{
    event::{LegionEvent, LegionEventHandler},
    resources::{EventResource, RegisteredComponentsResource, RemovedEntities},
};
use legion::{
    prelude::{Entity, Resources, Schedule, World},
    world::{HashMapCloneImplResult, HashMapEntityReplacePolicy, Universe},
};
use log::debug;
use net_sync::{state::WorldState, uid::UidAllocator, ComponentData};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

pub struct WorldInstance {
    pub(crate) world: World,
    pub(crate) schedule: Schedule,
}

impl WorldInstance {
    pub fn new(world: World, schedule: Schedule) -> WorldInstance {
        WorldInstance { world, schedule }
    }

    pub fn execute(&mut self, resources: &mut Resources) {
        self.schedule.execute(&mut self.world, resources);
    }
}

pub struct WorldMappingResource {
    pub(crate) replace_mappings: HashMap<Entity, Entity>,
}

impl WorldMappingResource {
    pub fn remote_representative(&self, entity: Entity) -> Option<Entity> {
        self.replace_mappings
            .iter()
            .find(|(remote, main)| **main == entity)
            .map(|val| *val.0)
    }

    pub fn refresh_mappings(&mut self, result_mappings: HashMap<Entity, Entity>) {
        self.replace_mappings
            .extend(result_mappings.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
}

impl Default for WorldMappingResource {
    fn default() -> Self {
        WorldMappingResource {
            replace_mappings: HashMap::new(),
        }
    }
}

pub struct NetworkUniverse {
    pub(crate) universe: Universe,
    pub(crate) main: WorldInstance,
    pub(crate) remote: WorldInstance,
}

impl NetworkUniverse {
    pub fn new(universe: Universe, main: WorldInstance, remote: WorldInstance) -> NetworkUniverse {
        NetworkUniverse {
            universe,
            main,
            remote,
        }
    }

    pub fn merge_into(&mut self, resources: &mut WorldMappingResource) {

    }

    pub fn create_world(&self) -> World {
        self.universe.create_world()
    }

    pub fn remote_world(&self) -> &World {
        &self.remote.world
    }

    pub fn main_world(&self) -> &World {
        &self.main.world
    }

    pub fn remote_world_mut(&mut self) -> &mut World {
        &mut self.remote.world
    }

    pub fn main_world_mut(&mut self) -> &mut World {
        &mut self.main.world
    }
}

impl Deref for NetworkUniverse {
    type Target = Universe;

    fn deref(&self) -> &Self::Target {
        &self.universe
    }
}

impl DerefMut for NetworkUniverse {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.universe
    }
}

#[cfg(test)]
pub mod test {
    pub use crate as legion_sync;
    use crate::filters::filter_fns::registered;
    use crate::universe::network::WorldMappingResource;
    use crate::{
        components::UidComponent,
        resources::{EventResource, RegisteredComponentsResource, RemovedEntities},
        tracking::*,
        universe::network::{NetworkUniverse, WorldInstance},
    };
    use legion::{
        borrow::Ref,
        prelude::{Entity, Resources, Schedule, Universe},
        query::{IntoQuery, Read},
    };
    use net_sync::{state::WorldState, uid::UidAllocator};

    #[sync]
    #[derive(Debug)]
    pub struct Position {
        pub x: u16,
        pub y: u16,
    }

    impl Default for Position {
        fn default() -> Self {
            Position { x: 5, y: 4 }
        }
    }

    #[test]
    fn merge_remote_with_local_world() {
        let universe = Universe::new();
        let local = universe.create_world();
        let remote = universe.create_world();

        let mut universe = NetworkUniverse::new(
            universe,
            WorldInstance::new(local, Schedule::builder().build()),
            WorldInstance::new(remote, Schedule::builder().build()),
        );
        let query = <Read<Position>>::query();

        let mut resources = Resources::default();
        resources.insert(RemovedEntities::new());
        resources.insert(UidAllocator::<Entity>::new());
        resources.insert(RegisteredComponentsResource::new());
        resources.insert(EventResource::new(universe.main_world_mut(), registered()));

        // insert into remote world
        let entities = universe
            .remote_world_mut()
            .insert(
                (),
                vec![
                    (Position { x: 1, y: 1 }, UidComponent::new(1)),
                    (Position { x: 1, y: 1 }, UidComponent::new(2)),
                    (Position { x: 1, y: 1 }, UidComponent::new(3)),
                ],
            )
            .to_vec();

        let mut event_resource = resources.get_mut::<EventResource>().unwrap();
        let rx = event_resource.legion_receiver();

        {
            let mut allocator = resources.get_mut::<UidAllocator<Entity>>().unwrap();
            allocator.allocate(entities[0], Some(1));
            allocator.allocate(entities[1], Some(2));
            allocator.allocate(entities[2], Some(3));
        }

        // assert local world
        for _ in query.iter(universe.main_world()) {
            panic!("should not contain entities.")
        }

        let mut world_mapping = WorldMappingResource::default();

        let mut world_state = WorldState::new();

        universe.merge_into(&mut world_mapping);

        while let Ok(event) = rx.try_recv() {
            println!("merge 1 {:?}", event);
        }

        {
            let mut pos = universe
                .remote_world_mut()
                .get_component_mut::<Position>(entities[0])
                .unwrap();
            pos.x = 22;
        }
        //
        //        {
        //            universe.remote_world_mut().delete(entities[1]);
        //            let mut removed = resources.get_mut::<RemovedEntities>().unwrap();
        //            removed.add(entities[1]);
        //        }

        {
            universe.merge_into(&mut world_mapping);
        }

        while let Ok(event) = rx.try_recv() {
            println!("merge 2 {:?}", event);
        }

        println!("state: {:?}", world_state);

        // re-assert local world should contain merged entity.
        assert_eq!(
            query
                .iter(universe.main_world())
                .collect::<Vec<Ref<Position>>>()
                .len(),
            1
        );
    }
}
