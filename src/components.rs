//! A number of components that can be used to synchronize and trace components.

use net_sync::uid::Uid;
use std::ops::{Deref, DerefMut};

/// A component with a random `UUID`.
///
/// If modifications are serialized we need to know from which component they came.
/// With this component you can identify your entity.
pub struct UidComponent {
    uid: Uid,
}

impl UidComponent {
    pub fn new(uid: Uid) -> UidComponent {
        UidComponent { uid }
    }

    /// Returns the Uuid of this component.
    pub fn uid(&self) -> Uid {
        self.uid.clone()
    }
}

impl Deref for UidComponent {
    type Target = Uid;

    fn deref(&self) -> &Self::Target {
        &self.uid
    }
}

impl DerefMut for UidComponent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.uid
    }
}
