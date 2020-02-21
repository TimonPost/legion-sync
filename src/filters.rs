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
            || return resource.inserted.contains(identifier)
                || return resource.modified.contains(identifier)
    }
}

impl TrackResourceFilter for RemovedFilter {
    fn filter(&self, resource: &TrackResource, identifier: usize) -> bool {
        return resource.removed.contains(identifier);
    }
}

impl TrackResourceFilter for ModifiedFilter {
    fn filter(&self, resource: &TrackResource, identifier: usize) -> bool {
        return resource.modified.contains(identifier);
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
