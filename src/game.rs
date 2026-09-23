//! Game-state sub-module.
//!
//! Owns the process-wide game state that the legacy C engine previously kept
//! in globals:
//!
//! * the **current level** — the [`Level`] singleton (tile map, flags, rooms,
//!   passages, floor items, and per-cell monster occupancy) for the live
//!   dungeon depth;
//! * the **current equipment** — non-owning pointers to the armor, rings, and
//!   weapon selected from the player's pack.
//!
//! Cell display glyphs and flat flags are no longer cached in a legacy per-cell
//! grid (the `p_ch`/`p_flags` members were removed); every access goes through
//! `crate::draw`, which computes them from the [`Level`] tile map and flag
//! grids on the fly.

use std::os::raw::c_int;
use std::sync::RwLock;

use crate::config::GameConfig;
use crate::entity::player::CThing;
use crate::level::Level;
use glam::IVec2;

/// A non-owning, interior-mutable cell for a raw [`CThing`] pointer.
///
/// The game is single-threaded, but these pointers are shared through
/// `static`s (which must be `Sync`). The raw pointer itself is neither `Send`
/// nor `Sync`, so this wrapper opts in explicitly and exposes lock-guarded
/// access.
struct PtrCell(RwLock<*mut CThing>);

// SAFETY: access to the pointer is always guarded by the inner `RwLock`, and
// the pointer is only dereferenced by the owning (single-threaded) game logic.
unsafe impl Send for PtrCell {}
unsafe impl Sync for PtrCell {}

impl PtrCell {
    const fn new() -> Self {
        Self(RwLock::new(std::ptr::null_mut()))
    }

    #[inline]
    fn get(&self) -> *mut CThing {
        *self.0.read().unwrap_or_else(|poison| poison.into_inner())
    }

    #[inline]
    fn set(&self, ptr: *mut CThing) {
        *self.0.write().unwrap_or_else(|poison| poison.into_inner()) = ptr;
    }
}

/// Non-owning pointers to the objects currently equipped by the player.
pub struct Equipment {
    armor: PtrCell,
    rings: [PtrCell; 2],
    weapon: PtrCell,
}

impl Equipment {
    const EMPTY: Self = Self {
        armor: PtrCell::new(),
        rings: [PtrCell::new(), PtrCell::new()],
        weapon: PtrCell::new(),
    };

    #[inline]
    pub fn armor(&self) -> *mut CThing {
        self.armor.get()
    }

    #[inline]
    pub fn set_armor(&self, armor: *mut CThing) {
        self.armor.set(armor);
    }

    #[inline]
    pub fn left_ring(&self) -> *mut CThing {
        self.rings[0].get()
    }

    #[inline]
    pub fn right_ring(&self) -> *mut CThing {
        self.rings[1].get()
    }

    #[inline]
    pub fn set_left_ring(&self, ring: *mut CThing) {
        self.rings[0].set(ring);
    }

    #[inline]
    pub fn set_right_ring(&self, ring: *mut CThing) {
        self.rings[1].set(ring);
    }

    #[inline]
    pub fn weapon(&self) -> *mut CThing {
        self.weapon.get()
    }

    #[inline]
    pub fn set_weapon(&self, weapon: *mut CThing) {
        self.weapon.set(weapon);
    }
}

/// Current player equipment. Items remain owned by the player's pack.
pub static EQUIPMENT: Equipment = Equipment::EMPTY;

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

/// Read the monster at `(y, x)`, or null.
#[inline]
pub unsafe fn monster_at(y: c_int, x: c_int) -> *mut CThing {
    with_current_level(|level| level.monsters.at(y as usize, x as usize))
}

/// Place `tp` at `(y, x)` in the per-cell monster occupancy map.
#[inline]
pub unsafe fn set_monster(y: c_int, x: c_int, tp: *mut CThing) {
    with_current_level_mut(|level| level.monsters.set(y as usize, x as usize, tp));
}

/// Read the monster map at `(y, x)` (equivalent to [`monster_at`]).
#[inline]
pub unsafe fn moat_at(y: c_int, x: c_int) -> *mut CThing {
    monster_at(y, x)
}

/// Place a monster in the per-cell monster occupancy map.
#[inline]
pub unsafe fn set_moat_at(y: c_int, x: c_int, tp: *mut CThing) {
    set_monster(y, x, tp);
}

/// Clear every cell's monster pointer for a fresh level.
pub unsafe fn clear_level() {
    with_current_level_mut(|level| level.monsters.clear());
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
pub static mut ROOM_GOLD: [IVec2; GameConfig::MAX_ROOMS] = [IVec2::ZERO; GameConfig::MAX_ROOMS];

/// A stable raw pointer to the gold-stash position of `reference` (or null).
///
/// The pointed-to slot lives in the process-wide [`ROOM_GOLD`] array, so it
/// does not borrow the level lock and is safe for chase-target storage.
#[inline]
pub unsafe fn room_gold_ptr(room: Option<usize>) -> *mut IVec2 {
    match room {
        Some(i) if i < GameConfig::MAX_ROOMS => (&raw mut ROOM_GOLD[i]) as *mut IVec2,
        _ => std::ptr::null_mut(),
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

/// Convenience alias for the crate-wide level size constants.
pub const GAME_HEIGHT: usize = GameConfig::LEVEL_HEIGHT;
pub const GAME_WIDTH: usize = GameConfig::LEVEL_WIDTH;

#[cfg(test)]
mod tests {
    use super::{CurrentLevel, Equipment, Level};
    use crate::entity::player::CThing;
    use std::mem::MaybeUninit;

    #[test]
    fn ring_accessors_keep_hands_independent() {
        let equipment = Equipment::EMPTY;
        let mut left = MaybeUninit::<CThing>::uninit();
        let mut right = MaybeUninit::<CThing>::uninit();

        equipment.set_left_ring(left.as_mut_ptr());
        equipment.set_right_ring(right.as_mut_ptr());

        assert_eq!(equipment.left_ring(), left.as_mut_ptr());
        assert_eq!(equipment.right_ring(), right.as_mut_ptr());
    }

    #[test]
    fn current_level_initializes_once() {
        let current_level = CurrentLevel::EMPTY;

        let first = current_level.with_mut(|level| level as *mut Level);
        let second = current_level.with(|level| level as *const Level);

        assert_eq!(first.cast_const(), second);
    }
}
