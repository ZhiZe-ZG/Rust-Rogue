//! Level generation.
//!
//! Digs and populates a new dungeon level: room layout, maze corridors,
//! passages, objects, traps, and the down staircase. Exposed to the C game
//! through the legacy FFI symbols (see [`ffi`]).

mod ffi;
pub mod passages;
pub mod rooms;
pub mod structure;
pub mod tile;

pub use ffi::{door_open, new_level};
pub use rooms::{place_tile, Room};
pub use structure::Structure;
pub use tile::Tile;
