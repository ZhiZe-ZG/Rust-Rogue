//! Trap handling.
//!
//! [`be_trapped`] applies the effect of the trap under a dungeon cell: arrow
//! darts, teleportation, rusting armor, a fall into a deeper level, and so
//! on. This is a pure Rust API consumed by the movement code in
//! [`crate::player`]; the legacy C ABI is intentionally not retained.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::curses as cur;
use crate::draw;
use crate::io::msg_str;
use crate::player::{CCoord, CThing, CThingMonster, CThingObject};
use crate::rnd::rnd;

const LEFT: usize = 0;
const RIGHT: usize = 1;

const TRAP: c_char = b'^' as c_char;

const ISLEVIT: c_short = 0o0000010;
const ISRUN: c_short = 0o020000;

pub const T_DOOR: c_char = 0;
pub const T_ARROW: c_char = 1;
pub const T_SLEEP: c_char = 2;
pub const T_BEAR: c_char = 3;
pub const T_TELEP: c_char = 4;
pub const T_DART: c_char = 5;
pub const T_RUST: c_char = 6;
pub const T_MYST: c_char = 7;

const R_SUSTSTR: c_int = 2;
const ARROW: c_int = 3;
const VS_POISON: c_int = 0;

const FALSE: c_uchar = 0;

unsafe extern "C" {
    static mut running: c_uchar;
    static mut count: c_int;
    static mut no_command: c_int;
    static mut no_move: c_int;
    static mut level: c_int;
    static mut cNCOLORS: c_int;
    static mut rainbow: [*const c_char; 27];
    static mut player: CThing;
    static mut cur_armor: *mut CThing;
    static mut cur_ring: [*mut CThing; 2];

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
unsafe fn hero_pos() -> CCoord {
    (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn ring_is(which: usize, ring_type: c_int) -> bool {
    let ring = cur_ring[which];
    !ring.is_null() && (*thing_o(ring)).o_which == ring_type
}

/// Pick a color name from the C `rainbow` table.
#[inline]
unsafe fn rainbow_color() -> *const c_char {
    rainbow[rnd(cNCOLORS) as usize]
}

/// Applies the trap at the given map cell, returning the trap kind that fired.
///
/// The cell's `p_flags` nibble holds the trap number (0-7). If the hero is
/// levitating, no trap effect applies. Uses the C engine helpers (`msg`,
/// `roll`, `spread`, `teleport`, ...) exactly as the legacy `be_trapped` did,
/// but is callable only from Rust.
pub unsafe fn be_trapped(pos: CCoord) -> c_char {
    let trap = draw::trap_kind_at(pos.y, pos.x);

    if ((*thing_t(&raw mut player)).t_flags & ISLEVIT) != 0 {
        return T_RUST;
    }

    running = FALSE;
    count = FALSE as c_int;
    draw::reveal_trap_at(pos.y, pos.x);

    match trap {
        T_DOOR => {
            level += 1;
            new_level();
            msg_str("you fell into a trap!");
        }
        T_BEAR => {
            no_move += spread(3);
            msg_str("you are caught in a bear trap");
        }
        T_MYST => {
            let color = || CStr::from_ptr(rainbow_color()).to_string_lossy().into_owned();
            match rnd(11) {
                0 => {
                    msg_str("you are suddenly in a parallel dimension");
                }
                1 => {
                    msg_str(&format!("the light in here suddenly seems {}", color()));
                }
                2 => {
                    msg_str("you feel a sting in the side of your neck");
                }
                3 => {
                    msg_str("multi-colored lines swirl around you, then fade");
                }
                4 => {
                    msg_str(&format!("a {} light flashes in your eyes", color()));
                }
                5 => {
                    msg_str("a spike shoots past your ear!");
                }
                6 => {
                    msg_str(&format!("{} sparks dance across your armor", color()));
                }
                7 => {
                    msg_str("you suddenly feel very thirsty");
                }
                8 => {
                    msg_str("you feel time speed up suddenly");
                }
                9 => {
                    msg_str("time now seems to be going slower");
                }
                10 => {
                    msg_str(&format!("you pack turns {}!", color()));
                }
                _ => {}
            }
        }
        T_SLEEP => {
            no_command += spread(5);
            (*thing_t(&raw mut player)).t_flags &= !ISRUN;
            msg_str("a strange white mist envelops you and you fall asleep");
        }
        T_ARROW => {
            let stats = &mut (*thing_t(&raw mut player)).t_stats;
            if swing(stats.s_lvl - 1, stats.s_arm, 1) != 0 {
                stats.s_hpt -= roll(1, 6);
                if stats.s_hpt <= 0 {
                    msg_str("an arrow killed you");
                    death(b'a' as c_char);
                } else {
                    msg_str("oh no! An arrow shot you");
                }
            } else {
                let arrow = new_item();
                init_weapon(arrow, ARROW);
                (*thing_o(arrow)).o_count = 1;
                (*thing_o(arrow)).o_pos = hero_pos();
                fall(arrow, FALSE);
                msg_str("an arrow shoots past you");
            }
        }
        T_TELEP => {
            teleport();
            cur::mvaddch(pos.y, pos.x, TRAP as c_uint);
        }
        T_DART => {
            let stats = &mut (*thing_t(&raw mut player)).t_stats;
            if swing(stats.s_lvl + 1, stats.s_arm, 1) == 0 {
                msg_str("a small dart whizzes by your ear and vanishes");
            } else {
                stats.s_hpt -= roll(1, 4);
                if stats.s_hpt <= 0 {
                    msg_str("a poisoned dart killed you");
                    death(b'd' as c_char);
                }
                if !ring_is(LEFT, R_SUSTSTR) && !ring_is(RIGHT, R_SUSTSTR) && save(VS_POISON) == 0 {
                    chg_str(-1);
                }
                msg_str("a small dart just hit you in the shoulder");
            }
        }
        T_RUST => {
            msg_str("a gush of water hits you on the head");
            rust_armor(cur_armor);
        }
        _ => {}
    }

    flush_type();
    trap
}