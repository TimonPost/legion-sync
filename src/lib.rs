pub use event::Event;
pub use transport::{ComponentRecord, Message, ReceivedPacket, SentPacket, UrgencyRequirement};

mod event;
mod transport;

pub mod clone_merge;
pub mod components;
pub mod error;
pub mod filters;
pub mod network_universe;
pub mod resources;
pub mod systems;
#[macro_use]
pub mod register;

pub mod tracking {
    //! Re-export of the [track](LINK) crate.
    //!
    //! Track struct data modifications.

    pub use track::{preclude::*, *};

    pub use inventory;
    pub use legion_sync_macro::sync;
}

pub fn create_copy_clone_impl() -> clone_merge::CopyCloneImpl {
    let component_registry = register::ComponentRegister::by_component_id();
    let clone_merge_impl = clone_merge::CopyCloneImpl::new(component_registry);
    clone_merge_impl
}
