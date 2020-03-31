use legion::{prelude::Event, systems::SubWorld};

use crate::filters::TrackResourceFilter;
use crate::resources::TrackResource;
use crate::universe::network::WorldMappingResource;
use crate::{
    components::UidComponent,
    event::{LegionEvent, LegionEventHandler},
    filters::filter_fns::registered,
    resources::RegisteredComponentsResource,
};
use legion::filter::{
    ArchetypeFilterData, ChunkFilterData, ChunksetFilterData, EntityFilterTuple, Filter,
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
use std::any::{Any, TypeId};
use track::{
    re_exports::crossbeam_channel::{unbounded, Receiver, Sender, TryIter},
    ModificationChannel, ModificationEvent,
};

pub struct EventResource {
    pub(crate) modification_channel: ModificationChannel<Uid>,
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
}
