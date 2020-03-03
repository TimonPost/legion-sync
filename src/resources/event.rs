use legion::{prelude::Event, systems::SubWorld};

use track::{
    re_exports::crossbeam_channel::{unbounded, Receiver, Sender, TryIter},
    ModificationChannel, ModificationEvent,
};

use crate::{
    components::UidComponent,
    resources::{RegisteredComponentsResource, SentBufferResource},
    transport::ComponentRecord,
};
use legion::{
    filter::EntityFilter,
    prelude::{Entity, World},
};
use net_sync::uid::Uid;

pub struct EventResource {
    modification_channel: ModificationChannel<Uid>,
    legion_events_tx: Sender<Event>,
    legion_events_rx: Receiver<Event>,
}

impl EventResource {
    pub fn new() -> EventResource {
        let (tx, rx) = unbounded();

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
        world: &mut SubWorld,
    ) {
        for legion_event in self.legion_events() {
            match legion_event {
                Event::EntityInserted(entity, _chunk_id) => {
                    let mut serialized_components: Vec<ComponentRecord> = Vec::new();

                    for component in components.slice_with_uid().iter() {
                        if let Some(data) =
                            component.1.serialize_if_in_entity(world, entity).unwrap()
                        {
                            let record = ComponentRecord::new(component.0.id(), data);
                            serialized_components.push(record);
                        }
                    }

                    // TODO: the same identifier is also in the `serialized_components`.
                    let identifier = get_identifier_component(world, entity);
                    transport.send_immediate(crate::event::Event::EntityInserted(
                        identifier,
                        serialized_components,
                    ));
                }
                Event::EntityRemoved(entity, _) => {
                    let identifier = get_identifier_component(world, entity);

                    transport.send_immediate(crate::event::Event::EntityRemoved(identifier));
                }
                _ => { /*modified events are handled below */ }
            }
        }

        for modified in self.changed_components() {
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

fn get_identifier_component(world: &SubWorld, entity: Entity) -> Uid {
    world
        .get_component::<UidComponent>(entity)
        .expect(
            "Could not find `UuidComponent`. \
               This component is needed for tracking purposes. \
               Make sure to add it to the entity which you are trying to track.",
        )
        .uid()
}
