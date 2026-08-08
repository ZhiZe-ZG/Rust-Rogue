use std::os::raw::{c_char, c_int, c_uchar};

use crate::player::{CCoord, CPlace, CThing, CThingMonster, CThingObject};
use crate::rnd::rnd;

const SCROLL: c_char = b'?' as c_char;
const S_SCARE: c_int = 10;

unsafe extern "C" {
    static mut places: [CPlace; 32 * 80];
    static mut lvl_obj: *mut CThing;

    fn diag_ok(sp: *mut CCoord, ep: *mut CCoord) -> c_uchar;
    fn step_ok(ch: c_int) -> c_int;
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
unsafe fn place_at(y: c_int, x: c_int) -> *mut CPlace {
    places.as_mut_ptr().add(((x as usize) << 5) + (y as usize))
}

#[inline]
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    (*place_at(y, x)).p_ch
}

#[inline]
unsafe fn winat(y: c_int, x: c_int) -> c_char {
    let tp = (*place_at(y, x)).p_monst;
    if tp.is_null() {
        chat_at(y, x)
    } else {
        (*thing_o(tp)).o_packch
    }
}

/// Persistent return coordinate, mirroring C's `static coord ret`.
static mut RET: CCoord = CCoord { x: 0, y: 0 };

/// rndmove:
/// Move in a random direction if the monster/person is confused.
#[no_mangle]
pub unsafe extern "C" fn rndmove(who: *mut CThing) -> *mut CCoord {
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
    if step_ok(ch as c_int) == 0 {
        RET = pos;
        return &raw mut RET;
    }

    // Refuse to step on a scroll of scare monster
    if ch == SCROLL {
        let mut obj = lvl_obj;
        while !obj.is_null() {
            if RET.y == (*thing_o(obj)).o_pos.y && RET.x == (*thing_o(obj)).o_pos.x {
                break;
            }
            obj = (*thing_o(obj)).l_next;
        }
        if !obj.is_null() && (*thing_o(obj)).o_which == S_SCARE {
            RET = pos;
            return &raw mut RET;
        }
    }

    &raw mut RET
}
