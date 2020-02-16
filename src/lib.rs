pub use event::Event;
pub use packet::{
    Message, NetworkPacket, NetworkPacketBuilder, NetworkPacketReader, UrgencyRequirement,
};

mod components;
mod event;
mod packet;

pub mod resources;
pub mod systems;

pub mod tracking {
    //! Re-export of the [track](LINK) crate.
    //!
    //! Track struct data modifications.

    pub use track::{preclude::*, *};
}
