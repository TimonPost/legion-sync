use legion::{prelude::Event, systems::SubWorld};

use track::{
    re_exports::crossbeam_channel::{unbounded, Receiver, Sender, TryIter},
    ModificationChannel, ModificationEvent,
};

use crate::{components::UuidComponent, resources::SentBufferResource};

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
                    let uuid_component = world.get_component::<UuidComponent>(entity).unwrap();

                    transport.send_immediate(
                        uuid_component.uuid(),
                        crate::event::Event::Inserted(vec![]),
                    );
                }
                Event::EntityRemoved(entity, _) => {
                    let uuid_component = world.get_component::<UuidComponent>(entity).unwrap();

                    transport.send_immediate(uuid_component.uuid(), crate::event::Event::Removed);
                }
                _ => {}
            }
        }

        for event in self.changed_components() {
            transport.send(
                event.identifier.unwrap(),
                crate::event::Event::Modified(event.modified_fields),
            );
        }
    }
}
