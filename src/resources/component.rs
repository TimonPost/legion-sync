use crate::register::{ComponentRegister, RegisteredComponent};
use net_sync::uid::Uid;
use std::{
    collections::{
        hash_map::{self},
        HashMap,
    },
    slice,
    sync::{Arc, Mutex, MutexGuard},
};

pub struct RegisteredComponentsResource {
    hashmap_registry: Arc<Mutex<HashMap<Uid, RegisteredComponent>>>,
    vec_registry: Arc<Mutex<Vec<(Uid, RegisteredComponent)>>>,
}

impl RegisteredComponentsResource {
    pub fn new() -> Self {
        let hashmap_registry = ComponentRegister::by_unique_uid();

        let mut vec_registry = hashmap_registry
            .clone()
            .into_iter()
            .map(|(k, v)| (k, v))
            .collect::<Vec<(Uid, RegisteredComponent)>>();
        vec_registry.sort_by(|a, b| a.1.ty().partial_cmp(&b.1.ty()).unwrap());

        Self {
            hashmap_registry: Arc::new(Mutex::new(hashmap_registry)),
            vec_registry: Arc::new(Mutex::new(vec_registry)),
        }
    }

    pub fn hashmap(&self) -> HashmapRegistry<'_> {
        HashmapRegistry::new(self.hashmap_registry.lock().unwrap())
    }

    pub fn slice(&self) -> SliceRegistry<'_> {
        SliceRegistry::new(self.vec_registry.lock().unwrap())
    }
}

pub struct HashmapRegistry<'a> {
    lock: MutexGuard<'a, HashMap<Uid, RegisteredComponent>>,
}

impl<'a> HashmapRegistry<'a> {
    pub fn new(guard: MutexGuard<'a, HashMap<Uid, RegisteredComponent>>) -> HashmapRegistry<'a> {
        Self { lock: guard }
    }

    pub fn iter(&self) -> hash_map::Iter<'_, Uid, RegisteredComponent> {
        self.lock.iter()
    }

    pub fn get(&self, id: &Uid) -> Option<&RegisteredComponent> {
        self.lock.get(id)
    }
}

pub struct SliceRegistry<'a> {
    lock: MutexGuard<'a, Vec<(Uid, RegisteredComponent)>>,
}

impl<'a> SliceRegistry<'a> {
    pub fn new(guard: MutexGuard<'a, Vec<(Uid, RegisteredComponent)>>) -> SliceRegistry<'a> {
        SliceRegistry { lock: guard }
    }

    pub fn iter(&self) -> slice::Iter<(Uid, RegisteredComponent)> {
        self.lock.iter()
    }
}
