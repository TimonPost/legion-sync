use legion::{prelude::World, world::Universe};
use std::ops::{Deref, DerefMut};

pub struct NetworkUniverse {
    universe: Universe,
    local_world: World,
    remote_world: World,
}

impl NetworkUniverse {
    pub fn new() -> NetworkUniverse {
        let universe = Universe::new();
        NetworkUniverse {
            local_world: universe.create_world(),
            remote_world: universe.create_world(),
            universe,
        }
    }

    pub fn local_update(&mut self) {
        self.local_world.clone_merge(
            &self.remote_world,
            &crate::create_copy_clone_impl(),
            None,
            None,
        );
    }

    pub fn local(&self) -> &World {
        &self.local_world
    }

    pub fn local_mut(&mut self) -> &mut World {
        &mut self.local_world
    }

    pub fn remote_mut(&mut self) -> &mut World {
        &mut self.remote_world
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
    use crate::{components::UidComponent, network_universe::NetworkUniverse, tracking::*};
    use legion::{
        borrow::Ref,
        query::{IntoQuery, Read},
        world::Universe,
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
    pub fn merge_remote_with_local_world() {
        let mut universe = NetworkUniverse::new();

        let query = <(Read<Position>)>::query();

        // insert into remote world
        universe
            .remote_mut()
            .insert(
                (),
                vec![(
                    Position { x: 1, y: 1 },
                    UidComponent::new(UidAllocator::new().allocate(Some(1))),
                )],
            )
            .to_vec();

        // assert local world
        for entry in query.iter(&universe.local()) {
            panic!("should not contain entities.")
        }

        universe.local_update();

        // re-assert local world should contain merged entity.
        assert_eq!(
            query
                .iter(&universe.local())
                .collect::<Vec<(Ref<Position>)>>()
                .len(),
            1
        );
    }
}
