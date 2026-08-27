use crate::rnd::rnd;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::draw::place_at;
use crate::player::{CCoord, CPlace, CThing, CThingMonster, CThingObject};

const LEFT: usize = 0;
const RIGHT: usize = 1;

const TRAP: c_char = b'^' as c_char;
const FLOOR: c_char = b'.' as c_char;
const DOOR: c_char = b'+' as c_char;

const ISLEVIT: c_short = 0o0000010;
const ISRUN: c_short = 0o020000;

const F_SEEN: c_char = 0x40u8 as c_char;
const F_TMASK: c_char = 0x07u8 as c_char;

const T_DOOR: c_char = 0;
const T_ARROW: c_char = 1;
const T_SLEEP: c_char = 2;
const T_BEAR: c_char = 3;
const T_TELEP: c_char = 4;
const T_DART: c_char = 5;
const T_RUST: c_char = 6;
const T_MYST: c_char = 7;

const R_SUSTSTR: c_int = 2;
const ARROW: c_int = 3;
const VS_POISON: c_int = 0;

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

#[repr(C)]
pub union CThingAlias {
    pub t: CThingMonster,
    pub o: CThingObject,
}

unsafe extern "C" {
    static mut running: c_uchar;
    static mut count: c_int;
    static mut no_command: c_int;
    static mut no_move: c_int;
    static mut level: c_int;
    static mut cNCOLORS: c_int;
    static mut rainbow: [*const c_char; 27];
    static mut places: [CPlace; 32 * 80];
    static mut player: CThing;
    static mut cur_armor: *mut CThing;
    static mut cur_ring: [*mut CThing; 2];

    fn msg(fmt: *const c_char, ...);
    fn roll(num: c_int, sides: c_int) -> c_int;
    fn swing(at_lvl: c_int, op_arm: c_int, wplus: c_int) -> c_int;
    fn save(which: c_int) -> c_int;
    fn new_item() -> *mut CThing;
    fn init_weapon(weap: *mut CThing, which: c_int);
    fn fall(obj: *mut CThing, pr: c_uchar);
    fn teleport();
    fn flush_type();
    fn chg_str(amt: c_int);
    fn new_level();
    fn rust_armor(arm: *mut CThing);
    fn death(thing: c_char) -> !;
    fn spread(nm: c_int) -> c_int;
    fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int;
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
unsafe fn hero() -> CCoord {
    (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn ring_is(which: usize, ring_type: c_int) -> bool {
    let ring = cur_ring[which];
    !ring.is_null() && (*thing_o(ring)).o_which == ring_type
}

#[no_mangle]
pub unsafe extern "C" fn be_trapped(tc: *mut CCoord) -> c_char {
    let place = place_at((&raw mut places) as *mut CPlace, (*tc).y, (*tc).x);
    let trap = (*place).p_flags & F_TMASK;

    if ((*thing_t(&raw mut player)).t_flags & ISLEVIT) != 0 {
        return T_RUST;
    }

    running = FALSE;
    count = FALSE as c_int;
    (*place).p_ch = TRAP;
    (*place).p_flags |= F_SEEN;

    match trap {
        T_DOOR => {
            level += 1;
            new_level();
            msg(c"you fell into a trap!".as_ptr());
        }
        T_BEAR => {
            no_move += spread(3);
            msg(c"you are caught in a bear trap".as_ptr());
        }
        T_MYST => {
            match rnd(11) {
                0 => msg(c"you are suddenly in a parallel dimension".as_ptr()),
                1 => msg(c"the light in here suddenly seems %s".as_ptr(), rainbow[rnd(cNCOLORS) as usize]),
                2 => msg(c"you feel a sting in the side of your neck".as_ptr()),
                3 => msg(c"multi-colored lines swirl around you, then fade".as_ptr()),
                4 => msg(c"a %s light flashes in your eyes".as_ptr(), rainbow[rnd(cNCOLORS) as usize]),
                5 => msg(c"a spike shoots past your ear!".as_ptr()),
                6 => msg(c"%s sparks dance across your armor".as_ptr(), rainbow[rnd(cNCOLORS) as usize]),
                7 => msg(c"you suddenly feel very thirsty".as_ptr()),
                8 => msg(c"you feel time speed up suddenly".as_ptr()),
                9 => msg(c"time now seems to be going slower".as_ptr()),
                10 => msg(c"you pack turns %s!".as_ptr(), rainbow[rnd(cNCOLORS) as usize]),
                _ => {}
            }
        }
        T_SLEEP => {
            no_command += spread(5);
            (*thing_t(&raw mut player)).t_flags &= !ISRUN;
            msg(c"a strange white mist envelops you and you fall asleep".as_ptr());
        }
        T_ARROW => {
            let stats = &mut (*thing_t(&raw mut player)).t_stats;
            if swing(stats.s_lvl - 1, stats.s_arm, 1) != 0 {
                stats.s_hpt -= roll(1, 6);
                if stats.s_hpt <= 0 {
                    msg(c"an arrow killed you".as_ptr());
                    death(b'a' as c_char);
                } else {
                    msg(c"oh no! An arrow shot you".as_ptr());
                }
            } else {
                let arrow = new_item();
                init_weapon(arrow, ARROW);
                (*thing_o(arrow)).o_count = 1;
                (*thing_o(arrow)).o_pos = hero();
                fall(arrow, FALSE);
                msg(c"an arrow shoots past you".as_ptr());
            }
        }
        T_TELEP => {
            teleport();
            mvaddch((*tc).y, (*tc).x, TRAP as c_uint);
        }
        T_DART => {
            let stats = &mut (*thing_t(&raw mut player)).t_stats;
            if swing(stats.s_lvl + 1, stats.s_arm, 1) == 0 {
                msg(c"a small dart whizzes by your ear and vanishes".as_ptr());
            } else {
                stats.s_hpt -= roll(1, 4);
                if stats.s_hpt <= 0 {
                    msg(c"a poisoned dart killed you".as_ptr());
                    death(b'd' as c_char);
                }
                if !ring_is(LEFT, R_SUSTSTR) && !ring_is(RIGHT, R_SUSTSTR) && save(VS_POISON) == 0 {
                    chg_str(-1);
                }
                msg(c"a small dart just hit you in the shoulder".as_ptr());
            }
        }
        T_RUST => {
            msg(c"a gush of water hits you on the head".as_ptr());
            rust_armor(cur_armor);
        }
        _ => {}
    }

    flush_type();
    trap
}