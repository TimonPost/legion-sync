use legion::prelude::BitSet;

#[derive(Debug)]
pub struct TrackResource {
    pub(crate) inserted: BitSet<u32>,
    pub(crate) modified: BitSet<u32>,
    pub(crate) removed: BitSet<u32>,
}

impl TrackResource {
    pub fn new() -> TrackResource {
        TrackResource {
            inserted: BitSet::new(),
            modified: BitSet::new(),
            removed: BitSet::new(),
        }
    }

    pub fn insert(&mut self, set: usize) {
        // If previously removed/modified we don't need to know that anymore.
        self.removed.remove(set);
        self.modified.remove(set);
        self.inserted.insert(set);
    }

    pub fn remove(&mut self, set: usize) {
        // Don't need to know that it was inserted/modified if it was subsequently
        // removed.
        self.inserted.remove(set);
        self.modified.remove(set);
        self.removed.insert(set);
    }

    pub fn modify(&mut self, set: usize) {
        self.modified.insert(set);
    }

    pub fn clear(&mut self) {
        self.inserted.clear();
        self.modified.clear();
        self.removed.clear();
    }
}

impl Clone for TrackResource {
    fn clone(&self) -> Self {
        TrackResource {
            inserted: self.inserted.clone(),
            removed: self.removed.clone(),
            modified: self.removed.clone(),
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::resources::TrackResource;

    #[test]
    fn update_insert_test() {
        let mut resource = TrackResource::new();
        resource.modified.insert(1);
        resource.removed.insert(1);

        resource.insert(1);

        assert!(!resource.removed.contains(1));
        assert!(!resource.modified.contains(1));
        assert!(resource.inserted.contains(1));
    }

    #[test]
    fn update_modified_test() {
        let mut resource = TrackResource::new();
        resource.inserted.insert(1);
        resource.removed.insert(1);

        resource.modify(1);

        assert!(resource.modified.contains(1));
    }

    #[test]
    fn update_remove_test() {
        let mut resource = TrackResource::new();
        resource.inserted.insert(1);
        resource.modified.insert(1);

        resource.remove(1);

        assert!(!resource.inserted.contains(1));
        assert!(!resource.modified.contains(1));
        assert!(resource.removed.contains(1));
    }

    #[test]
    fn clear_test() {
        let mut resource = TrackResource::new();
        resource.inserted.insert(1);
        resource.modified.insert(1);
        resource.removed.insert(1);

        resource.clear();

        assert!(!resource.inserted.contains(1));
        assert!(!resource.modified.contains(1));
        assert!(!resource.removed.contains(1));
    }
}
