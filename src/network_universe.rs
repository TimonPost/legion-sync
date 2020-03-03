use legion::{prelude::World, world::Universe};
use std::ops::{Deref, DerefMut};
use std::collections::HashMap;
use legion::prelude::Entity;

pub struct NetworkUniverse {
    universe: Universe,
    replace_mappings: HashMap<Entity, Entity>,
    result_mappings: HashMap<Entity, Entity>
}

impl NetworkUniverse {
    pub fn new() -> NetworkUniverse {
        let universe = Universe::new();
        NetworkUniverse {
            universe,
            replace_mappings: HashMap::new(),
            result_mappings: HashMap::new()
        }
    }

    pub fn merge_into(&mut self, local: &mut World, remote: &World) {
        local.clone_from(
            &remote,
            &crate::create_copy_clone_impl(),
            Some(&self.replace_mappings),
            Some(&mut self.result_mappings),
        );

        self.replace_mappings.extend(self.result_mappings.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    pub fn create_world(&self) -> World {
        self.universe.create_world()
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
    fn merge_remote_with_local_world() {
        let mut universe = NetworkUniverse::new();
        let mut local = universe.create_world();
        let mut remote = universe.create_world();

        let query = <(Read<Position>)>::query();

        // insert into remote world
        remote.insert(
            (),
            vec![(
                Position { x: 1, y: 1 },
                UidComponent::new(UidAllocator::new().allocate(Some(1))),
            )],
        )
        .to_vec();

        // assert local world
        for entry in query.iter(&local) {
            panic!("should not contain entities.")
        }


        universe.merge_into(&mut local, &remote);
        universe.merge_into(&mut local, &remote);

        // re-assert local world should contain merged entity.
        assert_eq!(
            query
                .iter(&local)
                .collect::<Vec<(Ref<Position>)>>()
                .len(),
            1
        );
    }
}
