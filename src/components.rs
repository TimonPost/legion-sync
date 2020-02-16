use std::ops::{Deref, DerefMut};

use track::preclude::Uuid;

pub struct UuidComponent {
    uuid: Uuid,
}

impl UuidComponent {
    pub fn uuid(&self) -> Uuid {
        self.uuid.clone()
    }
}

impl Default for UuidComponent {
    fn default() -> Self {
        UuidComponent {
            uuid: Uuid::new_v4(),
        }
    }
}

impl From<Uuid> for UuidComponent {
    fn from(uuid: Uuid) -> Self {
        UuidComponent { uuid }
    }
}

impl Deref for UuidComponent {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.uuid
    }
}

impl DerefMut for UuidComponent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.uuid
    }
}
