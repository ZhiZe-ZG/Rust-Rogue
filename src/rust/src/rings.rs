use std::os::raw::{c_char, c_int, c_uchar};

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

#[repr(C)]
pub struct CCoord {
    pub x: c_int,
    pub y: c_int,
}

#[repr(C)]
pub struct CThing {
    pub l_next: *mut CThing,
    pub l_prev: *mut CThing,
    pub o_type: c_int,
    pub o_pos: CCoord,
    pub o_text: *mut c_char,
    pub o_launch: c_int,
    pub o_packch: c_char,
    pub o_damage: [c_char; 8],
    pub o_hurldmg: [c_char; 8],
    pub o_count: c_int,
    pub o_which: c_int,
    pub o_hplus: c_int,
    pub o_dplus: c_int,
    pub o_arm: c_int,
    pub o_flags: c_int,
    pub o_group: c_int,
    pub o_label: *mut c_char,
}

unsafe extern "C" {
    static mut cur_ring: [*mut CThing; 2];
    static mut terse: c_uchar;
    static mut mpos: c_int;

    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn is_current(obj: *mut CThing) -> c_uchar;
    fn msg(fmt: *const c_char, ...);
    fn addmsg(fmt: *const c_char, ...);
    fn inv_name(obj: *mut CThing, drop: c_uchar) -> *mut c_char;
    fn chg_str(amt: c_int);
    fn invis_on();
    fn aggravate();
    fn dropcheck(obj: *mut CThing) -> c_uchar;
    fn readchar() -> c_int;
    fn rnd(range: c_int) -> c_int;
    fn num(n1: c_int, n2: c_int, obj_type: c_char) -> *mut c_char;
    fn snprintf(s: *mut c_char, n: usize, fmt: *const c_char, ...) -> c_int;
}

static mut RING_NUM_BUF: [c_char; 10] = [0; 10];

/// Prompts for a ring and equips it on an available hand, applying immediate ring effects.
#[no_mangle]
pub unsafe extern "C" fn ring_on() {
    let obj = get_item(c"put on".as_ptr(), RING_TYPE);
    if obj.is_null() {
        return;
    }
    if (*obj).o_type != RING_TYPE {
        if terse == 0 {
            msg(c"it would be difficult to wrap that around a finger".as_ptr());
        } else {
            msg(c"not a ring".as_ptr());
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
            msg(c"you already have a ring on each hand".as_ptr());
        } else {
            msg(c"wearing two".as_ptr());
        }
        return;
    };

    cur_ring[ring] = obj;

    match (*obj).o_which {
        R_ADDSTR => chg_str((*obj).o_arm),
        R_SEEINVIS => invis_on(),
        R_AGGR => aggravate(),
        _ => {}
    }

    if terse == 0 {
        addmsg(c"you are now wearing ".as_ptr());
    }
    msg(c"%s (%c)".as_ptr(), inv_name(obj, 1), (*obj).o_packch as c_int);
}

/// Removes a worn ring from the chosen hand after passing drop constraints.
#[no_mangle]
pub unsafe extern "C" fn ring_off() {
    let ring = if cur_ring[LEFT].is_null() && cur_ring[RIGHT].is_null() {
        if terse != 0 {
            msg(c"no rings".as_ptr());
        } else {
            msg(c"you aren't wearing any rings".as_ptr());
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
        msg(c"not wearing such a ring".as_ptr());
        return;
    }

    if dropcheck(obj) != 0 {
        msg(c"was wearing %s(%c)".as_ptr(), inv_name(obj, 1), (*obj).o_packch as c_int);
    }
}

/// Asks which hand the player means and returns LEFT, RIGHT, or -1 on escape.
#[no_mangle]
pub unsafe extern "C" fn gethand() -> c_int {
    loop {
        if terse != 0 {
            msg(c"left or right ring? ".as_ptr());
        } else {
            msg(c"left hand or right hand? ".as_ptr());
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
            msg(c"L or R".as_ptr());
        } else {
            msg(c"please type L or R".as_ptr());
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

    let which = (*ring).o_which as usize;
    if which >= USES.len() {
        return 0;
    }

    let mut eat = USES[which];
    if eat < 0 {
        eat = if rnd(-eat) == 0 { 1 } else { 0 };
    }
    if (*ring).o_which == R_DIGEST {
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
    if ((*obj).o_flags & ISKNOW) == 0 {
        return c"".as_ptr() as *mut c_char;
    }

    match (*obj).o_which {
        R_PROTECT | R_ADDSTR | R_ADDDAM | R_ADDHIT => {
            let _ = snprintf(
                (&raw mut RING_NUM_BUF) as *mut c_char,
                10,
                c" [%s]".as_ptr(),
                num((*obj).o_arm, 0, RING_TYPE as c_char),
            );
            (&raw mut RING_NUM_BUF) as *mut c_char
        }
        _ => c"".as_ptr() as *mut c_char,
    }
}
