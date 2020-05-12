//! A number of components that can be used to synchronize and trace components.

use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};

use net_sync::uid::Uid;

use crate::tracking::{serde_diff, Bincode, SerdeDiff};

/// A component with a random `UUID`.
///
/// If modifications are serialized we need to know from which component they came.
/// With this component you can identify your entity.
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Serialize, Deserialize, SerdeDiff)]
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

impl Default for UidComponent {
    fn default() -> Self {
        UidComponent { uid: 0 }
    }
}

crate::register_component_type!(UidComponent, Bincode);
