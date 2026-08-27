//! Corridor/passage digging helpers, Rust-side per-cell flags, and the C
//! global mirroring.
//!
//! Level generation writes per-cell flags into [`Level::flags`] and the door
//! exits of each numbered passage component into [`Level::passage_links`]
//! (see [`Level::mark_passages`] and [`Level::number_passages`]) instead of
//! poking the C `places`/`rooms`/`passages` globals directly. Once the whole
//! level is generated, [`copy_flags_to_c`], [`sync_rooms_to_c`], and
//! [`sync_passages_to_c`] translate those Rust structures into the C arrays
//! the engine consumes. [`add_pass`] continues to read `places` during screen
//! redraw.

use std::os::raw::{c_char, c_int, c_uint};

use glam::IVec2;

use crate::draw::place_at;
use crate::player::{CCoord, CPlace, CRoom, CThingMonster};

use super::ffitools::{DOOR, F_PASS, F_PNUM, F_REAL, F_SEEN, PASSAGE};
use super::level::{Level, LEVEL_HEIGHT, LEVEL_WIDTH};
use super::roomgraph::MAX_ROOMS;

/// Size of the C `passages` room array (also the cap on numbered components).
pub(crate) const MAX_PASSAGES: usize = 13;
/// Max exits writeable into one C `r_exit` array.
pub(crate) const MAX_EXITS: usize = 12;
/// Width of the playable C `places` screen.
pub(crate) const SCREEN_COLS: c_int = 80;
/// Height of the playable C `places` screen.
pub(crate) const SCREEN_LINES: c_int = 24;

unsafe extern "C" {
    static mut rooms: [CRoom; MAX_ROOMS];
    static mut passages: [CRoom; MAX_PASSAGES];
    static mut places: [CPlace; 32 * 80];

    fn r#move(y: c_int, x: c_int) -> c_int;
    fn addch(ch: c_uint) -> c_int;
    fn standout() -> c_int;
    fn standend() -> c_int;
}

/// A corridor connecting two rooms.
///
/// Mirrors the [`Room`](super::rooms::Room) abstraction: a bounding box
/// (`position`/`size`) plus the relative coordinates of every passage tile
/// and entry point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passage {
    pub position: IVec2,
    pub size: IVec2,
    /// Coordinates of every passage tile, relative to `position`.
    pub tiles: Vec<IVec2>,
    /// Coordinates of the doors joining adjacent rooms, relative to `position`.
    pub entry_points: Vec<IVec2>,
}

/// Door exits of one numbered passage component.
///
/// Produced by [`Level::number_passages`] and mirrored to one slot of the C
/// `passages` array (a `CRoom` used as an exit table) by
/// [`sync_passages_to_c`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PassageLinks {
    /// Absolute map coordinates of the component's doorways.
    pub exits: Vec<IVec2>,
}

/// Geometric plan of the L-shaped corridor between two rooms.
///
/// Produced by [`Level::plan_corridor`] and consumed by [`Level::dig_corridor`]
/// to register the corridor's doors and lay its tiles. All coordinates are
/// absolute map coordinates in the level's Rust tile grid.
pub(crate) struct CorridorPlan {
    /// Index of the room the corridor leaves (the lower room index).
    pub(crate) base_room: usize,
    /// Index of the room the corridor enters (the room paired with `base_room`).
    pub(crate) partner_room: usize,
    /// Per-cell step of the straight run: `(0, 1)` for vertical corridors,
    /// `(1, 0)` for horizontal ones.
    pub(crate) step: IVec2,
    /// Entry point on `base_room`'s boundary.
    pub(crate) start: IVec2,
    /// Exit point on `partner_room`'s boundary.
    pub(crate) end: IVec2,
    /// Number of cells laid along `step` before the turn.
    pub(crate) distance: i32,
    /// Per-cell step of the perpendicular turn.
    pub(crate) turn_step: IVec2,
    /// Number of cells laid along `turn_step`.
    pub(crate) turn_distance: i32,
    /// Position along the straight run at which the turn begins.
    pub(crate) turn_spot: i32,
}

/// Whether `ch`/`flags` describe a doorway or a hidden (non-real) wall.
///
/// [`add_pass`] treats `+` doors and `-`/`|` walls whose `F_REAL` bit has been
/// cleared as part of the passage network.
#[inline]
fn is_door_or_hidden(ch: c_char, flags: c_char) -> bool {
    ch == DOOR
        || ((flags as u8 & F_REAL as u8) == 0 && (ch == b'|' as c_char || ch == b'-' as c_char))
}

/// Draw all passage and door tiles for the current level (FFI export).
///
/// Iterates the C `places` grid and redraws every cell marked as a passage
/// or a door, marking it seen (`F_SEEN`). Exported with `#[no_mangle]` so
/// the C engine can call it during screen redraw.
/// Uses globals: `places`.
#[no_mangle]
pub unsafe extern "C" fn add_pass() {
    for y in 1..SCREEN_LINES - 1 {
        for x in 0..SCREEN_COLS {
            let pp = place_at((&raw mut places) as *mut CPlace, y, x);
            let flags = (*pp).p_flags;
            let ch = (*pp).p_ch;
            if (flags as u8 & F_PASS as u8) != 0 || is_door_or_hidden(ch, flags) {
                let mut out_ch = ch;
                if (flags as u8 & F_PASS as u8) != 0 {
                    out_ch = PASSAGE;
                }
                (*pp).p_flags = ((*pp).p_flags as u8 | F_SEEN as u8) as c_char;
                r#move(y, x);
                if !(*pp).p_monst.is_null() {
                    let monst = (*pp).p_monst as *mut CThingMonster;
                    (*monst).t_oldch = (*pp).p_ch;
                } else if (flags as u8 & F_REAL as u8) != 0 {
                    addch(out_ch as c_uint);
                } else {
                    standout();
                    addch(if (flags as u8 & F_PASS as u8) != 0 { PASSAGE as c_uint } else { DOOR as c_uint });
                    standend();
                }
            }
        }
    }
}

/// Copy the Rust flag grids of `level` into the C `places` grid's `p_flags`.
///
/// Reconstructs the legacy flat bits after level generation has finished
/// digging rooms, doors, and passages, so no C globals are touched while
/// generating. Cell flags are the OR of the passage component number
/// (`passnum`), `F_PASS`, `F_SEEN`, and `F_REAL` (except where a real wall
/// was hidden).
/// Uses globals: `places`.
pub(crate) unsafe fn copy_flags_to_c(level: &Level) {
    for y in 0..LEVEL_HEIGHT {
        for x in 0..LEVEL_WIDTH {
            let idx = y * LEVEL_WIDTH + x;
            let mut flags = level.flags.passnum[idx] & F_PNUM as u8;
            if level.flags.passage[idx] {
                flags |= F_PASS as u8;
            }
            if level.flags.seen[idx] {
                flags |= F_SEEN as u8;
            }
            if level.flags.real[idx] {
                flags |= F_REAL as u8;
            }
            let pp = place_at((&raw mut places) as *mut CPlace, y as c_int, x as c_int);
            (*pp).p_flags = flags as c_char;
        }
    }
}

/// Copy `level`'s rooms' Rust-side entry points into the C `rooms` array so
/// the engine can follow room exits.
/// Uses globals: `rooms`.
pub(crate) unsafe fn sync_rooms_to_c(level: &Level) {
    for (i, room) in level.rooms.iter().enumerate() {
        let rp = &raw mut rooms[i];
        (*rp).r_nexits = room.entry_point_count.min(MAX_EXITS as i32);
        for (j, ep) in room.entry_points.iter().take(MAX_EXITS).enumerate() {
            let abs = *ep + room.position;
            (*rp).r_exit[j] = CCoord { x: abs.x, y: abs.y };
        }
    }
}

/// Copy `level`'s numbered passage components into the C `passages` array.
///
/// Each Rust [`PassageLinks`] entry (produced by [`Level::number_passages`])
/// is written into the matching `passages[]` slot: `r_nexits` and the
/// absolute coordinates of its doorways.
/// Uses globals: `passages`.
pub(crate) unsafe fn sync_passages_to_c(level: &Level) {
    for rp in &mut passages[..MAX_PASSAGES] {
        rp.r_nexits = 0;
    }
    for (i, links) in level.passage_links.iter().enumerate().take(MAX_PASSAGES) {
        let rp = &mut passages[i];
        rp.r_nexits = links.exits.len().min(MAX_EXITS) as c_int;
        for (j, exit) in links.exits.iter().take(MAX_EXITS).enumerate() {
            rp.r_exit[j] = CCoord { x: exit.x, y: exit.y };
        }
    }
}