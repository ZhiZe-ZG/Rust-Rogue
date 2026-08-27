//! Level generation.
//!
//! Digs and populates a new dungeon level: room layout, maze corridors,
//! passages, objects, traps, and the down staircase. Exposed to the C game
//! through the legacy FFI symbols (see [`ffi`]).

mod ffi;
mod ffitools;
mod level;
mod mirror;
mod passages;
mod presence;
mod redraw;
mod roomgraph;
mod rooms;
mod structure;
mod symbols;
mod tile;
mod trap;

pub use ffi::{door_open, new_level};
pub use level::{current_level_mut, Level, LevelFlags, LEVEL_HEIGHT, LEVEL_WIDTH};

pub use passages::Passage;
pub use roomgraph::{RoomGraph, MAX_ROOMS};
pub use rooms::{Door, DoorKind, Room};
pub use structure::Structure;
pub use tile::Tile;
pub use trap::{be_trapped, T_ARROW, T_BEAR, T_DART, T_DOOR, T_MYST, T_RUST, T_SLEEP, T_TELEP};
