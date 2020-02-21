use legion::{prelude::Event, systems::SubWorld};

use track::{
    re_exports::crossbeam_channel::{unbounded, Receiver, Sender, TryIter},
    ModificationChannel, ModificationEvent,
};

use crate::{components::UidComponent, resources::SentBufferResource};
use legion::{
    filter::EntityFilter,
    prelude::{any, Entity, World},
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

    pub fn gather_events(&self, transport: &mut SentBufferResource, world: &mut SubWorld) {
        for event in self.legion_events() {
            match event {
                Event::EntityInserted(entity, chunk_id) => {
                    let identifier = get_identifier_component(world, entity);
                    transport.send_immediate(identifier, crate::event::Event::Inserted(vec![]));
                }
                Event::EntityRemoved(entity, _) => {
                    let identifier = get_identifier_component(world, entity);

                    transport.send_immediate(identifier, crate::event::Event::Removed);
                }
                _ => {}
            }
        }

        for event in self.changed_components() {
            transport.send(
                event.identifier,
                crate::event::Event::Modified(event.modified_fields),
            );
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
