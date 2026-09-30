//! Level generation.
//!
//! Digs and populates a new dungeon level: room layout, maze corridors,
//! passages, objects, traps, and the down staircase.

mod level;
mod passages;
mod presence;
mod roomgraph;

pub use level::{Level, LevelFlags};
pub(crate) use presence::{find_floor, populate_level};

// ---------------------------------------------------------------------------
// Generation-global accessors
// ---------------------------------------------------------------------------
//
// Level generation reads and updates a handful of process-wide globals. Rather
// than reaching into `game::globals` from several modules, funnel the access
// through these small helpers so generation code stays free of raw `static mut`
// references.

use crate::game::globals::{max_level, no_food};

/// Raise the recorded maximum dungeon depth to at least `depth`.
pub(crate) unsafe fn record_max_depth(depth: i32) {
    if depth > max_level {
        max_level = depth;
    }
}

/// The highest dungeon depth reached so far.
pub(crate) fn max_depth() -> i32 {
    unsafe { max_level }
}

/// Note that one more level's food has been supplied.
pub(crate) fn bump_no_food() {
    unsafe {
        no_food += 1;
    }
}

pub use passages::PassageLinks;
pub use roomgraph::RoomGraph;

// `Passage` now lives with the other level-geometry models in `structure`;
// re-export it here so existing `crate::level::Passage` callers keep working.
pub use crate::structure::Passage;

// Scoped live-level access lives in the game-state module; re-export it here so
// callers can keep using `crate::level::{with_current_level, ...}`.
pub use crate::game::{with_current_level, with_current_level_mut};
