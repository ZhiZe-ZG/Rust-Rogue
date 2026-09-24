//! Per-cell monster occupancy for the current level.

use crate::config::GameConfig;
use crate::entity::player::Thing;

/// Per-cell monster occupancy grid for the current level.
///
/// Stores the monster pointer resting on each map cell (or null), indexed with
/// the legacy `(x << 5) + y` layout so the save format stays byte-compatible.
/// This replaces the legacy C `places` global, whose only remaining member was
/// the per-cell `p_monst` pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonsterMap {
    cells: [*mut Thing; GameConfig::LEVEL_HEIGHT * GameConfig::LEVEL_WIDTH],
}

// The raw pointers are neither `Send` nor `Sync`, but the game is
// single-threaded and they are only dereferenced by the gameplay loop. `Level`
// (which owns this grid) is shared through `CURRENT_LEVEL`'s `RwLock`.
unsafe impl Send for MonsterMap {}
unsafe impl Sync for MonsterMap {}

impl MonsterMap {
    /// An empty map.
    pub fn new() -> Self {
        Self {
            cells: [std::ptr::null_mut(); GameConfig::LEVEL_HEIGHT * GameConfig::LEVEL_WIDTH],
        }
    }

    /// Legacy flat index of `(y, x)`: `(x << 5) + y`.
    #[inline]
    fn index(y: usize, x: usize) -> usize {
        (x << 5) + y
    }

    /// The monster at `(y, x)`, or null.
    #[inline]
    pub fn at(&self, y: usize, x: usize) -> *mut Thing {
        self.cells[Self::index(y, x)]
    }

    /// Place `ptr` at `(y, x)`.
    #[inline]
    pub fn set(&mut self, y: usize, x: usize, ptr: *mut Thing) {
        self.cells[Self::index(y, x)] = ptr;
    }

    /// Clear every cell.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = std::ptr::null_mut();
        }
    }
}
