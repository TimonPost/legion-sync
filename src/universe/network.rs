use legion::{
    prelude::{Resources, Schedule, World},
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

#[cfg(test)]
pub mod test {
    use legion::{
        borrow::Ref,
        prelude::{Entity, Resources, Schedule, Universe},
        query::{IntoQuery, Read},
    };

    use net_sync::{state::WorldState, uid::UidAllocator};

    pub use crate as legion_sync;
    use crate::filters::filter_fns::registered;
    use crate::universe::network::WorldMappingResource;
    use crate::{
        components::UidComponent,
        resources::{EventResource, RegisteredComponentsResource, RemovedEntities},
        tracking::*,
        universe::network::{NetworkUniverse, WorldInstance},
    };

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
