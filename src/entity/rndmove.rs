//! Random movement for confused or otherwise disoriented monsters.
//!
//! Split out from the movement logic originally found in `src/c/move.c`.
use glam::IVec2;
use std::os::raw::{c_char, c_int};

use crate::entity::chase::diag_ok;
use crate::entity::player::{Thing, ThingObject};
use crate::item::scrolls::ScrollType;
use crate::rnd::rnd;

const SCROLL: c_char = b'?' as c_char;

#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

/// Persistent return coordinate, mirroring C's `static coord ret`.
static mut RET: IVec2 = IVec2 { x: 0, y: 0 };

/// rndmove:
/// Pick a random move for a confused actor starting from `pos`.
///
/// Standing still is a valid outcome. Returns the chosen coordinate; the caller
/// owns it, so no pointer is stored in a shared slot.
pub unsafe fn rndmove_from(pos: IVec2) -> IVec2 {
    let mut chosen = IVec2 {
        y: pos.y + rnd(3) - 1,
        x: pos.x + rnd(3) - 1,
    };

    // Standing still is a valid outcome
    if chosen.y == pos.y && chosen.x == pos.x {
        return chosen;
    }

    let mut pos_copy = pos;
    if diag_ok(&raw mut pos_copy, &raw mut chosen) == 0 {
        return pos;
    }

    if !crate::game::cell_is_walkable(chosen.y, chosen.x) {
        return pos;
    }

    // Refuse to step on a scroll of scare monster
    let mut obj = crate::game::with_current_level(|level| level.items.head());
    while !obj.is_null() {
        if chosen.y == (*thing_o(obj)).o_pos.y && chosen.x == (*thing_o(obj)).o_pos.x {
            break;
        }
        obj = crate::entity::player::thing_next(obj);
    }
    if !obj.is_null() && (*thing_o(obj)).o_which == ScrollType::Scare as c_int {
        return pos;
    }

    chosen
}

/// rndmove:
/// Move in a random direction if the monster/person is confused.
#[no_mangle]
pub unsafe extern "C" fn rndmove(who: *mut Thing) -> *mut IVec2 {
    let pos = (*crate::entity::player::thing_t(who)).t_pos;
    RET = rndmove_from(pos);
    &raw mut RET
}