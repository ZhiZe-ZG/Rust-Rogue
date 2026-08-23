//! Level generation.
//!
//! Digs and populates a new dungeon level: room layout, maze corridors,
//! passages, objects, traps, and the down staircase. Exposed to the C game
//! through the legacy FFI symbols (see [`ffi`]).

mod ffi;
mod ffitools;
mod level;
pub mod passages;
pub mod roomgraph;
pub mod rooms;
pub mod structure;
pub mod tile;

pub use ffi::{door_open, new_level};
pub use level::{
    create_level, current_level_mut, set_current_level, Door, DoorKind, Level, LEVEL_HEIGHT,
    LEVEL_WIDTH, MAX_LEVEL_PASSAGES, MAX_LEVEL_ROOMS,
};

pub use passages::Passage;
pub use roomgraph::{RoomGraph, MAX_ROOMS};
pub use rooms::Room;
pub use structure::Structure;
pub use tile::Tile;
