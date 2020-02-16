use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialOrd, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Inserted = 1,
    Modified = 2,
    Removed = 4,
}

impl From<u8> for Event {
    fn from(value: u8) -> Self {
        match value {
            1 => Event::Inserted,
            2 => Event::Modified,
            4 => Event::Removed,
            _ => panic!("Value {} cannot be cast to an ChangeEvent.", value),
        }
    }
}
