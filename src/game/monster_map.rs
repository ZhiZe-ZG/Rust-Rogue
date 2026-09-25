//! Per-cell monster occupancy for the current level.
//!
//! The legacy C engine kept the per-cell monster pointer in the `places` grid.
//! This module replaces that with a Rust-native owner: a [`MonsterMap`] stores
//! the [`MonsterId`] of the monster resting on each map cell, and the single
//! live map is the process-wide [`MONSTER_MAP`] global. The grid is held behind
//! a `Mutex` so the standalone global can be mutated through shared references,
//! mirroring [`crate::game::MonsterList`].

use std::sync::{Mutex, MutexGuard};

use crate::config::GameConfig;

use super::MonsterId;

/// Number of cells in the per-cell occupancy grid.
const CELLS: usize = GameConfig::LEVEL_HEIGHT * GameConfig::LEVEL_WIDTH;

/// Per-cell monster occupancy grid for the current level.
///
/// Stores the [`MonsterId`] of the monster resting on each map cell (or `None`),
/// indexed with the legacy `(x << 5) + y` layout so the save format stays
/// byte-compatible. This replaces the legacy C `places` global, whose only
/// remaining member was the per-cell raw `p_monst` pointer.
pub struct MonsterMap {
    cells: Mutex<[Option<MonsterId>; CELLS]>,
}

impl Default for MonsterMap {
    fn default() -> Self {
        Self::new()
    }
}

impl MonsterMap {
    /// An empty map.
    const fn new() -> Self {
        Self {
            cells: Mutex::new([None; CELLS]),
        }
    }

    fn lock(&self) -> MutexGuard<'_, [Option<MonsterId>; CELLS]> {
        self.cells
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Legacy flat index of `(y, x)`: `(x << 5) + y`.
    #[inline]
    fn index(y: usize, x: usize) -> usize {
        (x << 5) + y
    }

    /// The monster occupying `(y, x)`, if any.
    #[inline]
    pub fn at(&self, y: usize, x: usize) -> Option<MonsterId> {
        self.lock()[Self::index(y, x)]
    }

    /// Place `id` at `(y, x)`.
    #[inline]
    pub fn set(&self, y: usize, x: usize, id: Option<MonsterId>) {
        self.lock()[Self::index(y, x)] = id;
    }

    /// Clear every cell.
    pub fn clear(&self) {
        let mut cells = self.lock();
        for cell in cells.iter_mut() {
            *cell = None;
        }
    }
}

/// The process-wide per-cell monster occupancy for the live level.
pub static MONSTER_MAP: MonsterMap = MonsterMap::new();
