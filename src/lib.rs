use legion::prelude::{Entity, World};

use crate::register::ComponentRegistration;

mod event;

pub mod clone_merge;
pub mod components;
pub mod error;
pub mod filters;

pub mod resources;
pub mod systems;
#[macro_use]
pub mod register;

pub mod universe;

pub mod tracking {
    //! Re-export of the [track](LINK) crate.
    //!
    //! Track struct data modifications.

    pub use inventory;
    pub use legion::storage::ComponentTypeId;

    pub use legion_sync_macro::sync;
    pub use track::{preclude::*, *};
}

pub fn create_copy_clone_impl() -> clone_merge::CopyCloneImpl {
    let component_registry = register::ComponentRegister::by_component_id();
    let clone_merge_impl = clone_merge::CopyCloneImpl::new(component_registry);
    clone_merge_impl
}

pub trait WorldAbstraction {
    fn has_component(&self, entity: Entity, component: &ComponentRegistration) -> bool;
}

impl WorldAbstraction for World {
    fn has_component(&self, entity: Entity, component: &ComponentRegistration) -> bool {
        component.exists_in_world(&self, entity)
    }
}

impl WorldAbstraction for legion::systems::SubWorld {
    fn has_component(&self, entity: Entity, component: &ComponentRegistration) -> bool {
        component.exists_in_subworld(&self, entity)
    }
}
