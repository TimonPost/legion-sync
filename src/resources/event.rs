use legion::{prelude::Event, systems::SubWorld};

use crate::{
    event::{LegionEvent, LegionEventHandler},
    filters::filter_fns::registered,
    resources::{RegisteredComponentsResource, SentBufferResource},
    transport::ComponentRecord,
};
use legion::{
    filter::EntityFilter,
    prelude::{Entity, World},
};
use log::debug;
use net_sync::uid::{Uid, UidAllocator};
use track::{
    re_exports::crossbeam_channel::{unbounded, Receiver, Sender, TryIter},
    ModificationChannel, ModificationEvent,
};

pub struct EventResource {
    modification_channel: ModificationChannel<Uid>,
    legion_events_tx: Sender<Event>,
    legion_events_rx: Receiver<Event>,
}

impl EventResource {
    pub fn new(world: &mut World) -> EventResource {
        let (tx, rx) = unbounded();

        world.subscribe(tx.clone(), registered());

        EventResource {
            legion_events_tx: tx,
            legion_events_rx: rx,
            modification_channel: ModificationChannel::new(),
        }
    }

    fn changed_components(&self) -> TryIter<ModificationEvent<Uid>> {
        self.modification_channel.receiver().try_iter()
    }

    fn legion_events(&self) -> TryIter<Event> {
        self.legion_events_rx.try_iter()
    }

    pub fn legion_subscriber(&self) -> &Sender<Event> {
        &self.legion_events_tx
    }

    pub fn notifier(&self) -> &Sender<ModificationEvent<Uid>> {
        &self.modification_channel.sender()
    }

    pub fn subscribe_to_world<F: EntityFilter + Sync + 'static>(
        &self,
        world: &mut World,
        filter: F,
    ) {
        world.subscribe(self.legion_subscriber().clone(), filter);
    }

    pub fn gather_events(
        &self,
        transport: &mut SentBufferResource,
        components: &RegisteredComponentsResource,
        uid_allocator: &mut UidAllocator<Entity>,
        world: &mut SubWorld,
    ) {
        let mut event_handler = LegionEventHandler::new();
        let events = event_handler.handle(&self.legion_events_rx, world, components);

        for legion_event in events {
            match legion_event {
                LegionEvent::EntityInserted(entity, component_count) => {
                    debug!("Inserted {:?} with {} components", entity, component_count);

                    let identifier = uid_allocator.get(&entity);

                    let mut serialized_components: Vec<ComponentRecord> = Vec::new();

                    for component in components.slice_with_uid().iter() {
                        if let Some(data) =
                            component.1.serialize_if_in_entity(world, entity).unwrap()
                        {
                            let record = ComponentRecord::new(component.0.id(), data);
                            serialized_components.push(record);
                        }
                    }

                    transport.send_immediate(crate::event::Event::EntityInserted(
                        identifier,
                        serialized_components,
                    ));
                }
                LegionEvent::EntityRemoved(entity) => {
                    debug!("Removed {:?}", entity);

                    let identifier = uid_allocator
                        .deallocate(entity)
                        .expect("Entity should be allocated.");
                    transport.send_immediate(crate::event::Event::EntityRemoved(Uid(identifier)));
                }
                LegionEvent::ComponentAdded(entity, component_count) => {
                    let identifier = uid_allocator.get(&entity);

                    transport.send_immediate(crate::event::Event::ComponentAdd(
                        identifier,
                        ComponentRecord::new(0, vec![]),
                    ));
                    debug!(
                        "Add component to entity {:?}; component count: {}",
                        entity, component_count
                    );
                }
                LegionEvent::ComponentRemoved(entity, component_count) => {
                    let identifier = uid_allocator.get(&entity);
                    transport.send_immediate(crate::event::Event::ComponentRemoved(identifier));
                    debug!(
                        "Remove component from entity {:?}; component count: {}.",
                        entity, component_count
                    );
                }
            }
        }

        for modified in self.changed_components() {
            debug!("Modified {:?}", modified);

            let uid = components
                .get_uid(&modified.type_id)
                .expect("Type is not registered. Make sure to apply the `sync` attribute.");

            transport.send(crate::event::Event::ComponentModified(
                modified.identifier,
                ComponentRecord::new(uid.0, modified.modified_fields),
            ));
        }
    }
}
