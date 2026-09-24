//! Level generation.
//!
//! Digs and populates a new dungeon level: room layout, maze corridors,
//! passages, objects, traps, and the down staircase.

mod generation;
mod level;
mod monster_map;
mod passages;
mod presence;
mod roomgraph;
mod structure;
mod tile;

pub use generation::{door_open, new_level};
pub use level::{with_current_level, with_current_level_mut, Level, LevelFlags};
pub use monster_map::MonsterMap;
pub(crate) use presence::find_floor;

pub use passages::{Passage, PassageLinks};
pub use roomgraph::RoomGraph;
pub use structure::{Room, Structure};
pub use tile::{Tile, Trap, TrapHit};
