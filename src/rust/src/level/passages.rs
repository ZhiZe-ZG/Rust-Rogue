//! Corridor/passage digging helpers and C `places` grid mirroring.
//!
//! Mirrors the legacy `passages.c` behavior: flags passage tiles on the C
//! `places` grid ([`sync_passages_to_c`]), numbers contiguous passage
//! networks ([`passnum`]/[`numpass`]), and redraws doors and passages on
//! screen refresh ([`add_pass`]).

use std::os::raw::{c_char, c_int, c_uchar, c_uint};

use glam::IVec2;

use crate::draw::{clear_tile_flag, place_at};
use crate::player::{CCoord, CPlace, CRoom, CThingMonster};

use super::ffitools::{DOOR, F_PASS, F_REAL, PASSAGE};
use super::level::Level;
use super::roomgraph::MAX_ROOMS;
use super::tile::Tile;

/// Size of the C `passages` room array.
const MAXPASS: usize = 13;
/// Width of the on-screen C `places` grid.
const NUMCOLS: c_int = 80;
/// Height of the on-screen C `places` grid.
const NUMLINES: c_int = 24;
/// Max exits writeable into the C `r_exit` array.
const MAX_EXITS: usize = 12;

/// Flat `p_flags` bit holding a passage component number (0-15).
const F_PNUM: c_char = 0x0fu8 as c_char;
/// Flat `p_flags` bit marking a cell as already drawn ([`add_pass`]).
const F_SEEN: c_char = 0x40u8 as c_char;

const FALSE: c_uchar = 0;
const TRUE: c_uchar = 1;

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

/// Number of the passage component currently being scanned by [`passnum`].
static mut PNUM: c_int = 0;

/// Whether the next cell reached by [`numpass`] opens a new component.
static mut NEW_PNUM: c_uchar = FALSE;

unsafe extern "C" {
    static mut rooms: [CRoom; MAX_ROOMS];
    static mut passages: [CRoom; MAXPASS];
    static mut places: [CPlace; 32 * 80];

    fn rnd(range: c_int) -> c_int;
    fn r#move(y: c_int, x: c_int) -> c_int;
    fn addch(ch: c_uint) -> c_int;
    fn standout() -> c_int;
    fn standend() -> c_int;
}

/// Whether `ch`/`flags` describe a doorway or a hidden (non-real) wall.
///
/// Both [`add_pass`] and [`numpass`] treat `+` doors and `-`/`|` walls whose
/// `F_REAL` bit has been cleared as part of the passage network.
#[inline]
fn is_door_or_hidden(ch: c_char, flags: c_char) -> bool {
    ch == DOOR
        || ((flags as u8 & F_REAL as u8) == 0 && (ch == b'|' as c_char || ch == b'-' as c_char))
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

/// Mirror `level.map`'s passage tiles onto the C `places` grid.
///
/// Marks every passage cell with the `F_PASS` flag so [`passnum`] and the
/// C-side screen redraw ([`add_pass`]) can find it. Matching the legacy
/// `putpass`, a cell is occasionally hidden by clearing `F_REAL` so it
/// renders as a wall glyph (`-`/`|`) instead of `#`.
/// Uses globals: `places`.
pub(crate) unsafe fn sync_passages_to_c(level: &Level) {
    let depth = level.depth;
    for y in 0..level.map.height() {
        for x in 0..level.map.width() {
            if !matches!(level.map.get(y, x), Some(Tile::Passage)) {
                continue;
            }
            let pp = place_at((&raw mut places) as *mut CPlace, y as c_int, x as c_int);
            (*pp).p_flags = ((*pp).p_flags as u8 | F_PASS as u8) as c_char;
            if rnd(10) + 1 < depth && rnd(40) == 0 {
                clear_tile_flag(y as c_int, x as c_int, F_REAL);
            } else {
                (*pp).p_ch = PASSAGE;
            }
        }
    }
}

/// Draw all passage and door tiles for the current level (FFI export).
///
/// Iterates the C `places` grid and redraws every cell marked as a passage
/// or a door, marking it seen (`F_SEEN`). Exported with `#[no_mangle]` so
/// the C engine can call it during screen redraw.
/// Uses globals: `places`.
#[no_mangle]
pub unsafe extern "C" fn add_pass() {
    for y in 1..NUMLINES - 1 {
        for x in 0..NUMCOLS {
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

/// Copy `level`'s rooms' Rust-side entry points into the C `rooms` array so
/// that [`passnum`] can flood-fill the passage network from the registered
/// exits.
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

/// Number the passages reachable from every room exit.
///
/// Resets the passage table, then flood-fills from each room exit using
/// [`numpass`]. Every contiguous passage network is assigned a number used
/// to index the C `passages` array.
/// Uses globals: `PNUM`, `NEW_PNUM`, `passages`, `rooms`.
pub(crate) unsafe fn passnum() {
    PNUM = 0;
    NEW_PNUM = FALSE;
    for rp in &mut passages[..MAXPASS] {
        rp.r_nexits = 0;
    }
    for rp in &mut rooms[..MAX_ROOMS] {
        for i in 0..rp.r_nexits as usize {
            NEW_PNUM = TRUE;
            numpass(rp.r_exit[i].y, rp.r_exit[i].x);
        }
    }
}

/// Recursively flood-fill a passage network, numbering its cells.
///
/// Stops at the screen edge, already-numbered cells, or tiles that are
/// neither passages nor doors, then recurses into the four neighbours.
/// Each new contiguous component increments the current passage number and
/// its exits are registered in the C `passages` array.
/// Uses globals: `PNUM`, `NEW_PNUM`, `passages`, `places`.
unsafe fn numpass(y: c_int, x: c_int) {
    if x >= NUMCOLS || x < 0 || y >= NUMLINES || y <= 0 {
        return;
    }

    let pp = place_at((&raw mut places) as *mut CPlace, y, x);
    if (*pp).p_flags as u8 & F_PNUM as u8 != 0 {
        return;
    }
    if NEW_PNUM != 0 {
        PNUM += 1;
        NEW_PNUM = FALSE;
    }

    let ch = (*pp).p_ch;
    let flags = (*pp).p_flags;
    if is_door_or_hidden(ch, flags) {
        let rp = &mut passages[PNUM as usize];
        rp.r_exit[rp.r_nexits as usize].y = y;
        rp.r_exit[rp.r_nexits as usize].x = x;
        rp.r_nexits += 1;
    } else if (flags as u8 & F_PASS as u8) == 0 {
        return;
    }

    (*pp).p_flags = ((*pp).p_flags as u8 | PNUM as u8) as c_char;
    numpass(y + 1, x);
    numpass(y - 1, x);
    numpass(y, x + 1);
    numpass(y, x - 1);
}