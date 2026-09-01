use crate::rnd::rnd;
use crate::curses as cur;
/*
 * All the daemon and fuse callback functions.
 *
 * Ported from daemons.c to Rust.
 *
 * Rogue: Exploring the Dungeons of Doom
 * Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
 * All rights reserved.
 *
 * See the file LICENSE.TXT for full copyright and licensing information.
 */

use crate::io::{addmsg_str, msg_str};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint, c_void};

use crate::player::{CCoord, CRoom, CThing, CThingMonster, CThingObject};

// ─── Constants ───────────────────────────────────────────────────────────────

const TRUE:  c_uchar = 1;
const FALSE: c_uchar = 0;

// d_type flags (BEFORE/AFTER)
const BEFORE: c_int = 1; // spread(1) == 1 always
const AFTER:  c_int = 2; // spread(2) == 2 always

// Player-flags
const ISBLIND:  c_short = 0o0000004;
const ISHASTE:  c_short = 0o0000100;
const ISHUH:    c_short = 0o0001000;
const ISINVIS:  c_short = 0o0002000;
const ISHALU:   c_short = 0o0004000;
const ISRUN:    c_short = 0o0020000;
const SEEMONST: c_short = 0o0040000;
const CANSEE:   c_short = 0o0000002;
const ISLEVIT:  c_short = 0o0000010;

// Room flags
const ISGONE: c_short = 0o0000002;

// Ring types
const R_REGEN:  c_int = 9;
const LEFT:     usize = 0;
const RIGHT:    usize = 1;

// Food constants
const MORETIME:   c_int = 150;
const STARVETIME: c_int = 850;

// ─── Extern C globals ────────────────────────────────────────────────────────

unsafe extern "C" {
    static mut player:       CThing;
    static mut quiet:        c_int;
    static mut cur_ring:     [*mut CThing; 2];
    static mut mlist:        *mut CThing;
    static mut lvl_obj:      *mut CThing;
    static mut hungry_state: c_int;
    static mut food_left:    c_int;
    static mut no_command:   c_int;
    static mut terse:        c_uchar;
    static mut amulet:       c_uchar;
    static mut running:      c_uchar;
    static mut to_death:     c_uchar;
    static mut count:        c_int;
    static mut after:        c_uchar;
    static mut jump:         c_uchar;
    static mut seenstairs:   c_uchar;
    static mut stairs:       CCoord;
}

// ─── Extern C functions ──────────────────────────────────────────────────────

unsafe extern "C" {
    fn roll(number: c_int, sides: c_int) -> c_int;
    fn see_monst(mp: *mut CThing) -> c_uchar;
    fn enter_room(cp: *mut CCoord);
    fn choose_str(ts: *const c_char, ns: *const c_char) -> *const c_char;
    fn ring_eat(hand: c_int) -> c_int;
    fn death(monst: c_char);
    fn wanderer();
    fn cansee(y: c_int, x: c_int) -> c_uchar;
    fn rnd_thing() -> c_char;
    fn spread(nm: c_int) -> c_int;
    // Daemon/fuse management (implemented in daemon.rs, same library)
    fn start_daemon(func: *const c_void, arg: c_int, typ: c_int);
    fn kill_daemon(func: *const c_void);
    fn fuse(func: *const c_void, arg: c_int, time: c_int, typ: c_int);
    fn extinguish(func: *const c_void);
}

// ─── Module-local helpers ─────────────────────────────────────────────────────

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

/// ISRING(hand, ring_type): true when the player wears ring_type on hand.
#[inline]
unsafe fn isring(hand: usize, ring_type: c_int) -> bool {
    !cur_ring[hand].is_null()
        && (*thing_o(cur_ring[hand])).o_which == ring_type
}

// ─── Module globals ───────────────────────────────────────────────────────────

/// Counter used by rollwand() to pace wandering-monster checks.
/// Originally defined in daemons.c as `int between = 0;`.
#[no_mangle]
pub static mut between: c_int = 0;

// ─── Daemon / fuse callbacks ──────────────────────────────────────────────────

/// doctor:
/// A healing daemon that restores hit points after rest.
#[no_mangle]
pub unsafe extern "C" fn doctor() {
    let lv  = (*thing_t(&raw mut player)).t_stats.s_lvl;
    let ohp = (*thing_t(&raw mut player)).t_stats.s_hpt;
    quiet += 1;
    if lv < 8 {
        if quiet + (lv << 1) > 20 {
            (*thing_t(&raw mut player)).t_stats.s_hpt += 1;
        }
    } else if quiet >= 3 {
        (*thing_t(&raw mut player)).t_stats.s_hpt += rnd(lv - 7) + 1;
    }
    if isring(LEFT, R_REGEN) {
        (*thing_t(&raw mut player)).t_stats.s_hpt += 1;
    }
    if isring(RIGHT, R_REGEN) {
        (*thing_t(&raw mut player)).t_stats.s_hpt += 1;
    }
    if ohp != (*thing_t(&raw mut player)).t_stats.s_hpt {
        let max = (*thing_t(&raw mut player)).t_stats.s_maxhp;
        if (*thing_t(&raw mut player)).t_stats.s_hpt > max {
            (*thing_t(&raw mut player)).t_stats.s_hpt = max;
        }
        quiet = 0;
    }
}

/// swander:
/// Called when it is time to start rolling for wandering monsters.
#[no_mangle]
pub unsafe extern "C" fn swander() {
    start_daemon(rollwand as *const c_void, 0, BEFORE);
}

/// rollwand:
/// Called to roll to see if a wandering monster starts up.
#[no_mangle]
pub unsafe extern "C" fn rollwand() {
    between += 1;
    if between >= 4 {
        if roll(1, 6) == 4 {
            wanderer();
            kill_daemon(rollwand as *const c_void);
            fuse(swander as *const c_void, 0, spread(70), BEFORE);
        }
        between = 0;
    }
}

/// unconfuse:
/// Release the poor player from his confusion.
#[no_mangle]
pub unsafe extern "C" fn unconfuse() {
    (*thing_t(&raw mut player)).t_flags &= !ISHUH;
    msg_str(&format!(
        "you feel less {} now",
        CStr::from_ptr(choose_str(c"trippy".as_ptr(), c"confused".as_ptr())).to_string_lossy()
    ));
}

/// unsee:
/// Turn off the ability to see invisible.
#[no_mangle]
pub unsafe extern "C" fn unsee() {
    let mut th = mlist;
    while !th.is_null() {
        if ((*thing_t(th)).t_flags & ISINVIS) != 0 && see_monst(th) != 0 {
            cur::mvaddch(
                (*thing_t(th)).t_pos.y,
                (*thing_t(th)).t_pos.x,
                (*thing_t(th)).t_oldch as c_uchar as c_uint,
            );
        }
        th = (*thing_t(th)).l_next;
    }
    (*thing_t(&raw mut player)).t_flags &= !CANSEE;
}

/// sight:
/// He gets his sight back.
#[no_mangle]
pub unsafe extern "C" fn sight() {
    if ((*thing_t(&raw mut player)).t_flags & ISBLIND) != 0 {
        extinguish(sight as *const c_void);
        (*thing_t(&raw mut player)).t_flags &= !ISBLIND;
        let proom: *mut CRoom = (*thing_t(&raw mut player)).t_room;
        if !proom.is_null() && ((*proom).r_flags & ISGONE) == 0 {
            enter_room(&mut (*thing_t(&raw mut player)).t_pos);
        }
        msg_str(&CStr::from_ptr(choose_str(
            c"far out!  Everything is all cosmic again".as_ptr(),
            c"the veil of darkness lifts".as_ptr(),
        )).to_string_lossy());
    }
}

/// nohaste:
/// End the hasting.
#[no_mangle]
pub unsafe extern "C" fn nohaste() {
    (*thing_t(&raw mut player)).t_flags &= !ISHASTE;
    msg_str("you feel yourself slowing down");
}

/// stomach:
/// Digest the hero's food.
#[no_mangle]
pub unsafe extern "C" fn stomach() {
    let orig_hungry = hungry_state;

    if food_left <= 0 {
        // Post-decrement comparison: check old value, then decrement.
        let old_food = food_left;
        food_left -= 1;
        if old_food < -STARVETIME {
            death(b's' as c_char);
        }
        // The hero is fainting.
        if no_command != 0 || rnd(5) != 0 {
            return;
        }
        no_command += rnd(8) + 4;
        hungry_state = 3;
        if terse == 0 {
            addmsg_str(&CStr::from_ptr(choose_str(
                c"the munchies overpower your motor capabilities.  ".as_ptr(),
                c"you feel too weak from lack of food.  ".as_ptr(),
            )).to_string_lossy());
        }
        msg_str(&CStr::from_ptr(choose_str(c"You freak out".as_ptr(), c"You faint".as_ptr())).to_string_lossy());
    } else {
        let oldfood = food_left;
        food_left -= ring_eat(LEFT as c_int) + ring_eat(RIGHT as c_int) + 1 - amulet as c_int;

        if food_left < MORETIME && oldfood >= MORETIME {
            hungry_state = 2;
            msg_str(&CStr::from_ptr(choose_str(
                c"the munchies are interfering with your motor capabilites".as_ptr(),
                c"you are starting to feel weak".as_ptr(),
            )).to_string_lossy());
        } else if food_left < 2 * MORETIME && oldfood >= 2 * MORETIME {
            hungry_state = 1;
            if terse != 0 {
                msg_str(&CStr::from_ptr(choose_str(
                    c"getting the munchies".as_ptr(),
                    c"getting hungry".as_ptr(),
                )).to_string_lossy());
            } else {
                msg_str(&CStr::from_ptr(choose_str(
                    c"you are getting the munchies".as_ptr(),
                    c"you are starting to get hungry".as_ptr(),
                )).to_string_lossy());
            }
        }
    }

    if hungry_state != orig_hungry {
        (*thing_t(&raw mut player)).t_flags &= !ISRUN;
        running  = FALSE;
        to_death = FALSE;
        count    = 0;
    }
}

/// come_down:
/// Take the hero down off her acid trip.
#[no_mangle]
pub unsafe extern "C" fn come_down() {
    if ((*thing_t(&raw mut player)).t_flags & ISHALU) == 0 {
        return;
    }

    kill_daemon(visuals as *const c_void);
    (*thing_t(&raw mut player)).t_flags &= !ISHALU;

    if ((*thing_t(&raw mut player)).t_flags & ISBLIND) != 0 {
        return;
    }

    // Undo the things (objects on the level).
    let mut tp = lvl_obj;
    while !tp.is_null() {
        let op = thing_o(tp);
        if cansee((*op).o_pos.y, (*op).o_pos.x) != 0 {
            cur::mvaddch((*op).o_pos.y, (*op).o_pos.x, (*op).o_type as c_uint);
        }
        tp = (*thing_t(tp)).l_next;
    }

    // Undo the monsters.
    let seemonst = ((*thing_t(&raw mut player)).t_flags & SEEMONST) != 0;
    let mut tp = mlist;
    while !tp.is_null() {
        cur::move_((*thing_t(tp)).t_pos.y, (*thing_t(tp)).t_pos.x);
        if cansee((*thing_t(tp)).t_pos.y, (*thing_t(tp)).t_pos.x) != 0 {
            if ((*thing_t(tp)).t_flags & ISINVIS) == 0
                || ((*thing_t(&raw mut player)).t_flags & CANSEE) != 0
            {
                cur::addch((*thing_t(tp)).t_disguise as c_uchar as c_uint);
            }
            // If invisible and player can't see invisible, skip (original code
            // falls through to the else-if, but cansee returned true here,
            // so seemonst branch is not reached — matching C behavior).
        } else if seemonst {
            cur::standout();
            cur::addch((*thing_t(tp)).t_type as c_uchar as c_uint);
            cur::standend();
        }
        tp = (*thing_t(tp)).l_next;
    }

    msg_str("Everything looks SO boring now.");
}

/// visuals:
/// Change the displayed characters for the hallucinating player.
#[no_mangle]
pub unsafe extern "C" fn visuals() {
    if after == 0 || (running != 0 && jump != 0) {
        return;
    }

    // Change the things (objects).
    let mut tp = lvl_obj;
    while !tp.is_null() {
        let op = thing_o(tp);
        if cansee((*op).o_pos.y, (*op).o_pos.x) != 0 {
            cur::mvaddch((*op).o_pos.y, (*op).o_pos.x, rnd_thing() as c_uchar as c_uint);
        }
        tp = (*thing_t(tp)).l_next;
    }

    // Change the stairs.
    if seenstairs == 0 && cansee(stairs.y, stairs.x) != 0 {
        cur::mvaddch(stairs.y, stairs.x, rnd_thing() as c_uchar as c_uint);
    }

    // Change the monsters.
    let seemonst = ((*thing_t(&raw mut player)).t_flags & SEEMONST) != 0;
    let mut tp = mlist;
    while !tp.is_null() {
        cur::move_((*thing_t(tp)).t_pos.y, (*thing_t(tp)).t_pos.x);
        if see_monst(tp) != 0 {
            if (*thing_t(tp)).t_type == b'X' as c_char
                && (*thing_t(tp)).t_disguise != b'X' as c_char
            {
                cur::addch(rnd_thing() as c_uchar as c_uint);
            } else {
                cur::addch((rnd(26) + b'A' as c_int) as c_uint);
            }
        } else if seemonst {
            cur::standout();
            cur::addch((rnd(26) + b'A' as c_int) as c_uint);
            cur::standend();
        }
        tp = (*thing_t(tp)).l_next;
    }
}

/// land:
/// Land from a levitation potion.
#[no_mangle]
pub unsafe extern "C" fn land() {
    (*thing_t(&raw mut player)).t_flags &= !ISLEVIT;
    msg_str(&CStr::from_ptr(choose_str(
        c"bummer!  You've hit the ground".as_ptr(),
        c"you float gently to the ground".as_ptr(),
    )).to_string_lossy());
}
