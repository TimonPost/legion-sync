use crate::resources::RemovedEntities;
use legion::{
    prelude::{Entity, Resources, Schedule, World},
    world::{HashMapCloneImplResult, HashMapEntityReplacePolicy, Universe},
};
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

pub struct NetworkUniverse {
    pub(crate) universe: Universe,
    pub(crate) replace_mappings: HashMap<Entity, Entity>,
    pub(crate) result_mappings: HashMap<Entity, Entity>,
    pub(crate) main: WorldInstance,
    pub(crate) remote: WorldInstance,
}

impl NetworkUniverse {
    pub fn new(universe: Universe, main: WorldInstance, remote: WorldInstance) -> NetworkUniverse {
        NetworkUniverse {
            universe,
            replace_mappings: HashMap::new(),
            result_mappings: HashMap::new(),
            main,
            remote,
        }
    }

    pub fn merge_into(&mut self, resources: &Resources) {
        let removed_entities = resources.get_mut::<RemovedEntities>().unwrap();

        for to_remove in removed_entities.iter() {
            let removed = self
                .replace_mappings
                .remove(&to_remove)
                .expect("Tried to remove entity while it didn't exist.");
            self.result_mappings
                .remove(&to_remove)
                .expect("Tried to remove entity while it didn't exist.");

            self.main.world.delete(removed);
        }

        self.main.world.clone_from(
            &self.remote.world,
            &crate::create_copy_clone_impl(),
            &mut HashMapCloneImplResult(&mut self.result_mappings),
            &HashMapEntityReplacePolicy(&self.replace_mappings),
        );

        self.replace_mappings.extend(
            self.result_mappings
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
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
    use crate::{
        components::UidComponent, network_universe::NetworkUniverse, resources::RemovedEntities,
        tracking::*,
    };
    use legion::{
        borrow::Ref,
        prelude::CommandBuffer,
        query::{IntoQuery, Read},
    };
    use net_sync::uid::UidAllocator;

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
        let mut universe = NetworkUniverse::new();
        let mut local = universe.create_world();
        let mut remote = universe.create_world();

        let query = <Read<Position>>::query();

        // insert into remote world
        remote
            .insert(
                (),
                vec![(
                    Position { x: 1, y: 1 },
                    UidComponent::new(UidAllocator::new().allocate(1, Some(1))),
                )],
            )
            .to_vec();

        // assert local world
        for entry in query.iter(&local) {
            panic!("should not contain entities.")
        }

        universe.merge_into(&mut local, &remote, &RemovedEntities::new());
        universe.merge_into(&mut local, &remote, &RemovedEntities::new());

        // re-assert local world should contain merged entity.
        assert_eq!(
            query.iter(&local).collect::<Vec<(Ref<Position>)>>().len(),
            1
        );
    }

    #[test]
    fn a() {
        let mut universe = NetworkUniverse::new();
        let mut local = universe.create_world();
        let mut remote = universe.create_world();

        let query = <Read<Position>>::query();

        // insert into remote world
        let entities = remote
            .insert(
                (),
                vec![(
                    Position { x: 1, y: 1 },
                    UidComponent::new(UidAllocator::new().allocate(1, Some(1))),
                )],
            )
            .to_vec();

        universe.merge_into(&mut local, &remote, &RemovedEntities::new());

        // re-assert local world should contain merged entity.
        assert_eq!(
            query.iter(&local).collect::<Vec<(Ref<Position>)>>().len(),
            1
        );

        // let's remove a entity.
        let mut remove_resource = RemovedEntities::new();
        remove_resource.add(entities[0]);

        assert!(remote.delete(entities[0]));

        universe.merge_into(&mut local, &remote, &remove_resource);

        assert_eq!(
            query.iter(&local).collect::<Vec<(Ref<Position>)>>().len(),
            0
        );
        assert_eq!(
            query.iter(&remote).collect::<Vec<(Ref<Position>)>>().len(),
            0
        );
    }
}
