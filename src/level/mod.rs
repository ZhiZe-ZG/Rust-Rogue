//! Level generation.
//!
//! Digs and populates a new dungeon level: room layout, maze corridors,
//! passages, objects, traps, and the down staircase.

mod config;
mod generation;
mod level;
mod mirror;
mod passages;
mod presence;
mod roomgraph;
mod rooms;
mod structure;
mod symbols;
mod tile;
mod trap;

pub use config::GameConfig;
pub use generation::{door_open, new_level};
pub use level::{with_current_level, with_current_level_mut, Level, LevelFlags};
pub(crate) use presence::find_floor;

pub use passages::Passage;
pub use roomgraph::RoomGraph;
pub use rooms::{Door, DoorKind, Room};
pub use structure::Structure;
pub use tile::Tile;
pub use trap::{be_trapped, Trap};
