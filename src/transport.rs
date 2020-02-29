use std::fmt::Debug;

use serde::{Deserialize, Serialize};

mod message;
mod packet;

pub use message::Message;
pub use packet::{ReceivedPacket, SentPacket};

/// Specification of urgency of the sending of a message. Typically we'll want to send messages
/// on simulation tick but the option to send messages immediately is available.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub enum UrgencyRequirement {
    /// Message will be sent based on the current configuration of the simulation frame rate and
    /// the message send rate.
    OnTick,
    /// Message will be sent as soon as possible.
    Immediate,
}

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentRecord {
    register_id: u32,
    data: Vec<u8>,
}

impl ComponentRecord {
    pub(crate) fn new(register_id: u32, data: Vec<u8>) -> Self {
        ComponentRecord { register_id, data }
    }

    pub fn register_id(&self) -> u32 {
        self.register_id
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
pub mod test {}
