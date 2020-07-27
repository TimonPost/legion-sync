use crossbeam_channel::{unbounded, Receiver, Sender, TryIter};
use legion::{World, passthrough};
use legion::world::Event;
use legion::query::Passthrough;

pub struct EventResource {
    pub(crate) legion_events_tx: Sender<Event>,
    pub(crate) legion_events_rx: Receiver<Event>,
}

impl EventResource {
    pub fn new(
        world: &mut World,
    ) -> EventResource
    {
        let (tx, rx) = unbounded();

        world.subscribe(tx.clone(), passthrough());

        EventResource {
            legion_events_tx: tx,
            legion_events_rx: rx,
        }
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

    pub fn subscribe_to_world(
        &self,
        world: &mut World,
    ) {
        world.subscribe(self.legion_subscriber().clone(), passthrough());
    }
}
