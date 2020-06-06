use crossbeam_channel::{unbounded, Receiver, Sender, TryIter};
use legion::{
    filter::{
        ArchetypeFilterData, ChunkFilterData, ChunksetFilterData, EntityFilter, EntityFilterTuple,
        Filter,
    },
    prelude::{Event, World},
};

pub struct EventResource {
    pub(crate) legion_events_tx: Sender<Event>,
    pub(crate) legion_events_rx: Receiver<Event>,
}

impl EventResource {
    pub fn new<A, B, C>(
        world: &mut World,
        event_filter: EntityFilterTuple<A, B, C>,
    ) -> EventResource
    where
        A: for<'a> Filter<ArchetypeFilterData<'a>> + Clone + 'static,
        B: for<'a> Filter<ChunksetFilterData<'a>> + Clone + 'static,
        C: for<'a> Filter<ChunkFilterData<'a>> + Clone + 'static,
    {
        let (tx, rx) = unbounded();

        world.subscribe(tx.clone(), event_filter);

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

    pub fn subscribe_to_world<F: EntityFilter + Sync + 'static>(
        &self,
        world: &mut World,
        filter: F,
    ) {
        world.subscribe(self.legion_subscriber().clone(), filter);
    }
}
