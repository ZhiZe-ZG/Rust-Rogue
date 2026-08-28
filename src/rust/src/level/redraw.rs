//! Screen redraw of the C `places` grid.
//!
//! `add_pass` is the only `#[no_mangle]` export here — the C engine calls it
//! during its screen refresh to redraw every passage/door tile. The small
//! `chat_at`/`winat`/`chat_at_mut` helpers wrap `place_at` over the raw C
//! `places` global.

use std::os::raw::{c_char, c_int, c_uint};

use crate::draw::place_at;
use crate::player::{CPlace, CThingMonster};

use super::ffitools::{DOOR, F_PASS, F_REAL, F_SEEN, PASSAGE};
use super::passages::{SCREEN_COLS, SCREEN_LINES};
use crate::game::places;
use super::symbols::{addch, r#move, standend, standout, thing_o};

/// Read a cell's on-screen character from the C `places` grid.
#[inline]
pub(crate) unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_ch
}

/// Read the cell's visible character: a monster's pack glyph if one stands
/// here, otherwise the floor/passage character.
#[inline]
pub(crate) unsafe fn winat(y: c_int, x: c_int) -> c_char {
    let tp = (*place_at((&raw mut places) as *mut CPlace, y, x)).p_monst;
    if tp.is_null() {
        chat_at(y, x)
    } else {
        (*thing_o(tp)).o_packch
    }
}

/// Mutable access to a cell's character slot.
#[inline]
pub(crate) unsafe fn chat_at_mut(y: c_int, x: c_int) -> *mut c_char {
    &raw mut (*place_at((&raw mut places) as *mut CPlace, y, x)).p_ch
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
                    addch(if (flags as u8 & F_PASS as u8) != 0 {
                        PASSAGE as c_uint
                    } else {
                        DOOR as c_uint
                    });
                    standend();
                }
            }
        }
    }
}