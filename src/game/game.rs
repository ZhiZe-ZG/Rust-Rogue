//! Game-state sub-module.
//!
//! Owns the process-wide dungeon-level state that the legacy C engine
//! previously kept in globals:
//!
//! * the **current level** — the [`Level`] singleton (tile map, flags, rooms,
//!   passages, and floor items) for the live dungeon depth.
//!
//! The player actor and equipment live in [`crate::game::player`]; the live
//! monster list in [`crate::game::MonsterList`]; and the per-cell monster
//! occupancy grid in [`crate::game::MONSTER_MAP`].
//!
//! Cell display glyphs and flat flags are no longer cached in a legacy per-cell
//! grid (the `p_ch`/`p_flags` members were removed); every access goes through
//! `crate::draw`, which computes them from the [`Level`] tile map and flag
//! grids on the fly.

use std::sync::RwLock;

use crate::config::GameConfig;
use crate::entity::player::Thing;
use crate::level::Level;
use crate::tile::Tile;
use glam::IVec2;

use crate::game::MONSTER_MAP;

/// Lazily initialized owner of the live dungeon level.
pub struct CurrentLevel {
    level: RwLock<Option<Level>>,
}

impl CurrentLevel {
    const EMPTY: Self = Self {
        level: RwLock::new(None),
    };

    /// Ensure the live level exists, initializing it on first access.
    #[inline]
    fn ensure_initialized(&self) {
        let mut level = self
            .level
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        if level.is_none() {
            *level = Some(Level::new());
        }
    }

    #[inline]
    pub fn with<R>(&self, operation: impl FnOnce(&Level) -> R) -> R {
        self.ensure_initialized();
        let level = self
            .level
            .read()
            .unwrap_or_else(|poison| poison.into_inner());
        operation(level.as_ref().unwrap())
    }

    #[inline]
    pub fn with_mut<R>(&self, operation: impl FnOnce(&mut Level) -> R) -> R {
        self.ensure_initialized();
        let mut level = self
            .level
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        operation(level.as_mut().unwrap())
    }
}

/// The [`MonsterId`] occupying `(y, x)`, or `None`.
///
/// This is the pointer-free accessor preferred by new code; the per-cell map
/// already stores a [`MonsterId`] and never a raw pointer.
#[inline]
pub fn monster_id_at(y: i32, x: i32) -> Option<crate::game::MonsterId> {
    MONSTER_MAP.at(y as usize, x as usize)
}

/// Whether a live monster stands at `(y, x)`.
#[inline]
pub fn monster_here(y: i32, x: i32) -> bool {
    MONSTER_MAP.at(y as usize, x as usize).is_some()
}

/// Place the monster `id` (or clear the cell with `None`) at `(y, x)`.
#[inline]
pub fn set_monster_id(y: i32, x: i32, id: Option<crate::game::MonsterId>) {
    MONSTER_MAP.set(y as usize, x as usize, id);
}

/// Clear the monster occupancy at `(y, x)`.
#[inline]
pub fn clear_monster(y: i32, x: i32) {
    MONSTER_MAP.set(y as usize, x as usize, None);
}

/// Clear every cell's monster pointer for a fresh level.
pub unsafe fn clear_level() {
    MONSTER_MAP.clear();
}

/// Whether the cell at `(y, x)` can be entered: no monster stands there and the
/// terrain tile is walkable.
pub unsafe fn cell_is_walkable(y: i32, x: i32) -> bool {
    if monster_here(y, x) {
        return false;
    }
    with_current_level(|level| level.tile_at(y as usize, x as usize).is_walkable())
}

/// The tile at `(y, x)`, defaulting to [`Tile::Empty`] outside the map.
pub unsafe fn tile_at(y: i32, x: i32) -> Tile {
    with_current_level(|level| level.tile_at(y as usize, x as usize))
}

/// Whether `(y, x)` is a door: an ordinary door, or a hidden door that has been
/// revealed.
pub unsafe fn is_door_at(y: i32, x: i32) -> bool {
    with_current_level(|level| level.is_door_at(y as usize, x as usize))
}

/// Process-wide owner for the live dungeon level.
///
/// The canonical holder of the current level. The level is initialized lazily
/// on its first access and is available only through scoped closure access.
pub static CURRENT_LEVEL: CurrentLevel = CurrentLevel::EMPTY;

/// Run `operation` with immutable access to the live level.
#[inline]
pub fn with_current_level<R>(operation: impl FnOnce(&Level) -> R) -> R {
    CURRENT_LEVEL.with(operation)
}

/// Run `operation` with mutable access to the live level.
#[inline]
pub fn with_current_level_mut<R>(operation: impl FnOnce(&mut Level) -> R) -> R {
    CURRENT_LEVEL.with_mut(operation)
}

/// Read the current dungeon depth (`Level::depth`).
#[inline]
pub fn current_depth() -> i32 {
    CURRENT_LEVEL.with(|level| level.depth)
}

/// Set the current dungeon depth (`Level::depth`).
#[inline]
pub fn set_current_depth(depth: i32) {
    CURRENT_LEVEL.with_mut(|level| level.depth = depth);
}

/// Read the current down-staircase position (`Level::stairs`).
#[inline]
pub fn stairs() -> IVec2 {
    CURRENT_LEVEL.with(|level| level.stairs)
}

/// Set the current down-staircase position (`Level::stairs`).
#[inline]
pub fn set_stairs(pos: IVec2) {
    CURRENT_LEVEL.with_mut(|level| level.stairs = pos);
}

/// Whether the room reference `reference` points to a dark room.
#[inline]
pub fn room_dark(room: Option<usize>) -> bool {
    CURRENT_LEVEL.with(|level| level.room_dark(room))
}

/// Whether the room reference `reference` points to a removed room.
#[inline]
pub fn room_gone(room: Option<usize>) -> bool {
    CURRENT_LEVEL.with(|level| level.room_gone(room))
}

/// Whether the room reference `reference` points to a maze room.
#[inline]
pub fn room_maze(room: Option<usize>) -> bool {
    CURRENT_LEVEL.with(|level| level.room_maze(room))
}

/// The value of the gold stash of the room `reference` points to.
#[inline]
pub fn room_goldval(room: Option<usize>) -> i32 {
    CURRENT_LEVEL.with(|level| level.room_goldval(room))
}

/// Set the value of the gold stash of the room `reference` points to.
#[inline]
pub fn set_room_goldval(room: Option<usize>, value: i32) {
    CURRENT_LEVEL.with_mut(|level| level.set_room_goldval(room, value));
}

/// Stable per-room gold positions, mirrored from `Level` so chase targets can
/// hold raw pointers without borrowing the locked level. Kept in sync by level
/// population (`presence::place_room_contents`) and save restore.
///
/// Owned by [`crate::game::globals`] and re-exported here for callers that
/// reach it through `crate::game::…`.
pub use crate::game::globals::ROOM_GOLD;

/// The stable gold-stash position of `reference` (or `None` when out of range).
///
/// The slot lives in the process-wide [`ROOM_GOLD`] array, so it does not
/// borrow the level lock.
#[inline]
pub unsafe fn room_gold_pos(room: Option<usize>) -> Option<IVec2> {
    match room {
        Some(i) if i < GameConfig::MAX_ROOMS => Some(ROOM_GOLD[i]),
        _ => None,
    }
}

/// The `(position, size)` of the room `reference` points to.
#[inline]
pub fn room_bounds(room: Option<usize>) -> Option<(IVec2, IVec2)> {
    CURRENT_LEVEL.with(|level| level.room_bounds(room))
}

/// Absolute door-exit coordinates for the room/passage `reference` points to.
#[inline]
pub fn room_exits(room: Option<usize>) -> Vec<IVec2> {
    CURRENT_LEVEL.with(|level| level.room_exits(room))
}

/// Absolute door-exit coordinates for a passage index.
#[inline]
pub fn passage_exits(passage: Option<usize>) -> Vec<IVec2> {
    CURRENT_LEVEL.with(|level| level.passage_exits(passage))
}

/// Arena handles for the floor items of the live level, head first.
///
/// This is the pointer-free accessor preferred by new code; callers look the
/// object up through [`crate::item::arena::OBJECTS`] when needed. The order
/// matches [`item_ptrs`].
#[inline]
pub fn item_ids() -> Vec<crate::item::arena::ThingId> {
    with_current_level(|level| {
        level
            .items
            .iter()
            .copied()
            .filter(|&id| crate::item::arena::OBJECTS.contains(id))
            .collect()
    })
}

/// Convenience alias for the crate-wide level size constants.
pub const GAME_HEIGHT: usize = GameConfig::LEVEL_HEIGHT;
pub const GAME_WIDTH: usize = GameConfig::LEVEL_WIDTH;

#[cfg(test)]
mod tests {
    use super::{CurrentLevel, Level};

    #[test]
    fn current_level_initializes_once() {
        let current_level = CurrentLevel::EMPTY;

        let first = current_level.with_mut(|level| level as *mut Level);
        let second = current_level.with(|level| level as *const Level);

        assert_eq!(first.cast_const(), second);
    }
}
