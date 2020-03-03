use crate::transport::ComponentRecord;
use net_sync::uid::Uid;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    EntityInserted(Uid, Vec<ComponentRecord>),
    EntityRemoved(Uid),

    ComponentModified(Uid, ComponentRecord),
    ComponentRemoved(Uid),
    ComponentAdd(Uid, ComponentRecord),
}
