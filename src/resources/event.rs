use legion::{prelude::Event, systems::SubWorld};

use crate::{
    components::UidComponent,
    event::{LegionEvent, LegionEventHandler},
    filters::filter_fns::registered,
    resources::RegisteredComponentsResource,
};
use legion::{
    filter::EntityFilter,
    prelude::{Entity, World},
};
use log::debug;
use net_sync::{
    transport::PostBox,
    uid::{Uid, UidAllocator},
    ClientMessage, ComponentData, ServerMessage,
};
use std::any::TypeId;
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

    pub fn changed_components(&self) -> TryIter<ModificationEvent<Uid>> {
        self.modification_channel.receiver().try_iter()
    }

    fn legion_events(&self) -> TryIter<Event> {
        self.legion_events_rx.try_iter()
    }

    pub fn legion_subscriber(&self) -> &Sender<Event> {
        &self.legion_events_tx
    }

    pub fn legion_receiver(&self) -> &Receiver<Event> {
        &self.legion_events_rx
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
        transport: &mut PostBox<ServerMessage, ClientMessage>,
        components: &RegisteredComponentsResource,
        uid_allocator: &mut UidAllocator<Entity>,
        world: &mut SubWorld,
    ) {
        let mut event_handler = LegionEventHandler::new();
        let events = event_handler.handle(&self.legion_events_rx, world, components);

        for legion_event in events {
            debug!("{:?}", legion_event);

            match legion_event {
                LegionEvent::EntityInserted(entity, _component_count) => {
                    let identifier = uid_allocator.get(&entity);

                    let mut serialized_components: Vec<ComponentData> = Vec::new();

                    for component in components.slice_with_uid().iter() {
                        // do not sent uid components, as the server will append it's onw.
                        if component.1.ty() == TypeId::of::<UidComponent>() {
                            continue;
                        }

                        if let Some(data) =
                            component.1.serialize_if_in_subworld(world, entity).unwrap()
                        {
                            let record = ComponentData::new(component.0, data);
                            serialized_components.push(record);
                        }
                    }

                    transport.send_immediate(net_sync::ClientMessage::EntityInserted(
                        identifier,
                        serialized_components,
                    ));
                }
                LegionEvent::EntityRemoved(entity) => {
                    let identifier = uid_allocator
                        .deallocate(entity)
                        .expect("Entity should be allocated.");
                    transport.send_immediate(net_sync::ClientMessage::EntityRemoved(identifier));
                }
                LegionEvent::ComponentAdded(entity, _component_count) => {
                    let identifier = uid_allocator.get(&entity);

                    transport.send_immediate(net_sync::ClientMessage::ComponentAdd(
                        identifier,
                        ComponentData::new(0, vec![]),
                    ));
                }
                LegionEvent::ComponentRemoved(entity, _component_count) => {
                    let identifier = uid_allocator.get(&entity);
                    transport.send_immediate(net_sync::ClientMessage::ComponentRemoved(identifier));
                }
            }
        }

        for modified in self.changed_components() {
            debug!("Modified {:?}", modified);

            let uid = components
                .get_uid(&modified.type_id)
                .expect("Type is not registered. Make sure to apply the `sync` attribute.");

            transport.send(net_sync::ClientMessage::ComponentModified(
                modified.identifier,
                ComponentData::new(*uid, modified.modified_fields),
            ));
        }
    }
}
