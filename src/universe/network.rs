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

    pub fn merge_into(&mut self, resources: &Resources, world_state: &mut WorldState) {
        // Setup resources
        let mut allocator = resources.get_mut::<UidAllocator<Entity>>().unwrap();
        let mut removed_entities = resources.get_mut::<RemovedEntities>().unwrap();
        let components = resources.get::<RegisteredComponentsResource>().unwrap();
        let event_resource = resources.get_mut::<EventResource>().unwrap();

        // Handle remove events, and clear mappings to prevent merge of removed entities and delete entity from worlds.
        for to_remove in removed_entities.drain() {
            let removed = self
                .replace_mappings
                .remove(&to_remove)
                .expect("Tried to remove entity while it didn't exist.");

            self.result_mappings
                .remove(&to_remove)
                .expect("Tried to remove entity while it didn't exist.");

            self.main.world.delete(removed);

            let identifier = allocator
                .deallocate(to_remove)
                .expect("Entity should be allocated.");

            world_state.remove_entity(identifier);
        }

        {
            // All (syncable) entities are mapped, retrieve the entity id and get its registration instance.
            // Then compare and serialize the changes from the the remote and main world component.
            let slice = components.slice_with_uid();
            for (remote, main) in self.replace_mappings.iter() {
                for (_id, comp) in slice.iter() {
                    comp.serialize_if_different(
                        &self.main.world,
                        *main,
                        &self.remote.world,
                        *remote,
                        event_resource.notifier(),
                    );
                }
            }
        }

        let clone_impl = crate::create_copy_clone_impl();

        // Clone remote world into main world.
        self.main.world.clone_from(
            &self.remote.world,
            &clone_impl,
            &mut HashMapCloneImplResult(&mut self.result_mappings),
            &HashMapEntityReplacePolicy(&self.replace_mappings),
        );

        self.replace_mappings.extend(
            self.result_mappings
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );

        // Handle the events from above merge operation.
        let mut event_handler = LegionEventHandler::new();

        let events = event_handler.handle(
            &event_resource.legion_receiver(),
            &self.main.world,
            &components,
        );

        for legion_event in events {
            debug!("{:?}", legion_event);
            match legion_event {
                LegionEvent::EntityInserted(entity, _component_count) => {
                    let entity = self
                        .replace_mappings
                        .iter()
                        .find(|(_k, v)| **v == entity)
                        .expect("Entity should be in replace mappings.");

                    let identifier = allocator.get(&entity.0);

                    let mut serialized_components: Vec<ComponentData> = Vec::new();

                    for component in components.slice_with_uid().iter() {
                        if let Some(data) = component
                            .1
                            .serialize_if_in_world(&self.main.world, *entity.1)
                            .unwrap()
                        {
                            let record = ComponentData::new(component.0, data);
                            serialized_components.push(record);
                        }
                    }

                    world_state.insert_entity(identifier, serialized_components);
                }
                LegionEvent::ComponentAdded(entity, _component_count) => {
                    let identifier = allocator.get(&entity);
                    world_state.add_component(identifier, ComponentData::new(0, vec![]))
                }
                LegionEvent::ComponentRemoved(entity, _component_count) => {
                    let identifier = allocator.get(&entity);
                    world_state.remove_component(identifier, 0);
                }
                _ => {}
            }
        }

        for event in event_resource.changed_components() {
            let register_id = components.get_uid(&event.type_id).expect("Should exist");
            world_state.change(
                event.identifier,
                ComponentData::new(*register_id, event.modified_fields),
            );
        }
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
        resources.insert(EventResource::new(universe.main_world_mut()));

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

        let mut world_state = WorldState::new();

        universe.merge_into(&resources, &mut world_state);

        {
            let mut pos = universe
                .remote_world_mut()
                .get_component_mut::<Position>(entities[0])
                .unwrap();
            pos.x = 22;
        }

        {
            universe.remote_world_mut().delete(entities[1]);
            let mut removed = resources.get_mut::<RemovedEntities>().unwrap();
            removed.add(entities[1]);
        }

        {
            universe.merge_into(&resources, &mut world_state);
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
