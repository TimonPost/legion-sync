use crate::{Event, UrgencyRequirement};
use net_sync::uid::Uid;
use serde::{Deserialize, Serialize};

/// Structure used to hold message payloads before they are consumed and sent by an underlying
/// NetworkSystem.
#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// The identifier that identifies the entity to which this change belongs.
    identifier: Uid,
    /// The event that defines what kind of packet this is.
    event: Event,
    /// The requirement around when this message should be sent.
    urgency: UrgencyRequirement,
}

impl Message {
    /// Creates and returns a new Message.
    pub(crate) fn new(identifier: Uid, event: Event, urgency: UrgencyRequirement) -> Self {
        Self {
            identifier,
            event,
            urgency,
        }
    }

    pub fn identifier(&self) -> Uid {
        self.identifier
    }

    pub fn event(self) -> Event {
        self.event
    }

    pub fn event_ref(&self) -> &Event {
        &self.event
    }

    pub fn urgency(&self) -> UrgencyRequirement {
        self.urgency
    }
}

#[cfg(test)]
pub mod test {
    use crate::{Event, Message, UrgencyRequirement};
    use net_sync::uid::Uid;

    #[test]
    fn create_message_test() {
        let id = Uid(0);
        let event = Event::Removed;
        let requirement = UrgencyRequirement::Immediate;

        let message = Message::new(id, event.clone(), requirement.clone());
        assert_eq!(message.identifier(), id);
        assert_eq!(message.event_ref(), &event);
        assert_eq!(message.urgency(), requirement);
    }
}
