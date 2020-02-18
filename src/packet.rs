use std::fmt::Debug;
use track::preclude::Uuid;

use crate::event::Event;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

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

/// Structure used to hold message payloads before they are consumed and sent by an underlying
/// NetworkSystem.
#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// The identifier that identifies the entity to which this change belongs.
    pub uuid: Uuid,
    /// The event that defines what kind of packet this is.
    pub event: Event,
    /// The requirement around when this message should be sent.
    pub urgency: UrgencyRequirement,
}

impl Message {
    /// Creates and returns a new Message.
    pub(crate) fn new(uuid: Uuid, event: Event, urgency: UrgencyRequirement) -> Self {
        Self {
            uuid,
            event,
            urgency,
        }
    }
}

/// Structure used to hold message payloads before they are consumed and sent by an underlying
/// NetworkSystem.
pub struct ReceivedPacket {
    identifier: Uuid,
    addr: SocketAddr,
    event: Event,
}

impl ReceivedPacket {
    pub fn new(addr: SocketAddr, packet: NetworkPacket) -> Self {
        ReceivedPacket {
            event: packet.event,
            identifier: packet.uuid,
            addr,
        }
    }

    pub fn identifier(&self) -> &Uuid {
        &self.identifier
    }

    pub fn source(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn event(&self) -> Event {
        self.event.clone()
    }
}

/// The network packet sent over the network
#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPacket {
    /// The identifier that identifies the entity to which this change belongs.
    pub uuid: Uuid,
    /// The event that defines what kind of packet this is.
    pub event: Event,
}

impl NetworkPacket {
    pub fn new(uuid: Uuid, event: Event) -> NetworkPacket {
        NetworkPacket { uuid, event }
    }
}
