pub mod components;
pub mod error;
pub mod resources;
pub mod systems;
#[macro_use]
pub mod register;
pub mod event;
pub mod filters;
pub mod world;

pub mod tracking {
    //! Re-export of the [track](LINK) crate.
    //!
    //! Track struct data modifications.

    pub use inventory;
    pub use legion::storage::ComponentTypeId;

    pub use legion_sync_macro::sync;
    pub use net_sync::{re_exports, track_attr};
}
