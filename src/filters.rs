use crate::{components::UidComponent, resources::TrackResource};
use legion::{
    filter::*,
    iterator::SliceVecIter,
    storage::{ComponentResourceSet, ComponentStorage, ComponentTypeId},
};
use std::{collections::HashSet, slice::Iter};

pub mod filter_fns {
    use super::{ModifiedFilter, RemovedFilter, TrackFilter};
    use crate::{
        filters::{AllFilter, RegisteredComponentFilter},
        register::ComponentRegister,
        resources::TrackResource,
        tracking::ComponentTypeId,
    };
    use legion::filter::{Any, EntityFilterTuple, Passthrough};
    use std::collections::HashSet;

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

    pub fn registered() -> EntityFilterTuple<RegisteredComponentFilter, Any, Any> {
        let registered_components = ComponentRegister::by_component_id()
            .iter()
            .map(|(k, _)| *k)
            .collect::<HashSet<ComponentTypeId>>();

        EntityFilterTuple::new(
            RegisteredComponentFilter::new(registered_components),
            Any,
            Any,
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
            Some(self.filter.filter(&self.cash, raw.uid() as usize))
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

#[derive(Debug)]
pub struct RegisteredComponentFilter(HashSet<ComponentTypeId>);

impl RegisteredComponentFilter {
    fn new(registered_components: HashSet<ComponentTypeId>) -> Self {
        RegisteredComponentFilter(registered_components)
    }
}

impl ActiveFilter for RegisteredComponentFilter {}

impl Clone for RegisteredComponentFilter {
    fn clone(&self) -> Self {
        RegisteredComponentFilter(self.0.clone())
    }
}

impl<'a> Filter<ArchetypeFilterData<'a>> for RegisteredComponentFilter {
    type Iter = SliceVecIter<'a, ComponentTypeId>;

    #[inline]
    fn collect(&self, source: ArchetypeFilterData<'a>) -> Self::Iter {
        source.component_types.iter()
    }

    #[inline]
    fn is_match(&self, item: &<Self::Iter as Iterator>::Item) -> Option<bool> {
        for i in item.iter() {
            if self.0.contains(i) {
                return Some(true);
            }
        }

        return Some(false);
    }
}

impl std::ops::Not for RegisteredComponentFilter {
    type Output = Not<Self>;

    #[inline]
    fn not(self) -> Self::Output {
        Not { filter: self }
    }
}

impl<'a, Rhs: ActiveFilter> std::ops::BitAnd<Rhs> for RegisteredComponentFilter {
    type Output = And<(Self, Rhs)>;

    #[inline]
    fn bitand(self, rhs: Rhs) -> Self::Output {
        And {
            filters: (self, rhs),
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::{
        components::UidComponent,
        filters::{
            filter_fns::{all, modified, removed},
            AllFilter, ModifiedFilter, RegisteredComponentFilter, RemovedFilter,
            TrackResourceFilter,
        },
        resources::TrackResource,
        tracking::ComponentTypeId,
    };
    use legion::{
        event::Event,
        filter::*,
        prelude::{IntoQuery, Read, Universe, World},
    };
    use std::collections::HashSet;

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
        let resource = TrackResource::new();
        assert_eq!(AllFilter.filter(&resource, 1), false);
    }

    #[test]
    fn modified_filter_should_fail_test() {
        let resource = TrackResource::new();
        assert_eq!(ModifiedFilter.filter(&resource, 1), false);
    }

    #[test]
    fn removed_filter_should_fail_test() {
        let resource = TrackResource::new();
        assert_eq!(RemovedFilter.filter(&resource, 1), false);
    }

    #[test]
    fn filter_modified_query() {
        let (_, world) = get_world();

        let query = <Read<UidComponent>>::query();

        let mut track_resource = TrackResource::new();
        track_resource.modify(1);

        let empty_resource = TrackResource::new();

        // 1) Filter and query modified components and retrieve the packets for those.
        let pass_query = query.clone().filter(modified(&track_resource));

        for modified in pass_query.iter(&world) {
            assert_eq!(modified.uid(), 1);
        }

        let empty_query = query.clone().filter(modified(&empty_resource));

        for _ in empty_query.iter(&world) {
            assert!(false);
        }
    }

    #[test]
    fn filter_removed_query() {
        let (_, world) = get_world();

        let query = <Read<UidComponent>>::query();

        let mut track_resource = TrackResource::new();
        track_resource.remove(1);

        let empty_resource = TrackResource::new();

        // 1) Filter and query modified components and retrieve the packets for those.
        let pass_query = query.clone().filter(removed(&track_resource));

        for modified in pass_query.iter(&world) {
            assert_eq!(modified.uid(), 1);
        }

        let empty_query = query.clone().filter(removed(&empty_resource));

        for _ in empty_query.iter(&world) {
            assert!(false);
        }
    }

    #[test]
    fn filter_all_query() {
        let (_, world) = get_world();

        let query = <Read<UidComponent>>::query();

        let mut track_resource = TrackResource::new();
        track_resource.insert(1);

        let empty_resource = TrackResource::new();

        // 1) Filter and query modified components and retrieve the packets for those.
        let pass_query = query.clone().filter(all(&track_resource));

        for modified in pass_query.iter(&world) {
            assert_eq!(modified.uid(), 1);
        }

        let empty_query = query.clone().filter(all(&empty_resource));

        for _ in empty_query.iter(&world) {
            assert!(false);
        }
    }

    #[test]
    fn filter_registered_components() {
        struct A;
        struct B;
        struct C;

        let (_, mut world) = get_world();

        world.insert((), vec![(A,)]);
        world.insert((), vec![(B,)]);
        world.insert((), vec![(C,)]);

        let mut registered = HashSet::new();
        registered.insert(ComponentTypeId::of::<A>());
        registered.insert(ComponentTypeId::of::<B>());

        let filter = EntityFilterTuple::new(
            RegisteredComponentFilter::new(registered),
            Passthrough,
            Passthrough,
        );

        let a_query = <Read<A>>::query().filter(filter.clone());
        let b_query = <Read<B>>::query().filter(filter.clone());
        let c_query = <Read<C>>::query().filter(filter.clone());

        let mut count = 0;

        for _ in a_query.iter(&world) {
            count += 1;
        }

        for _ in b_query.iter(&world) {
            count += 1;
        }

        for _ in c_query.iter(&world) {
            count += 1;
        }

        assert_eq!(count, 2);
    }

    #[test]
    fn should_receive_events_test() {
        struct A;

        let (tx, rx) = crate::tracking::re_exports::crossbeam_channel::unbounded::<Event>();
        let (_universe, mut world) = get_world();

        let mut registered = HashSet::new();
        registered.insert(ComponentTypeId::of::<A>());

        let filter = EntityFilterTuple::new(RegisteredComponentFilter::new(registered), Any, Any);

        world.subscribe(tx, filter);

        let entities1 = world.insert((), vec![(A,)]).to_owned();
        world.delete(entities1[0]);
        let entities2 = world.insert((), vec![(A,)]).to_owned();
        world.delete(entities2[0]);
        let entities3 = world.insert((), vec![(A,)]).to_owned();
        world.delete(entities3[0]);

        let events = rx.try_iter().collect::<Vec<Event>>();
        assert_eq!(
            events
                .iter()
                .filter(|e| match **e {
                    Event::EntityRemoved(_, _) => true,
                    _ => false,
                })
                .map(|x| x.clone())
                .collect::<Vec<Event>>()
                .len(),
            3
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| match **e {
                    Event::EntityInserted(_, _) => true,
                    _ => false,
                })
                .map(|x| x.clone())
                .collect::<Vec<Event>>()
                .len(),
            3
        );
    }

    fn get_world() -> (Universe, World) {
        let universe = Universe::new();
        let mut world = universe.create_world();

        world.insert((), vec![(UidComponent::new(1),)]);

        (universe, world)
    }
}
