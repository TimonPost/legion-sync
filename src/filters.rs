use crate::{components::UidComponent, resources::TrackResource};
use legion::{
    filter::*,
    storage::{ComponentResourceSet, ComponentStorage, ComponentTypeId},
};
use std::slice::Iter;

pub mod filter_fns {
    use super::{ModifiedFilter, RemovedFilter, TrackFilter};
    use crate::{filters::AllFilter, resources::TrackResource};
    use legion::filter::{EntityFilterTuple, Passthrough};

    pub fn all<'a>(
        cash: &'a TrackResource,
    ) -> EntityFilterTuple<Passthrough, Passthrough, TrackFilter<'a, AllFilter>> {
        EntityFilterTuple::new(
            Passthrough,
            Passthrough,
            TrackFilter::<AllFilter>::new(cash, AllFilter),
        )
    }

    /// Creates an entity data filter which includes chunks that contain
    /// entity data components of type `T`.
    pub fn modified<'a>(
        cash: &'a TrackResource,
    ) -> EntityFilterTuple<Passthrough, Passthrough, TrackFilter<'a, ModifiedFilter>> {
        EntityFilterTuple::new(
            Passthrough,
            Passthrough,
            TrackFilter::<ModifiedFilter>::new(cash, ModifiedFilter),
        )
    }

    /// Creates an entity data filter which includes chunks that contain
    /// entity data components of type `T`.
    pub fn removed<'a>(
        cash: &'a TrackResource,
    ) -> EntityFilterTuple<Passthrough, Passthrough, TrackFilter<'a, RemovedFilter>> {
        EntityFilterTuple::new(
            Passthrough,
            Passthrough,
            TrackFilter::<RemovedFilter>::new(cash, RemovedFilter),
        )
    }
}
#[derive(Clone)]
pub struct AllFilter;
#[derive(Clone)]
pub struct RemovedFilter;
#[derive(Clone)]
pub struct ModifiedFilter;

pub trait TrackResourceFilter: Send + Sync + Clone {
    fn filter(&self, resource: &TrackResource, identifier: usize) -> bool;
}

impl TrackResourceFilter for AllFilter {
    fn filter(&self, resource: &TrackResource, identifier: usize) -> bool {
        resource.removed.contains(identifier)
            || resource.inserted.contains(identifier)
            || resource.modified.contains(identifier)
    }
}

impl TrackResourceFilter for RemovedFilter {
    fn filter(&self, resource: &TrackResource, identifier: usize) -> bool {
        resource.removed.contains(identifier)
    }
}

impl TrackResourceFilter for ModifiedFilter {
    fn filter(&self, resource: &TrackResource, identifier: usize) -> bool {
        resource.modified.contains(identifier)
    }
}

/// A filter which requires that entity data of type `T` has changed within the
/// chunk since the last time the filter was executed.
#[derive(Debug, Clone)]
pub struct TrackFilter<'a, F: TrackResourceFilter> {
    cash: &'a TrackResource,
    filter: F,
}

impl<'a, F: TrackResourceFilter> TrackFilter<'a, F> {
    pub fn new(cash: &'a TrackResource, filter: F) -> TrackFilter<'_, F> {
        TrackFilter { cash, filter }
    }
}

impl<'a, F: TrackResourceFilter> ActiveFilter for TrackFilter<'_, F> {}

impl<'a, F: TrackResourceFilter> Filter<ChunkFilterData<'a>> for TrackFilter<'_, F> {
    type Iter = Iter<'a, ComponentStorage>;

    fn collect(&self, source: ChunkFilterData<'a>) -> Self::Iter {
        source.chunks.iter()
    }

    #[inline]
    fn is_match(&self, item: &<Self::Iter as Iterator>::Item) -> Option<bool> {
        let component_id = ComponentTypeId::of::<UidComponent>();

        let components = item.components(component_id);
        if components.is_none() {
            return Some(false);
        }

        let components: &ComponentResourceSet = components.unwrap();

        unsafe {
            let raw = &components.data_slice::<UidComponent>()[0];
            Some(self.filter.filter(&self.cash, raw.uid().0 as usize))
        }
    }
}

impl<'a, F: TrackResourceFilter> std::ops::Not for TrackFilter<'_, F> {
    type Output = Not<Self>;

    #[inline]
    fn not(self) -> Self::Output {
        Not { filter: self }
    }
}

impl<'a, Rhs: ActiveFilter, F: TrackResourceFilter> std::ops::BitAnd<Rhs> for TrackFilter<'_, F> {
    type Output = And<(Self, Rhs)>;

    #[inline]
    fn bitand(self, rhs: Rhs) -> Self::Output {
        And {
            filters: (self, rhs),
        }
    }
}

impl<'a, F: TrackResourceFilter> std::ops::BitOr<Passthrough> for TrackFilter<'_, F> {
    type Output = Self;

    #[inline]
    fn bitor(self, _: Passthrough) -> Self::Output {
        self
    }
}

#[cfg(test)]
pub mod test {
    use crate::{
        components::UidComponent,
        filters::{
            filter_fns::{all, modified, removed},
            AllFilter, ModifiedFilter, RemovedFilter, TrackResourceFilter,
        },
        resources::TrackResource,
    };
    use legion::prelude::{IntoQuery, Read, Universe, World};
    use net_sync::uid::Uid;

    #[test]
    fn all_filter_should_pass_test() {
        let mut resource = TrackResource::new();
        resource.insert(1);
        resource.remove(1);
        resource.modify(1);

        assert_eq!(AllFilter.filter(&resource, 1), true);
    }

    #[test]
    fn modified_filter_should_pass_test() {
        let mut resource = TrackResource::new();
        resource.modify(1);

        assert_eq!(ModifiedFilter.filter(&resource, 1), true);
    }

    #[test]
    fn removed_filter_should_pass_test() {
        let mut resource = TrackResource::new();
        resource.remove(1);

        assert_eq!(RemovedFilter.filter(&resource, 1), true);
    }

    #[test]
    fn all_filter_should_fail_test() {
        let mut resource = TrackResource::new();
        assert_eq!(AllFilter.filter(&resource, 1), false);
    }

    #[test]
    fn modified_filter_should_fail_test() {
        let mut resource = TrackResource::new();
        assert_eq!(ModifiedFilter.filter(&resource, 1), false);
    }

    #[test]
    fn removed_filter_should_fail_test() {
        let mut resource = TrackResource::new();
        assert_eq!(RemovedFilter.filter(&resource, 1), false);
    }

    #[test]
    fn filter_modified_query() {
        let (universe, world) = get_world();

        let query = <Read<UidComponent>>::query();

        let mut track_resource = TrackResource::new();
        track_resource.modify(1);

        let empty_resource = TrackResource::new();

        // 1) Filter and query modified components and retrieve the packets for those.
        let pass_query = query.clone().filter(modified(&track_resource));

        for modified in pass_query.iter(&world) {
            assert_eq!(modified.uid().id(), 1);
        }

        let empty_query = query.clone().filter(modified(&empty_resource));

        for modified in empty_query.iter(&world) {
            assert!(false);
        }
    }

    #[test]
    fn filter_removed_query() {
        let (universe, world) = get_world();

        let query = <Read<UidComponent>>::query();

        let mut track_resource = TrackResource::new();
        track_resource.remove(1);

        let empty_resource = TrackResource::new();

        // 1) Filter and query modified components and retrieve the packets for those.
        let pass_query = query.clone().filter(removed(&track_resource));

        for modified in pass_query.iter(&world) {
            assert_eq!(modified.uid().id(), 1);
        }

        let empty_query = query.clone().filter(removed(&empty_resource));

        for modified in empty_query.iter(&world) {
            assert!(false);
        }
    }

    #[test]
    fn filter_all_query() {
        let (universe, world) = get_world();

        let query = <Read<UidComponent>>::query();

        let mut track_resource = TrackResource::new();
        track_resource.insert(1);

        let empty_resource = TrackResource::new();

        // 1) Filter and query modified components and retrieve the packets for those.
        let pass_query = query.clone().filter(all(&track_resource));

        for modified in pass_query.iter(&world) {
            assert_eq!(modified.uid().id(), 1);
        }

        let empty_query = query.clone().filter(all(&empty_resource));

        for modified in empty_query.iter(&world) {
            assert!(false);
        }
    }

    fn get_world() -> (Universe, World) {
        let universe = Universe::new();
        let mut world = universe.create_world();

        world.insert((), vec![(UidComponent::new(Uid(1)),)]);

        (universe, world)
    }
}
