//! Random movement for confused or otherwise disoriented monsters.
//!
//! Split out from the movement logic originally found in `src/c/move.c`.
use glam::IVec2;
use std::os::raw::{c_char, c_int, c_uchar};

use crate::entity::chase::diag_ok;
use crate::entity::player::{CThing, CThingMonster, CThingObject};
use crate::item::scrolls::ScrollType;
use crate::level::tile_is_walkable;
use crate::rnd::rnd;

const SCROLL: c_char = b'?' as c_char;

unsafe extern "C" {
}

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn winat(y: c_int, x: c_int) -> c_char {
    crate::draw::winat(y, x)
}

/// Persistent return coordinate, mirroring C's `static coord ret`.
static mut RET: IVec2 = IVec2 { x: 0, y: 0 };

/// rndmove:
/// Move in a random direction if the monster/person is confused.
#[no_mangle]
pub unsafe extern "C" fn rndmove(who: *mut CThing) -> *mut IVec2 {
    let pos = (*thing_t(who)).t_pos;
    RET.y = pos.y + rnd(3) - 1;
    RET.x = pos.x + rnd(3) - 1;

    // Standing still is a valid outcome
    if RET.y == pos.y && RET.x == pos.x {
        return &raw mut RET;
    }

    let mut pos_copy = pos;
    if diag_ok(&raw mut pos_copy, &raw mut RET) == 0 {
        RET = pos;
        return &raw mut RET;
    }

    let ch = winat(RET.y, RET.x);
    if !tile_is_walkable(ch as u8) {
        RET = pos;
        return &raw mut RET;
    }

    // Refuse to step on a scroll of scare monster
    if ch == SCROLL {
        let mut obj = crate::game::with_current_level(|level| level.items.head());
        while !obj.is_null() {
            if RET.y == (*thing_o(obj)).o_pos.y && RET.x == (*thing_o(obj)).o_pos.x {
                break;
            }
            obj = (*thing_o(obj)).l_next;
        }
        if !obj.is_null() && (*thing_o(obj)).o_which == ScrollType::Scare as c_int {
            RET = pos;
            return &raw mut RET;
        }
    }

    &raw mut RET
}
