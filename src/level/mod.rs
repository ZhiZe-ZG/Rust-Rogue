//! Level generation.
//!
//! Digs and populates a new dungeon level: room layout, maze corridors,
//! passages, objects, traps, and the down staircase.

mod generation;
mod level;
mod passages;
mod presence;
mod roomgraph;

pub use generation::{door_open, new_level};
pub use level::{Level, LevelFlags};
pub(crate) use presence::find_floor;

pub use passages::{Passage, PassageLinks};
pub use roomgraph::RoomGraph;

// Scoped live-level access lives in the game-state module; re-export it here so
// callers can keep using `crate::level::{with_current_level, ...}`.
pub use crate::game::{with_current_level, with_current_level_mut};