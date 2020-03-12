use crate::Event;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentPacket {
    /// The event that defines what kind of packet this is.
    event: Event,
}

impl SentPacket {
    pub(crate) fn new(event: Event) -> SentPacket {
        SentPacket { event }
    }

    pub fn event(&self) -> &Event {
        &self.event
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReceivedPacket {
    addr: SocketAddr,
    event: Event,
}

impl ReceivedPacket {
    pub fn new(addr: SocketAddr, packet: SentPacket) -> Self {
        ReceivedPacket {
            event: packet.event,
            addr,
        }
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
        let event = Event::EntityRemoved(id);

        let packet = SentPacket::new(event.clone());
        assert_eq!(packet.event(), &event);
    }
}
