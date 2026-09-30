//! Game-state sub-module.
//!
//! Owns the process-wide dungeon-level state that the legacy C engine
//! previously kept in globals:
//!
//! * the **current level** — the [`Level`] singleton (tile map, flags, rooms,
//!   passages, and floor items) for the live dungeon depth.
//!
//! The player actor and equipment live in [`crate::game::player`]; the live
//! monster list and the per-cell monster occupancy grid are owned by the
//! [`DUNGEON`](crate::dungeon::DUNGEON) singleton.
//!
//! Cell display glyphs and flat flags are no longer cached in a legacy per-cell
//! grid (the `p_ch`/`p_flags` members were removed); every access goes through
//! `crate::draw`, which computes them from the [`Level`] tile map and flag
//! grids on the fly.

use crate::config::GameConfig;
use crate::dungeon::{MonsterId, DUNGEON};
use crate::entity::player::Thing;
use crate::level::Level;
use crate::tile::Tile;
use glam::IVec2;

/// The [`MonsterId`] occupying `(y, x)`, or `None`.
///
/// This is the pointer-free accessor preferred by new code; the per-cell map
/// already stores a [`MonsterId`] and never a raw pointer.
#[inline]
pub fn monster_id_at(y: i32, x: i32) -> Option<MonsterId> {
    DUNGEON.monster_map.at(y as usize, x as usize)
}

/// Whether a live monster stands at `(y, x)`.
#[inline]
pub fn monster_here(y: i32, x: i32) -> bool {
    DUNGEON.monster_map.at(y as usize, x as usize).is_some()
}

/// Place the monster `id` (or clear the cell with `None`) at `(y, x)`.
#[inline]
pub fn set_monster_id(y: i32, x: i32, id: Option<MonsterId>) {
    DUNGEON.monster_map.set(y as usize, x as usize, id);
}

/// Clear the monster occupancy at `(y, x)`.
#[inline]
pub fn clear_monster(y: i32, x: i32) {
    DUNGEON.monster_map.set(y as usize, x as usize, None);
}

/// Clear every cell's monster pointer for a fresh level.
pub unsafe fn clear_level() {
    DUNGEON.monster_map.clear();
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

/// Run `operation` with immutable access to the live level.
#[inline]
pub fn with_current_level<R>(operation: impl FnOnce(&Level) -> R) -> R {
    DUNGEON.with_level(operation)
}

/// Run `operation` with mutable access to the live level.
#[inline]
pub fn with_current_level_mut<R>(operation: impl FnOnce(&mut Level) -> R) -> R {
    DUNGEON.with_level_mut(operation)
}

/// Read the current dungeon depth (`Level::depth`).
#[inline]
pub fn current_depth() -> i32 {
    DUNGEON.current_depth()
}

/// Set the current dungeon depth (`Level::depth`).
#[inline]
pub fn set_current_depth(depth: i32) {
    DUNGEON.set_current_depth(depth);
}

/// Read the current down-staircase position (`Level::stairs`).
#[inline]
pub fn stairs() -> IVec2 {
    DUNGEON.stairs()
}

/// Set the current down-staircase position (`Level::stairs`).
#[inline]
pub fn set_stairs(pos: IVec2) {
    DUNGEON.set_stairs(pos);
}

/// Whether the room reference `reference` points to a dark room.
#[inline]
pub fn room_dark(room: Option<usize>) -> bool {
    DUNGEON.with_level(|level| level.room_dark(room))
}

/// Whether the room reference `reference` points to a removed room.
#[inline]
pub fn room_gone(room: Option<usize>) -> bool {
    DUNGEON.with_level(|level| level.room_gone(room))
}

/// Whether the room reference `reference` points to a maze room.
#[inline]
pub fn room_maze(room: Option<usize>) -> bool {
    DUNGEON.with_level(|level| level.room_maze(room))
}

/// The value of the gold stash of the room `reference` points to.
#[inline]
pub fn room_goldval(room: Option<usize>) -> i32 {
    DUNGEON.with_level(|level| level.room_goldval(room))
}

/// Set the value of the gold stash of the room `reference` points to.
#[inline]
pub fn set_room_goldval(room: Option<usize>, value: i32) {
    DUNGEON.with_level_mut(|level| level.set_room_goldval(room, value));
}

/// Stable per-room gold positions, mirrored out of `Level` so a
/// [`crate::entity::player::DestRef::RoomGold`] chase target can name a room
/// without borrowing the locked level. Kept in sync by level population
/// (`presence::place_room_contents`) and save restore.
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
    DUNGEON.with_level(|level| level.room_bounds(room))
}

/// Absolute door-exit coordinates for the room/passage `reference` points to.
#[inline]
pub fn room_exits(room: Option<usize>) -> Vec<IVec2> {
    DUNGEON.with_level(|level| level.room_exits(room))
}

/// Absolute door-exit coordinates for a passage index.
#[inline]
pub fn passage_exits(passage: Option<usize>) -> Vec<IVec2> {
    DUNGEON.with_level(|level| level.passage_exits(passage))
}

/// Arena handles for the floor items of the live level, head first.
///
/// This is the pointer-free accessor preferred by new code; callers look the
/// object up through [`crate::item::arena::OBJECTS`] when needed.
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
    use super::with_current_level;

    #[test]
    fn current_level_initializes_once() {
        // A mutation through the mutable view is visible to a later immutable
        // read, confirming both scoped views observe the same lazily-created
        // level owned by the `DUNGEON` singleton.
        crate::game::set_current_depth(7);
        assert_eq!(with_current_level(|level| level.depth), 7);
    }
}
