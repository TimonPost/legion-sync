use legion::{prelude::Event, systems::SubWorld};

use track::{
    re_exports::crossbeam_channel::{unbounded, Receiver, Sender, TryIter},
    ModificationChannel, ModificationEvent,
};

use crate::{components::UuidComponent, resources::SentBufferResource};
use legion::prelude::Entity;
use track::preclude::Uuid;

pub struct EventListenerResource {
    modification_channel: ModificationChannel,
    legion_events_tx: Sender<Event>,
    legion_events_rx: Receiver<Event>,
}

impl EventListenerResource {
    pub fn new() -> EventListenerResource {
        let (tx, rx) = unbounded();

        EventListenerResource {
            legion_events_tx: tx,
            legion_events_rx: rx,
            modification_channel: ModificationChannel::new(),
        }
    }

    fn changed_components(&self) -> TryIter<ModificationEvent> {
        self.modification_channel.receiver().try_iter()
    }

    fn legion_events(&self) -> TryIter<Event> {
        self.legion_events_rx.try_iter()
    }

    pub fn legion_subscriber(&self) -> &Sender<Event> {
        &self.legion_events_tx
    }

    pub fn notifier(&self) -> &Sender<ModificationEvent> {
        &self.modification_channel.sender()
    }

    pub fn gather_events(&self, transport: &mut SentBufferResource, world: &mut SubWorld) {
        for event in self.legion_events() {
            match event {
                Event::EntityInserted(entity, _) => {
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
                event
                    .identifier
                    .expect("Event should always contain identifier."),
                crate::event::Event::Modified(event.modified_fields),
            );
        }
    }
}

fn get_identifier_component(world: &SubWorld, entity: Entity) -> Uuid {
    world
        .get_component::<UuidComponent>(entity)
        .expect(
            "Could not find `UuidComponent`. \
               This component is needed for tracking purposes. \
               Make sure to add it to the entity which you are trying to track.",
        )
        .uuid()
}
