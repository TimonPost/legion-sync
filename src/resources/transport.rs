use std::collections::VecDeque;

use crate::{Message, UrgencyRequirement};

/// Resource serving as the owner of the queue of messages to be sent. This resource also serves
/// as the interface for other systems to send messages.
pub struct TransportResource {
    messages: VecDeque<Message>,
    frame_budget_bytes: i32,
}

impl TransportResource {
    /// Creates a new `TransportResource`.
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            frame_budget_bytes: 0,
        }
    }

    /// Returns estimated number of bytes you can reliably send this frame.
    pub fn frame_budget_bytes(&self) -> i32 {
        self.frame_budget_bytes
    }

    /// Sets the frame budget in bytes. This should be called by a transport implementation.
    pub fn set_frame_budget_bytes(&mut self, budget: i32) {
        self.frame_budget_bytes = budget;
    }

    /// Creates a `Message` with the default guarantees provided by the `Socket` implementation and
    /// pushes it onto the messages queue to be sent on next sim tick.
    pub fn send(&mut self, payload: &[u8]) {
        self.messages
            .push_back(Message::new(payload, UrgencyRequirement::OnTick));
    }

    /// Creates a `Message` with the default guarantees provided by the `Socket` implementation and
    /// Pushes it onto the messages queue to be sent immediately.
    pub fn send_immediate(&mut self, payload: &[u8]) {
        self.messages
            .push_back(Message::new(payload, UrgencyRequirement::Immediate));
    }

    /// Returns true if there are messages enqueued to be sent.
    pub fn has_messages(&self) -> bool {
        !self.messages.is_empty()
    }

    /// Returns a reference to the owned messages.
    pub fn get_messages(&self) -> &VecDeque<Message> {
        &self.messages
    }

    /// Returns the messages to send by returning the immediate messages or anything adhering to
    /// the given filter.
    pub fn drain_messages_to_send(
        &mut self,
        mut filter: impl FnMut(&mut Message) -> bool,
    ) -> Vec<Message> {
        self.drain_messages(|message| {
            message.urgency == UrgencyRequirement::Immediate || filter(message)
        })
    }

    /// Drains the messages queue and returns the drained messages. The filter allows you to drain
    /// only messages that adhere to your filter. This might be useful in a scenario like draining
    /// messages with a particular urgency requirement.
    pub fn drain_messages(&mut self, mut filter: impl FnMut(&mut Message) -> bool) -> Vec<Message> {
        let mut drained = Vec::with_capacity(self.messages.len());
        let mut i = 0;
        while i != self.messages.len() {
            if filter(&mut self.messages[i]) {
                if let Some(m) = self.messages.remove(i) {
                    drained.push(m);
                }
            } else {
                i += 1;
            }
        }
        drained
    }
}

impl Default for TransportResource {
    fn default() -> Self {
        Self {
            messages: VecDeque::new(),
            frame_budget_bytes: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_with_default_requirements() {
        let mut resource = create_test_resource();

        resource.send(test_payload());

        let packet = &resource.messages[0];

        assert_eq!(resource.messages.len(), 1);
        assert_eq!(packet.urgency, UrgencyRequirement::OnTick);
    }

    #[test]
    fn test_send_immediate_message() {
        let mut resource = create_test_resource();

        resource.send_immediate(test_payload());

        let packet = &resource.messages[0];

        assert_eq!(resource.messages.len(), 1);
        assert_eq!(packet.urgency, UrgencyRequirement::Immediate);
    }

    #[test]
    fn test_has_messages() {
        let mut resource = create_test_resource();
        assert_eq!(resource.has_messages(), false);
        resource.send_immediate(test_payload());
        assert_eq!(resource.has_messages(), true);
    }

    #[test]
    fn test_drain_only_immediate_messages() {
        let mut resource = create_test_resource();

        let addr = "127.0.0.1:3000".parse().unwrap();
        resource.send_immediate(test_payload());
        resource.send_immediate(test_payload());
        resource.send(test_payload());
        resource.send(test_payload());
        resource.send_immediate(test_payload());

        assert_eq!(resource.drain_messages_to_send(|_| false).len(), 3);
        assert_eq!(resource.drain_messages_to_send(|_| false).len(), 0);
    }

    fn test_payload() -> &'static [u8] {
        b"test"
    }

    fn create_test_resource() -> TransportResource {
        TransportResource::new()
    }
}
