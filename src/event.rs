use crate::transport::ComponentRecord;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Inserted(Vec<ComponentRecord>),
    Modified(Vec<u8>),
    Removed,
}
