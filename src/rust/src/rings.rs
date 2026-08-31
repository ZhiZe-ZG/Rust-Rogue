use crate::rnd::rnd;
use crate::player::{CThing, CThingObject};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};
use crate::potions::invis_on;

use crate::io::{addmsg_str, msg_str};

const LEFT: usize = 0;
const RIGHT: usize = 1;
const RING_TYPE: c_int = '=' as c_int;
const ESCAPE: u8 = 27;
const ISKNOW: c_int = 0o000002;

const R_PROTECT: c_int = 0;
const R_ADDSTR: c_int = 1;
const R_SEEINVIS: c_int = 4;
const R_AGGR: c_int = 6;
const R_ADDHIT: c_int = 7;
const R_ADDDAM: c_int = 8;
const R_DIGEST: c_int = 10;

const USES: [c_int; 14] = [
    1,  // R_PROTECT
    1,  // R_ADDSTR
    1,  // R_SUSTSTR
    -3, // R_SEARCH
    -5, // R_SEEINVIS
    0,  // R_NOP
    0,  // R_AGGR
    -3, // R_ADDHIT
    -3, // R_ADDDAM
    2,  // R_REGEN
    -2, // R_DIGEST
    0,  // R_TELEPORT
    1,  // R_STEALTH
    1,  // R_SUSTARM
];

unsafe extern "C" {
    static mut cur_ring: [*mut CThing; 2];
    static mut terse: c_uchar;
    static mut mpos: c_int;

    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn is_current(obj: *mut CThing) -> c_uchar;
    fn inv_name(obj: *mut CThing, drop: c_uchar) -> *mut c_char;
    fn chg_str(amt: c_int);
    fn aggravate();
    fn dropcheck(obj: *mut CThing) -> c_uchar;
    fn readchar() -> c_int;
    fn num(n1: c_int, n2: c_int, obj_type: c_char) -> *mut c_char;
    fn snprintf(s: *mut c_char, n: usize, fmt: *const c_char, ...) -> c_int;
}

static mut RING_NUM_BUF: [c_char; 10] = [0; 10];

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

/// Prompts for a ring and equips it on an available hand, applying immediate ring effects.
#[no_mangle]
pub unsafe extern "C" fn ring_on() {
    let obj = get_item(c"put on".as_ptr(), RING_TYPE);
    if obj.is_null() {
        return;
    }
    if (*thing_o(obj)).o_type != RING_TYPE {
        if terse == 0 {
            msg_str("it would be difficult to wrap that around a finger");
        } else {
            msg_str("not a ring");
        }
        return;
    }

    if is_current(obj) != 0 {
        return;
    }

    let ring = if cur_ring[LEFT].is_null() && cur_ring[RIGHT].is_null() {
        let hand = gethand();
        if hand < 0 {
            return;
        }
        hand as usize
    } else if cur_ring[LEFT].is_null() {
        LEFT
    } else if cur_ring[RIGHT].is_null() {
        RIGHT
    } else {
        if terse == 0 {
            msg_str("you already have a ring on each hand");
        } else {
            msg_str("wearing two");
        }
        return;
    };

    cur_ring[ring] = obj;

    match (*thing_o(obj)).o_which {
        R_ADDSTR => chg_str((*thing_o(obj)).o_arm),
        R_SEEINVIS => invis_on(),
        R_AGGR => aggravate(),
        _ => {}
    }

    if terse == 0 {
        addmsg_str("you are now wearing ");
    }
    msg_str(&format!(
        "{} ({})",
        CStr::from_ptr(inv_name(obj, 1)).to_string_lossy(),
        (*thing_o(obj)).o_packch as u8 as char,
    ));
}

/// Removes a worn ring from the chosen hand after passing drop constraints.
#[no_mangle]
pub unsafe extern "C" fn ring_off() {
    let ring = if cur_ring[LEFT].is_null() && cur_ring[RIGHT].is_null() {
        if terse != 0 {
            msg_str("no rings");
        } else {
            msg_str("you aren't wearing any rings");
        }
        return;
    } else if cur_ring[LEFT].is_null() {
        RIGHT
    } else if cur_ring[RIGHT].is_null() {
        LEFT
    } else {
        let hand = gethand();
        if hand < 0 {
            return;
        }
        hand as usize
    };

    mpos = 0;
    let obj = cur_ring[ring];
    if obj.is_null() {
        msg_str("not wearing such a ring");
        return;
    }

    if dropcheck(obj) != 0 {
        msg_str(&format!(
            "was wearing {}({})",
            CStr::from_ptr(inv_name(obj, 1)).to_string_lossy(),
            (*thing_o(obj)).o_packch as u8 as char,
        ));
    }
}

/// Asks which hand the player means and returns LEFT, RIGHT, or -1 on escape.
#[no_mangle]
pub unsafe extern "C" fn gethand() -> c_int {
    loop {
        if terse != 0 {
            msg_str("left or right ring? ");
        } else {
            msg_str("left hand or right hand? ");
        }

        let c = readchar() as u8;
        if c == ESCAPE {
            return -1;
        }

        mpos = 0;
        if c == b'l' || c == b'L' {
            return LEFT as c_int;
        }
        if c == b'r' || c == b'R' {
            return RIGHT as c_int;
        }

        if terse != 0 {
            msg_str("L or R");
        } else {
            msg_str("please type L or R");
        }
    }
}

/// Computes per-turn food impact for the ring on the given hand.
#[no_mangle]
pub unsafe extern "C" fn ring_eat(hand: c_int) -> c_int {
    let hand_idx = hand as usize;
    if hand_idx > RIGHT {
        return 0;
    }

    let ring = cur_ring[hand_idx];
    if ring.is_null() {
        return 0;
    }

    let which = (*thing_o(ring)).o_which as usize;
    if which >= USES.len() {
        return 0;
    }

    let mut eat = USES[which];
    if eat < 0 {
        eat = if rnd(-eat) == 0 { 1 } else { 0 };
    }
    if (*thing_o(ring)).o_which == R_DIGEST {
        eat = -eat;
    }
    eat
}

/// Returns bracketed ring bonus text for known stat-modifier rings.
#[no_mangle]
pub unsafe extern "C" fn ring_num(obj: *mut CThing) -> *mut c_char {
    if obj.is_null() {
        return c"".as_ptr() as *mut c_char;
    }
    if ((*thing_o(obj)).o_flags & ISKNOW) == 0 {
        return c"".as_ptr() as *mut c_char;
    }

    match (*thing_o(obj)).o_which {
        R_PROTECT | R_ADDSTR | R_ADDDAM | R_ADDHIT => {
            let _ = snprintf(
                (&raw mut RING_NUM_BUF) as *mut c_char,
                10,
                c" [%s]".as_ptr(),
                num((*thing_o(obj)).o_arm, 0, RING_TYPE as c_char),
            );
            (&raw mut RING_NUM_BUF) as *mut c_char
        }
        _ => c"".as_ptr() as *mut c_char,
    }
}
