use crate::transport::ComponentRecord;
use serde::{Deserialize, Serialize};
use net_sync::uid::Uid;

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    EntityInserted(Uid, Vec<ComponentRecord>),
    EntityRemoved(Uid),

    ComponentModified(Uid, ComponentRecord),
    ComponentRemoved(Uid),
    ComponentAdd(Uid, ComponentRecord)
}
