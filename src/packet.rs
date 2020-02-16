use std::fmt::Debug;
use std::io::{Cursor, Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use track::preclude::Uuid;

use crate::event::Event;

/// Specification of urgency of the sending of a message. Typically we'll want to send messages
/// on simulation tick but the option to send messages immediately is available.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq)]
pub enum UrgencyRequirement {
    /// Message will be sent based on the current configuration of the simulation frame rate and
    /// the message send rate.
    OnTick,
    /// Message will be sent as soon as possible.
    Immediate,
}

/// Structure used to hold message payloads before they are consumed and sent by an underlying
/// NetworkSystem.
#[derive(Debug, PartialEq, Eq)]
pub struct Message {
    /// The serialized  payload itself.
    pub payload: Vec<u8>,
    /// The requirement around when this message should be sent.
    pub urgency: UrgencyRequirement,
}

impl Message {
    /// Creates and returns a new Message.
    pub(crate) fn new(payload: &[u8], urgency: UrgencyRequirement) -> Self {
        Self {
            payload: payload.to_owned(),
            urgency,
        }
    }
}

#[derive(Debug)]
pub struct NetworkPacket {
    pub uuid: Uuid,
    pub event_type: Event,
    pub remaining_data: Option<Vec<u8>>,
}

impl NetworkPacket {
    pub fn has_change_data(&self) {
        self.remaining_data.is_some();
    }
}

pub struct NetworkPacketReader<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> NetworkPacketReader<'a> {
    pub fn new(data: &'a [u8]) -> NetworkPacketReader<'a> {
        NetworkPacketReader {
            cursor: Cursor::new(data),
        }
    }
}

impl<'a> NetworkPacketReader<'a> {
    pub fn read(&mut self) -> Result<NetworkPacket, std::io::Error> {
        let mut packet = NetworkPacket {
            uuid: Uuid::from_u128(self.cursor.read_u128::<BigEndian>()?),
            event_type: Event::from(self.cursor.read_u8()?),
            remaining_data: None,
        };

        if packet.event_type == Event::Modified {
            let mut data = Vec::new();
            self.cursor.read_to_end(&mut data)?;
            packet.remaining_data = Some(data);
        }

        Ok(packet)
    }
}

pub struct NetworkPacketBuilder {
    data: Vec<u8>,
}

impl NetworkPacketBuilder {
    pub fn new(entity: Uuid, event_type: Event) -> NetworkPacketBuilder {
        NetworkPacketBuilder { data: Vec::new() }
            .with_entity(entity)
            .with_event_type(event_type)
    }

    pub fn with_capacity(
        identity: Uuid,
        event_type: Event,
        capacity: usize,
    ) -> NetworkPacketBuilder {
        let mut builder = NetworkPacketBuilder {
            data: Vec::with_capacity(capacity + 9),
        }
        .with_entity(identity)
        .with_event_type(event_type);

        builder
    }

    fn with_entity(mut self, entity: Uuid) -> Self {
        self.data.write_u128::<BigEndian>(entity.as_u128());
        self
    }

    fn with_event_type(mut self, event: Event) -> Self {
        self.data.write_u8(event as u8);
        self
    }

    pub fn with_data(mut self, data: &[u8]) -> Self {
        self.data.write_all(data);
        self
    }

    pub fn build(self) -> Vec<u8> {
        self.data
    }
}
