//! Per-cell monster occupancy for the current level.

use crate::config::GameConfig;

use super::MonsterId;

/// Per-cell monster occupancy grid for the current level.
///
/// Stores the [`MonsterId`] of the monster resting on each map cell (or `None`),
/// indexed with the legacy `(x << 5) + y` layout so the save format stays
/// byte-compatible. This replaces the legacy C `places` global, whose only
/// remaining member was the per-cell raw `p_monst` pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonsterMap {
    cells: [Option<MonsterId>; GameConfig::LEVEL_HEIGHT * GameConfig::LEVEL_WIDTH],
}

impl Default for MonsterMap {
    fn default() -> Self {
        Self::new()
    }
}

impl MonsterMap {
    /// An empty map.
    pub fn new() -> Self {
        Self {
            cells: [None; GameConfig::LEVEL_HEIGHT * GameConfig::LEVEL_WIDTH],
        }
    }

    /// Legacy flat index of `(y, x)`: `(x << 5) + y`.
    #[inline]
    fn index(y: usize, x: usize) -> usize {
        (x << 5) + y
    }

    /// The monster occupying `(y, x)`, if any.
    #[inline]
    pub fn at(&self, y: usize, x: usize) -> Option<MonsterId> {
        self.cells[Self::index(y, x)]
    }

    /// Place `id` at `(y, x)`.
    #[inline]
    pub fn set(&mut self, y: usize, x: usize, id: Option<MonsterId>) {
        self.cells[Self::index(y, x)] = id;
    }

    /// Clear every cell.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = None;
        }
    }
}