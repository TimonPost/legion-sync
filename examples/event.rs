use legion::prelude::{
    any, CommandBuffer, Entity, Event, Resources, Schedulable, Schedule, SystemBuilder, Universe,
    World,
};
use legion_sync::{components::UidComponent, tracking::re_exports::crossbeam_channel::*};
use net_sync::uid::{Uid, UidAllocator};
use std::{
    collections::{HashMap, HashSet},
    thread,
    time::Duration,
};

use legion::{index::ArchetypeIndex, storage::ArchetypeId, world::WorldId};
use legion_sync::register::HashmapRegistery;

fn main() {}