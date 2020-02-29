use crate::Event;
use net_sync::uid::Uid;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentPacket {
    /// The identifier that identifies the entity to which this change belongs.
    identifier: Uid,
    /// The event that defines what kind of packet this is.
    event: Event,
}

impl SentPacket {
    pub(crate) fn new(identifier: Uid, event: Event) -> SentPacket {
        SentPacket { identifier, event }
    }

    pub fn identifier(&self) -> &Uid {
        &self.identifier
    }

    pub fn event(&self) -> &Event {
        &self.event
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReceivedPacket {
    identifier: Uid,
    addr: SocketAddr,
    event: Event,
}

impl ReceivedPacket {
    pub fn new(addr: SocketAddr, packet: SentPacket) -> Self {
        ReceivedPacket {
            event: packet.event,
            identifier: packet.identifier,
            addr,
        }
    }

    pub fn identifier(&self) -> Uid {
        self.identifier
    }

    pub fn source(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn event(&self) -> Event {
        self.event.clone()
    }
}

#[cfg(test)]
pub mod test {
    use crate::{Event, SentPacket};
    use net_sync::uid::Uid;

    #[test]
    fn create_sent_packet_test() {
        let id = Uid(0);
        let event = Event::Removed;

        let packet = SentPacket::new(id, event.clone());
        assert_eq!(*packet.identifier(), id);
        assert_eq!(packet.event(), &event);
    }
}
