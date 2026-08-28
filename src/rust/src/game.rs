//! Game-state sub-module.
//!
//! Owns the process-wide game state that the legacy C engine previously kept
//! in globals:
//!
//! * the **current level** — the [`Level`] singleton (tile map, flags, rooms,
//!   passages) for the live dungeon depth;
//! * the **places grid** — a Rust-owned `places` array replacing the C
//!   `PLACE places[MAXLINES*MAXCOLS]` global; every legacy extern
//!   `static mut places: [CPlace; 32*80]` declaration binds to this symbol;
//! * the **monster map** — a dedicated [`MONSTERS`] per-cell monster
//!   occupancy array that backs the `p_monst` column of `places`.
//!
//! All access to the level map, the display grid, and monster placement flows
//! through this module instead of the raw C symbol (which no longer exists).

use std::os::raw::{c_char, c_int};

use crate::level::{Level, LEVEL_HEIGHT, LEVEL_WIDTH};
use crate::player::{CPlace, CThing};

/// Index of a grid cell, matching the legacy C layout `&places[(x<<5)+y]`.
#[inline]
fn cell_index(y: c_int, x: c_int) -> usize {
    ((x as usize) << 5) + (y as usize)
}

/// The legacy `PLACE` grid, now owned by Rust.
///
/// Previously defined in `extern.c` as `PLACE places[MAXLINES*MAXCOLS]`, this
/// array is the single source of truth for each cell's display glyph
/// (`p_ch`), flat flags (`p_flags`), and the mirrored monster pointer
/// (`p_monst`). The type stays `crate::player::CPlace` so every existing
/// `extern "C" { static mut places: [CPlace; 32 * 80] }` declaration links
/// against this storage unchanged.
#[no_mangle]
pub static mut places: [CPlace; LEVEL_HEIGHT * LEVEL_WIDTH] =
    [CPlace {
        p_ch: b' ' as c_char,
        p_flags: 0,
        p_monst: std::ptr::null_mut(),
    }; LEVEL_HEIGHT * LEVEL_WIDTH];

/// Dense per-cell monster occupancy map.
///
/// Replaces the conceptual `p_monst` column of the old C `places` global with
/// an explicit map. Uses the same `(x<<5)+y` indexing as the grid so the save
/// serializer can iterate both arrays in lockstep. `set_monster` keeps the
/// `p_monst` field of [`places`] in sync, preserving the legacy save format.
pub static mut MONSTERS: [*mut CThing; LEVEL_HEIGHT * LEVEL_WIDTH] =
    [std::ptr::null_mut(); LEVEL_HEIGHT * LEVEL_WIDTH];

/// Read the monster at `(y, x)`, or null.
#[inline]
pub unsafe fn monster_at(y: c_int, x: c_int) -> *mut CThing {
    MONSTERS[cell_index(y, x)]
}

/// Place `tp` at `(y, x)` on the monster map and mirror it into [`places`].
#[inline]
pub unsafe fn set_monster(y: c_int, x: c_int, tp: *mut CThing) {
    let i = cell_index(y, x);
    MONSTERS[i] = tp;
    places[i].p_monst = tp;
}

/// Mutable pointer into the [`places`] grid at `(y, x)`.
#[inline]
pub unsafe fn place_at(y: c_int, x: c_int) -> *mut CPlace {
    places.as_mut_ptr().add(cell_index(y, x))
}

/// Write a display glyph into the places grid.
#[inline]
pub unsafe fn set_tile_char(y: c_int, x: c_int, ch: c_char) {
    (*place_at(y, x)).p_ch = ch;
}

/// Read the display glyph at `(y, x)`.
#[inline]
pub unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    (*place_at(y, x)).p_ch
}

/// Read the flat flags at `(y, x)`.
#[inline]
pub unsafe fn flat_at(y: c_int, x: c_int) -> c_char {
    (*place_at(y, x)).p_flags
}

/// Read the monster map at `(y, x)` (equivalent to [`monster_at`]).
#[inline]
pub unsafe fn moat_at(y: c_int, x: c_int) -> *mut CThing {
    monster_at(y, x)
}

/// Place a monster on the monster map (and sync the places grid).
#[inline]
pub unsafe fn set_moat_at(y: c_int, x: c_int, tp: *mut CThing) {
    set_monster(y, x, tp);
}

/// Reset the places grid and the monster map for a fresh level.
pub unsafe fn clear_level() {
    for cell in places.iter_mut() {
        cell.p_ch = b' ' as c_char;
        cell.p_flags = 0;
        cell.p_monst = std::ptr::null_mut();
    }
    MONSTERS.fill(std::ptr::null_mut());
}

/// Process-wide singleton for the live dungeon level.
///
/// The canonical holder of the current level. `level::level` forwards its
/// `current_level_mut` here so the whole crate keeps using the same singleton
/// while ownership lives in the game-state module.
pub static mut CURRENT_LEVEL: Option<Level> = None;

/// The live level owner. Initializes the singleton on first use.
#[inline]
pub unsafe fn current_level_mut() -> &'static mut Level {
    if CURRENT_LEVEL.is_none() {
        CURRENT_LEVEL = Some(Level::new());
    }
    CURRENT_LEVEL.as_mut().unwrap()
}

/// Immutable access to the live level.
#[inline]
pub unsafe fn current_level() -> &'static Level {
    current_level_mut()
}

/// Convenience alias for the crate-wide level size constants.
pub use crate::level::{LEVEL_HEIGHT as GAME_HEIGHT, LEVEL_WIDTH as GAME_WIDTH};
