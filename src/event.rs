use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Inserted(Vec<u8>),
    Modified(Vec<u8>),
    Removed,
}
