//! Level generation.
//!
//! Digs and populates a new dungeon level: room layout, maze corridors,
//! passages, objects, traps, and the down staircase.

mod level;
mod passages;
mod roomgraph;

pub use level::{Level, LevelFlags};

pub use passages::PassageLinks;
pub use roomgraph::RoomGraph;

// `Passage` now lives with the other level-geometry models in `structure`;
// re-export it here so existing `crate::level::Passage` callers keep working.
pub use crate::structure::Passage;

// Scoped live-level access lives in the game-state module; re-export it here so
// callers can keep using `crate::level::{with_current_level, ...}`.
pub use crate::game::{with_current_level, with_current_level_mut};
