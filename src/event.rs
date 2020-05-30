use std::{collections::HashMap, fmt::Debug};

use legion::prelude::Entity;
use serde::export::{fmt::Error, Formatter};

use crate::{
    resources::RegisteredComponentsResource, tracking::re_exports::crossbeam_channel::Receiver,
    WorldAbstraction,
};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum LegionEvent {
    ComponentAdded(Entity, usize),
    ComponentRemoved(Entity, usize),
    EntityInserted(Entity, usize),
    EntityRemoved(Entity),
}

impl Debug for LegionEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match &self {
            LegionEvent::ComponentAdded(entity_id, count) => write!(
                f,
                "Component Added to Entity: {}, {} components",
                entity_id, count
            ),
            LegionEvent::ComponentRemoved(entity_id, count) => write!(
                f,
                "Component Removed from Entity: {}, {} components",
                entity_id, count
            ),
            LegionEvent::EntityInserted(entity_id, count) => write!(
                f,
                "Entity Inserted: {} with {} components",
                entity_id, count
            ),
            LegionEvent::EntityRemoved(entity_id) => write!(f, "Entity Removed: {}", entity_id),
        }
    }
}

#[derive(Debug)]
pub struct EntityTracker {
    data: HashMap<Entity, usize>,
}

impl EntityTracker {
    pub fn new() -> EntityTracker {
        EntityTracker {
            data: HashMap::new(),
        }
    }

    pub fn log_entity(&mut self, entity: Entity, component_count: usize) {
        self.data.insert(entity, component_count);
    }
}

#[derive(Debug)]
pub struct EventTracker {
    pub(crate) inserted: EntityTracker,
    pub(crate) removed: EntityTracker,
}

impl EventTracker {
    pub fn new() -> EventTracker {
        EventTracker {
            inserted: EntityTracker::new(),
            removed: EntityTracker::new(),
        }
    }

    pub fn contains_inserted(&self, entity: Entity) -> bool {
        self.inserted.data.contains_key(&entity)
    }

    pub fn contains_removed(&self, entity: Entity) -> bool {
        self.removed.data.contains_key(&entity)
    }

    pub fn previous_component_count(&self, entity: Entity) -> usize {
        *self.inserted.data.get(&entity).unwrap()
    }
}

pub struct LegionEventHandler {
    tracker: EventTracker,
}

impl LegionEventHandler {
    pub fn new() -> LegionEventHandler {
        LegionEventHandler {
            tracker: EventTracker::new(),
        }
    }
}

impl LegionEventHandler {
    /// A legion event sometimes arrives unexpectedly.
    /// When a user performs add/remove component action, we get three events (insert, remove, insert) because there is a re-allocation of an event.
    /// Legion-sync needs to know when a component has been added, removed or an entity has been inserted or removed.
    /// The following code keeps track of what kind of events are in the receiver and assumes the type of event based on input data.
    pub fn handle(
        &mut self,
        receiver: &Receiver<legion::event::Event>,
        world: &dyn WorldAbstraction,
        registered: &RegisteredComponentsResource,
    ) -> Vec<LegionEvent> {
        let events = receiver.try_iter().collect::<Vec<legion::event::Event>>();
        let mut result_events = Vec::with_capacity(events.len());
        let mut iterator = events.into_iter();

        while let Some(event) = iterator.next() {
            match event {
                legion::event::Event::EntityInserted(inserted, _chunk) => {
                    if self.tracker.contains_inserted(inserted)
                        && self.tracker.contains_removed(inserted)
                    {
                        // If we have seen the insert and remove event with this entity before then this insert means an component add or remove.
                        // Remember: component add/remove results in Insert(1) -> Remove(1) -> Insert(1)

                        // In order to know if component add/remove, compare the previous and current counted components.
                        let previous_component_count =
                            self.tracker.previous_component_count(inserted);

                        let new_component_count =
                            LegionEventHandler::count_components(registered, world, inserted);

                        if previous_component_count < new_component_count {
                            // old component has less components, therefore a added component.
                            result_events
                                .push(LegionEvent::ComponentAdded(inserted, new_component_count));
                        } else if previous_component_count > new_component_count {
                            // old component has more components there, therefore a removed component.
                            result_events
                                .push(LegionEvent::ComponentRemoved(inserted, new_component_count));
                        }
                    } else {
                        // Insert and remove haven't been seen before.
                        // This event is either: 1) a stand-alone insert 2) a future component add/remove.

                        let mut events = iterator.clone();

                        // Check if the following events contain an remove with the current entity id.
                        // This would indicate an component add/remove on this inserted entity.
                        let find_result = events.any(|x| match x {
                            legion::event::Event::EntityRemoved(entity, _) => entity == inserted,
                            _ => false,
                        });

                        let components_count =
                            LegionEventHandler::count_components(registered, world, inserted);

                        if find_result {
                            // Remember this entity for next round.
                            self.tracker.inserted.log_entity(inserted, components_count);
                        }

                        result_events.push(LegionEvent::EntityInserted(inserted, components_count))
                    }
                }
                legion::event::Event::EntityRemoved(removed, _chunk_id) => {
                    if !self.tracker.contains_inserted(removed) {
                        // We have not seen an insert with this entity before.
                        // This can't be a re-allocation.
                        result_events.push(LegionEvent::EntityRemoved(removed))
                    } else {
                        // This event is either: 1) a stand-alone removal 2) a future component add/remove.

                        let mut events = iterator.clone();

                        let find_result = events.any(|x| match x {
                            legion::event::Event::EntityInserted(entity, _) => entity == removed,
                            _ => false,
                        });

                        if find_result {
                            // It isn't a standalone removal, but part of reallocation events.
                            self.tracker.removed.log_entity(
                                removed,
                                LegionEventHandler::count_components(registered, world, removed),
                            );
                        } else {
                            // It is a stand-alone removal.
                            result_events.push(LegionEvent::EntityRemoved(removed))
                        }
                    }
                }
                legion::event::Event::ArchetypeCreated(_id) => {}
                legion::event::Event::ChunkCreated(_) => {}
            }
        }

        result_events
    }

    fn count_components(
        registered: &RegisteredComponentsResource,
        world: &dyn WorldAbstraction,
        entity: Entity,
    ) -> usize {
        let mut counter = 0;

        for component in registered.slice_with_uid().iter() {
            if world.has_component(entity, component.1) {
                counter += 1;
            }
        }

        counter
    }
}

#[cfg(test)]
mod tests {
    struct Component;
    //
    //    #[test]
    //    pub fn insert_remove_component() {
    //        let (mut world, mut resources) = initialize_test_world();
    //
    //        let mut schedule = Schedule::builder()
    //            .add_system(insert_remove_component_system())
    //            .build();
    //        schedule.execute(&mut world, &mut resources);
    //
    //        let receiver = resources.get::<Receiver<Event>>().unwrap();
    //
    //        let mut event_handler: LegionEventHandler = LegionEventHandler::new();
    //        let events = event_handler.handle(&receiver);
    //
    //        assert!(match events[0] {
    //            LegionEvent::EntityInserted(_, _) => true,
    //            _ => false,
    //        });
    //        assert!(match events[1] {
    //            LegionEvent::ComponentAdded(_, _) => true,
    //            _ => false,
    //        });
    //    }
    //
    //    #[test]
    //    pub fn insert_add_component() {
    //        let (mut world, mut resources) = initialize_test_world();
    //
    //        let mut schedule = Schedule::builder()
    //            .add_system(insert_add_component_system())
    //            .build();
    //        schedule.execute(&mut world, &mut resources);
    //
    //        let receiver = resources.get::<Receiver<Event>>().unwrap();
    //
    //        let mut event_handler: LegionEventHandler = LegionEventHandler::new();
    //        let events = event_handler.handle(&receiver);
    //
    //        assert!(match events[0] {
    //            LegionEvent::EntityInserted(_, _) => true,
    //            _ => false,
    //        });
    //        assert!(match events[1] {
    //            LegionEvent::ComponentRemoved(_, _) => true,
    //            _ => false,
    //        });
    //    }
    //
    //    #[test]
    //    pub fn insert_entity() {
    //        let (mut world, mut resources) = initialize_test_world();
    //
    //        let mut schedule = Schedule::builder().add_system(insert_system()).build();
    //        schedule.execute(&mut world, &mut resources);
    //
    //        let receiver = resources.get::<Receiver<Event>>().unwrap();
    //
    //        let mut event_handler: LegionEventHandler = LegionEventHandler::new();
    //        let events = event_handler.handle(&receiver);
    //
    //        assert!(match events[0] {
    //            LegionEvent::EntityInserted(_, _) => true,
    //            _ => false,
    //        });
    //    }
    //
    //    #[test]
    //    pub fn remove_entity() {
    //        let (mut world, mut resources) = initialize_test_world();
    //
    //        world.insert((), vec![()]);
    //
    //        let mut schedule = Schedule::builder().add_system(remove_system()).build();
    //        schedule.execute(&mut world, &mut resources);
    //
    //        let receiver = resources.get::<Receiver<Event>>().unwrap();
    //
    //        let mut event_handler: LegionEventHandler = LegionEventHandler::new();
    //        let events = event_handler.handle(&receiver);
    //
    //        assert!(match events[0] {
    //            LegionEvent::EntityInserted(_, _) => true,
    //            _ => false,
    //        }); // because of first insert
    //        assert!(match events[1] {
    //            LegionEvent::EntityRemoved(_) => true,
    //            _ => false,
    //        });
    //    }
    //
    //    #[test]
    //    pub fn some_random_order() {
    //        let (mut world, mut resources) = initialize_test_world();
    //
    //        let mut schedule = Schedule::builder()
    //            .add_system(insert_add_component_system())
    //            .add_system(insert_remove_component_system())
    //            .add_system(insert_system())
    //            .build();
    //
    //        schedule.execute(&mut world, &mut resources);
    //
    //        let receiver = resources.get::<Receiver<Event>>().unwrap();
    //
    //        let mut event_handler: LegionEventHandler = LegionEventHandler::new();
    //        let events = event_handler.handle(SubWorld::from(world), &receiver);
    //
    //        assert!(match events[0] {
    //            LegionEvent::EntityInserted(_, _) => true,
    //            _ => false,
    //        });
    //        assert!(match events[1] {
    //            LegionEvent::ComponentAdded(_, _) => true,
    //            _ => false,
    //        });
    //        assert!(match events[2] {
    //            LegionEvent::EntityInserted(_, _) => true,
    //            _ => false,
    //        });
    //        assert!(match events[3] {
    //            LegionEvent::ComponentRemoved(_, _) => true,
    //            _ => false,
    //        });
    //        assert!(match events[4] {
    //            LegionEvent::EntityInserted(_, _) => true,
    //            _ => false,
    //        });
    //    }
    //
    //    fn initialize_test_world() -> (World, Resources) {
    //        let universe = Universe::new();
    //        let mut world = universe.create_world();
    //
    //        let (tx, rx) = unbounded();
    //        world.subscribe(tx, any());
    //
    //        let mut resources = Resources::default();
    //        resources.insert(rx);
    //
    //        (world, resources)
    //    }
    //
    //    pub fn insert_system() -> Box<dyn Schedulable> {
    //        SystemBuilder::new("read_received_system").build(|mut command_buffer, _, _, _| {
    //            command_buffer
    //                .insert((), vec![(UidComponent::new(0),)])
    //                .to_vec()[0];
    //        })
    //    }
    //
    //    pub fn remove_system() -> Box<dyn Schedulable> {
    //        SystemBuilder::new("read_received_system").build(|mut command_buffer, _, _, _| {
    //            command_buffer.exec_mut(|w| w.delete_all());
    //        })
    //    }
    //
    //    pub fn insert_remove_component_system() -> Box<dyn Schedulable> {
    //        SystemBuilder::new("read_received_system").build(|mut command_buffer, _, _, _| {
    //            let entity = command_buffer
    //                .insert((), vec![(UidComponent::new(0), Component)])
    //                .to_vec()[0];
    //
    //            command_buffer.remove_component::<Component>(entity);
    //        })
    //    }
    //
    //    pub fn insert_add_component_system() -> Box<dyn Schedulable> {
    //        SystemBuilder::new("read_received_system").build(|mut command_buffer, _, _, _| {
    //            let entity = command_buffer
    //                .insert((), vec![(UidComponent::new(0),)])
    //                .to_vec()[0];
    //
    //            command_buffer.add_component(entity, Component);
    //        })
    //    }
}
