use std::{
    any::TypeId,
    collections::{
        hash_map::{self},
        HashMap,
    },
    hash::Hash,
    slice,
    sync::{Arc, Mutex, MutexGuard},
};

use net_sync::uid::Uid;

use crate::register::{ComponentRegister, ComponentRegistrationRef};

// Here we store three instances of registration storage's.
// This is relatively cheap because they store references and allow us to retrieve an registration by key.
pub struct RegisteredComponentsResource {
    type_id_with_uid: HashMap<TypeId, Uid>,
    uid_with_type_id: HashMap<Uid, TypeId>,

    registration_by_uid: Arc<Mutex<HashMap<Uid, ComponentRegistrationRef>>>,
    registration_by_type_id: Arc<Mutex<HashMap<TypeId, ComponentRegistrationRef>>>,
    uid_with_registration: Arc<Mutex<Vec<(Uid, ComponentRegistrationRef)>>>,
}

impl RegisteredComponentsResource {
    pub fn new() -> Self {
        let mut by_uid = HashMap::new();
        let mut by_type_id = HashMap::new();

        let mut uid_with_type_id = HashMap::new();
        let mut type_id_with_uid = HashMap::new();

        let mut sorted_registry = ComponentRegister::by_unique_uid()
            .clone()
            .into_iter()
            .map(|(k, v)| (k, v))
            .collect::<Vec<(Uid, ComponentRegistrationRef)>>();

        sorted_registry.sort_by(|a, b| a.1.ty().partial_cmp(&b.1.ty()).unwrap());

        for entry in sorted_registry.iter() {
            by_uid.insert(entry.0, entry.1);
            by_type_id.insert(entry.1.ty(), entry.1);

            type_id_with_uid.insert(entry.1.ty(), entry.0);
            uid_with_type_id.insert(entry.0, entry.1.ty());
        }

        Self {
            type_id_with_uid,
            uid_with_type_id,

            registration_by_uid: Arc::new(Mutex::new(by_uid)),
            registration_by_type_id: Arc::new(Mutex::new(by_type_id)),
            uid_with_registration: Arc::new(Mutex::new(sorted_registry)),
        }
    }

    pub fn by_uid(&self) -> HashmapRegistry<'_, Uid> {
        HashmapRegistry::new(self.registration_by_uid.lock().unwrap())
    }

    pub fn by_type_id(&self) -> HashmapRegistry<'_, TypeId> {
        HashmapRegistry::new(self.registration_by_type_id.lock().unwrap())
    }

    pub fn slice_with_uid(&self) -> SliceRegistry<'_> {
        SliceRegistry::new(self.uid_with_registration.lock().unwrap())
    }

    pub fn get_type(&self, uid: &Uid) -> Option<&TypeId> {
        self.uid_with_type_id.get(uid)
    }

    pub fn get_uid(&self, type_id: &TypeId) -> Option<&Uid> {
        self.type_id_with_uid.get(type_id)
    }
}

pub struct HashmapRegistry<'a, I>
where
    I: Eq + Hash,
{
    lock: MutexGuard<'a, HashMap<I, ComponentRegistrationRef>>,
}

impl<'a, I> HashmapRegistry<'a, I>
where
    I: Eq + Hash,
{
    pub fn new(
        guard: MutexGuard<'a, HashMap<I, ComponentRegistrationRef>>,
    ) -> HashmapRegistry<'a, I> {
        Self { lock: guard }
    }

    pub fn iter(&self) -> hash_map::Iter<'_, I, ComponentRegistrationRef> {
        self.lock.iter()
    }

    pub fn get(&self, id: &I) -> Option<&ComponentRegistrationRef> {
        self.lock.get(id)
    }
}

pub struct SliceRegistry<'a> {
    lock: MutexGuard<'a, Vec<(Uid, ComponentRegistrationRef)>>,
}

impl<'a> SliceRegistry<'a> {
    pub fn new(guard: MutexGuard<'a, Vec<(Uid, ComponentRegistrationRef)>>) -> SliceRegistry<'a> {
        SliceRegistry { lock: guard }
    }

    pub fn iter(&self) -> slice::Iter<(Uid, ComponentRegistrationRef)> {
        self.lock.iter()
    }
}

#[cfg(test)]
pub mod test {
    use crate::resources::RegisteredComponentsResource;

    #[test]
    fn register_should_have_same_components_test() {
        let registry = RegisteredComponentsResource::new();
        let by_type_id = registry.by_type_id();
        let by_uid = registry.by_uid();
        let slice_with_uid = registry.slice_with_uid();

        for entry in slice_with_uid.iter() {
            assert!(by_type_id.get(&entry.1.ty()).is_some());
            assert!(by_uid.get(&entry.0).is_some());
        }
    }

    #[test]
    fn type_mappings_are_correct_test() {
        let registry = RegisteredComponentsResource::new();

        let slice_with_uid = registry.slice_with_uid();

        for entry in slice_with_uid.iter() {
            assert!(registry.get_uid(&entry.1.ty()).is_some());
            assert!(registry.get_type(&entry.0).is_some());
        }
    }
}
