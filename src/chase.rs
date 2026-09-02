//! Port of `src/c/chase.c` — one creature chasing another.
//!
//! All functions are exported with the same C ABI as the original, so the
//! C object files can call them directly.  Constants that in C live behind
//! `#ifdef MASTER` (the `debug`/`abort` debug helpers) are replaced by plain
//! Rust `const` values so the debug behavior is always available and there is
//! no need for preprocessor conditionals in Rust.

use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::curses as cur;
use crate::io::msg_str;
use crate::player::{CCoord, CRoom, CThing, CThingMonster, CThingObject};
use crate::rnd::rnd;

const NUMLINES: c_int = 24;
const NUMCOLS: c_int = 80;
const MAXROOMS: usize = 9;
const MAXPASS: usize = 13;

const DRAGONSHOT: c_int = 5; // one chance in DRAGONSHOT that a dragon will flame

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

const F_PASS: c_char = 0x80u8 as c_char;
const F_PNUM: c_char = 0x0fu8 as c_char;

const ISDARK: c_short = 0o000001;
const ISGONE: c_short = 0o000002;
const ISBLIND: c_short = 0o000004;
const ISCANC: c_short = 0o000010;
const ISGREED: c_short = 0o000040;
const ISHASTE: c_short = 0o000100;
const ISTARGET: c_short = 0o000200;
const ISHELD: c_short = 0o000400;
const ISHUH: c_short = 0o001000;
const ISINVIS: c_short = 0o002000;
const ISRUN: c_short = 0o020000;
const SEEMONST: c_short = 0o040000;
const ISFLY: c_short = 0o004000;
const ISSLOW: c_short = 0o010000;
const CANSEE: c_short = 0o000002;

const DOOR: c_char = b'+' as c_char;
const FLOOR: c_char = b'.' as c_char;
const PASSAGE: c_char = b'#' as c_char;
const SCROLL: c_char = b'?' as c_char;
const S_SCARE: c_int = 10;
const BOLT_LENGTH: c_int = 6;
const LAMPDIST: c_int = 3;

/// `#ifdef MASTER` helper: replaced by a plain `const` so the preprocessor
/// conditional disappears.  The C build is compiled without `-DMASTER`, so the
/// debug-only `msg`/`abort` paths are disabled here too; flip to `true` to
/// enable the wizard/debug diagnostics.
const MASTER: bool = false;

/// Where chasing takes you (persistent return slot, mirrors C's `static coord ch_ret`).
static mut CH_RET: CCoord = CCoord { x: 0, y: 0 };
/// Temporary destination for chaser (mirrors C's `static coord this`).
static mut THIS: CCoord = CCoord { x: 0, y: 0 };
/// Temporary try position (mirrors C's `static coord tryp`).
static mut TRYP: CCoord = CCoord { x: 0, y: 0 };
/// Temporary coord for cansee (mirrors C's `static coord tp`).
static mut CANSEE_TP: CCoord = CCoord { x: 0, y: 0 };

unsafe extern "C" {
    static mut mlist: *mut CThing;
    static mut lvl_obj: *mut CThing;
    static mut player: CThing;
    static mut passages: [CRoom; MAXPASS];
    static mut rooms: [CRoom; MAXROOMS];

    static mut has_hit: c_uchar;
    static mut to_death: c_uchar;
    static mut running: c_uchar;
    static mut count: c_int;
    static mut quiet: c_int;
    static mut kamikaze: c_uchar;
    static mut see_floor: c_uchar;
    static mut delta: CCoord;
    static mut monsters: [crate::monsters::CMonster; 26];

    fn endmsg() -> c_int;
    fn step_ok(ch: c_int) -> c_int;
    fn attack(mp: *mut CThing) -> c_int;
    fn fire_bolt(start: *mut CCoord, dir: *mut CCoord, name: *mut c_char);
    fn sign(nm: c_int) -> c_int;
    fn rndmove(who: *mut CThing) -> *mut CCoord;
    fn _detach(list: *mut *mut CThing, item: *mut CThing);
    fn _attach(list: *mut *mut CThing, item: *mut CThing);
    fn abort() -> !;
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
unsafe fn hero_ptr() -> *mut CCoord {
    &mut (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn player_has(flag: c_short) -> bool {
    ((*thing_t(&raw mut player)).t_flags & flag) != 0
}

#[inline]
unsafe fn monster_has(tp: *mut CThing, flag: c_short) -> bool {
    ((*thing_t(tp)).t_flags & flag) != 0
}

#[inline]
unsafe fn coord_eq(a: CCoord, b: CCoord) -> bool {
    a.x == b.x && a.y == b.y
}

#[inline]
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    crate::draw::chat_at(y, x)
}

#[inline]
unsafe fn flat_at(y: c_int, x: c_int) -> c_char {
    crate::draw::flat_at(y, x)
}

#[inline]
unsafe fn moat_at(y: c_int, x: c_int) -> *mut CThing {
    crate::game::monster_at(y, x)
}

#[inline]
unsafe fn set_moat_at(y: c_int, x: c_int, tp: *mut CThing) {
    crate::game::set_monster(y, x, tp);
}

#[inline]
unsafe fn winat(y: c_int, x: c_int) -> c_char {
    let tp = moat_at(y, x);
    if tp.is_null() {
        chat_at(y, x)
    } else {
        (*thing_t(tp)).t_disguise
    }
}

/// runners:
/// Make all the running monsters move.
///
/// Uses globals: mlist, hero, to_death, has_hit.
#[no_mangle]
pub unsafe extern "C" fn runners() {
    let mut tp = mlist;
    while !tp.is_null() {
        // remember this in case the monster's "next" is changed
        let next = (*thing_t(tp)).l_next;
        if !monster_has(tp, ISHELD) && monster_has(tp, ISRUN) {
            let orig_pos = (*thing_t(tp)).t_pos;
            let wastarget = monster_has(tp, ISTARGET);
            if move_monst(tp) == -1 {
                tp = next;
                continue;
            }
            if monster_has(tp, ISFLY) && dist_cp(hero_ptr(), &raw mut (*thing_t(tp)).t_pos) >= 3 {
                move_monst(tp);
            }
            if wastarget && !coord_eq(orig_pos, (*thing_t(tp)).t_pos) {
                (*thing_t(tp)).t_flags &= !ISTARGET;
                to_death = FALSE;
            }
        }
        tp = next;
    }
    if has_hit != 0 {
        endmsg();
        has_hit = FALSE;
    }
}

/// move_monst:
/// Execute a single turn of running for a monster
#[no_mangle]
pub unsafe extern "C" fn move_monst(tp: *mut CThing) -> c_int {
    if !monster_has(tp, ISSLOW) || (*thing_t(tp)).t_turn != 0 {
        if do_chase(tp) == -1 {
            return -1;
        }
    }
    if monster_has(tp, ISHASTE) {
        if do_chase(tp) == -1 {
            return -1;
        }
    }
    (*thing_t(tp)).t_turn ^= TRUE;
    0
}

/// relocate:
/// Make the monster's new location be the specified one, updating
/// all the relevant state.
///
/// Uses globals: places (via moat), player, see_monst (function).
#[no_mangle]
pub unsafe extern "C" fn relocate(th: *mut CThing, new_loc: *mut CCoord) {
    if new_loc.is_null() {
        return;
    }
    if !coord_eq(*new_loc, (*thing_t(th)).t_pos) {
        cur::mvaddch(
            (*thing_t(th)).t_pos.y,
            (*thing_t(th)).t_pos.x,
            (*thing_t(th)).t_oldch as c_uint,
        );
        (*thing_t(th)).t_room = roomin(new_loc);
        set_oldch(th, new_loc);
        let oroom = (*thing_t(th)).t_room;
        set_moat_at((*thing_t(th)).t_pos.y, (*thing_t(th)).t_pos.x, std::ptr::null_mut());

        if oroom != (*thing_t(th)).t_room {
            (*thing_t(th)).t_dest = find_dest(th);
        }
        (*thing_t(th)).t_pos = *new_loc;
        set_moat_at((*new_loc).y, (*new_loc).x, th);
    }
    cur::r#move((*new_loc).y, (*new_loc).x);
    if see_monst(th) != FALSE {
        cur::addch((*thing_t(th)).t_disguise as c_uint);
    } else if player_has(SEEMONST) {
        cur::standout();
        cur::addch((*thing_t(th)).t_type as c_uint);
        cur::standend();
    }
}

/// do_chase:
/// Make one thing chase another.
///
/// Uses globals: hero, proom, passages, places (via flat/chat/moat),
/// delta, running, count, quiet, to_death, kamikaze, lvl_obj.
#[no_mangle]
pub unsafe extern "C" fn do_chase(th: *mut CThing) -> c_int {
    let mut mindist: c_int = 32767;
    let mut curdist: c_int;
    let mut stoprun = false; // TRUE means we are there
    let mut door: bool;
    let mut obj: *mut CThing;

    let mut rer = (*thing_t(th)).t_room; // Find room of chaser
    if monster_has(th, ISGREED) && (*rer).r_goldval == 0 {
        (*thing_t(th)).t_dest = hero_ptr(); // If gold has been taken, run after hero
    }
    let ree = if (*thing_t(th)).t_dest == hero_ptr() {
        // Find room of chasee
        (*thing_t(&raw mut player)).t_room
    } else {
        roomin((*thing_t(th)).t_dest)
    };
    // We don't count doors as inside rooms for this routine
    door = chat_at((*thing_t(th)).t_pos.y, (*thing_t(th)).t_pos.x) == DOOR;
    // If the object of our desire is in a different room,
    // and we are not in a corridor, run to the door nearest to
    // our goal.
    let mut loop_rer = rer;
    let mut loop_door = door;
    let mut loop_ree = ree;
    loop {
        if loop_rer != loop_ree {
            let exits: &[CCoord] = std::slice::from_raw_parts(
                (*loop_rer).r_exit.as_ptr(),
                (*loop_rer).r_nexits as usize,
            );
            for cp in exits {
                curdist = dist_cp((*thing_t(th)).t_dest, cp as *const CCoord as *mut CCoord);
                if curdist < mindist {
                    THIS = *cp;
                    mindist = curdist;
                }
            }
            if loop_door {
                let pnum = (flat_at((*thing_t(th)).t_pos.y, (*thing_t(th)).t_pos.x) & F_PNUM) as usize;
                loop_rer = &raw mut passages[pnum] as *mut CRoom;
                loop_door = false;
                continue;
            }
        } else {
            THIS = *(*thing_t(th)).t_dest;
            // For dragons check and see if (a) the hero is on a straight
            // line from it, and (b) that it is within shooting distance,
            // but outside of striking range.
            if (*thing_t(th)).t_type == 'D' as c_char
                && ((*thing_t(th)).t_pos.y == hero_pos().y
                    || (*thing_t(th)).t_pos.x == hero_pos().x
                    || ((*thing_t(th)).t_pos.y - hero_pos().y).abs()
                        == ((*thing_t(th)).t_pos.x - hero_pos().x).abs())
                && dist_cp(&raw mut (*thing_t(th)).t_pos, hero_ptr()) <= BOLT_LENGTH * BOLT_LENGTH
                && !monster_has(th, ISCANC)
                && rnd(DRAGONSHOT) == 0
            {
                let dy = hero_pos().y - (*thing_t(th)).t_pos.y;
                let dx = hero_pos().x - (*thing_t(th)).t_pos.x;
                delta.y = sign(dy);
                delta.x = sign(dx);
                if has_hit != 0 {
                    endmsg();
                }
                fire_bolt(
                    &raw mut (*thing_t(th)).t_pos,
                    &raw mut delta,
                    c"flame".as_ptr() as *mut c_char,
                );
                running = FALSE;
                count = 0;
                quiet = 0;
                if to_death != 0 && !monster_has(th, ISTARGET) {
                    to_death = FALSE;
                    kamikaze = FALSE;
                }
                return 0;
            }
        }
        break;
    }

    // This now contains what we want to run to this time
    // so we run to it.  If we hit it we either want to fight it
    // or stop running.
    if chase(th, &raw mut THIS) == FALSE {
        if coord_eq(THIS, hero_pos()) {
            return attack(th);
        } else if coord_eq(THIS, *(*thing_t(th)).t_dest) {
            obj = lvl_obj;
            while !obj.is_null() {
                if (*thing_t(th)).t_dest == &raw mut (*thing_o(obj)).o_pos {
                    _detach(&raw mut lvl_obj, obj);
                    _attach(&raw mut (*thing_t(th)).t_pack, obj);
                    // Objects render from the `lvl_obj` list; the floor glyph
                    // under a picked-up object is then the terrain char.
                    (*thing_t(th)).t_dest = find_dest(th);
                    break;
                }
                obj = (*thing_o(obj)).l_next;
            }
            if (*thing_t(th)).t_type != 'F' as c_char {
                stoprun = true;
            }
        }
    } else if (*thing_t(th)).t_type == 'F' as c_char {
        return 0;
    }
    relocate(th, &raw mut CH_RET);
    // And stop running if need be
    if stoprun && coord_eq((*thing_t(th)).t_pos, *(*thing_t(th)).t_dest) {
        (*thing_t(th)).t_flags &= !ISRUN;
    }
    0
}

/// set_oldch:
/// Set the oldch character for the monster
///
/// Uses globals: player, hero, see_floor, places (via chat).
#[no_mangle]
pub unsafe extern "C" fn set_oldch(tp: *mut CThing, cp: *mut CCoord) {
    if coord_eq((*thing_t(tp)).t_pos, *cp) {
        return;
    }

    let sch = (*thing_t(tp)).t_oldch;
    (*thing_t(tp)).t_oldch = (cur::mvinch((*cp).y, (*cp).x) & 0x7f) as c_char;
    if !player_has(ISBLIND) {
        if (sch == FLOOR || (*thing_t(tp)).t_oldch == FLOOR)
            && ((*(*thing_t(tp)).t_room).r_flags & ISDARK) != 0
        {
            (*thing_t(tp)).t_oldch = b' ' as c_char;
        } else if dist_cp(cp, hero_ptr()) <= LAMPDIST && see_floor != 0 {
            (*thing_t(tp)).t_oldch = chat_at((*cp).y, (*cp).x);
        }
    }
}

/// see_monst:
/// Return TRUE if the hero can see the monster
///
/// Uses globals: player, hero, proom, places (via chat).
#[no_mangle]
pub unsafe extern "C" fn see_monst(mp: *mut CThing) -> c_uchar {
    if player_has(ISBLIND) {
        return FALSE;
    }
    if monster_has(mp, ISINVIS) && !player_has(CANSEE) {
        return FALSE;
    }
    let y = (*thing_t(mp)).t_pos.y;
    let x = (*thing_t(mp)).t_pos.x;
    if dist(y, x, hero_pos().y, hero_pos().x) < LAMPDIST {
        if y != hero_pos().y
            && x != hero_pos().x
            && step_ok(chat_at(y, hero_pos().x) as c_int) == 0
            && step_ok(chat_at(hero_pos().y, x) as c_int) == 0
        {
            return FALSE;
        }
        return TRUE;
    }
    if (*thing_t(mp)).t_room != (*thing_t(&raw mut player)).t_room {
        return FALSE;
    }
    if ((*(*thing_t(mp)).t_room).r_flags & ISDARK) != 0 {
        FALSE
    } else {
        TRUE
    }
}

/// runto:
/// Set a monster running after the hero.
///
/// Uses globals: places (via moat).
#[no_mangle]
pub unsafe extern "C" fn runto(runner: *mut CCoord) {
    // If we couldn't find him, something is funny.
    // (C guarded this with `#ifdef MASTER`; always report in the Rust port.)
    let tp = moat_at((*runner).y, (*runner).x);
    if MASTER && tp.is_null() {
        msg_str(&format!(
            "couldn't find monster in runto at ({},{})",
            (*runner).y,
            (*runner).x
        ));
    }
    if tp.is_null() {
        return;
    }
    // Start the beastie running
    (*thing_t(tp)).t_flags |= ISRUN;
    (*thing_t(tp)).t_flags &= !ISHELD;
    (*thing_t(tp)).t_dest = find_dest(tp);
}

/// chase:
/// Find the spot for the chaser(er) to move closer to the
/// chasee(ee).  Returns TRUE if we want to keep on chasing later
/// FALSE if we reach the goal.
///
/// Uses globals: hero, lvl_obj, places (via moat/chat/winat).
#[no_mangle]
pub unsafe extern "C" fn chase(tp: *mut CThing, ee: *mut CCoord) -> c_uchar {
    let mut curdist: c_int;
    let mut thisdist: c_int;
    let er = &raw mut (*thing_t(tp)).t_pos;
    let mut plcnt = 1;

    // If the thing is confused, let it move randomly. Invisible
    // Stalkers are slightly confused all of the time, and bats are
    // quite confused all the time.
    if (monster_has(tp, ISHUH) && rnd(5) != 0)
        || ((*thing_t(tp)).t_type == 'P' as c_char && rnd(5) == 0)
        || ((*thing_t(tp)).t_type == 'B' as c_char && rnd(2) == 0)
    {
        // get a valid random move
        CH_RET = *rndmove(tp);
        curdist = dist_cp(&raw mut CH_RET, ee);
        // Small chance that it will become un-confused
        if rnd(20) == 0 {
            (*thing_t(tp)).t_flags &= !ISHUH;
        }
    }
    // Otherwise, find the empty spot next to the chaser that is
    // closest to the chasee.
    else {
        // This will eventually hold where we move to get closer.
        // If we can't find an empty spot, we stay where we are.
        curdist = dist_cp(er, ee);
        CH_RET = *er;

        let mut ey = (*er).y + 1;
        if ey >= NUMLINES - 1 {
            ey = NUMLINES - 2;
        }
        let mut ex = (*er).x + 1;
        if ex >= NUMCOLS {
            ex = NUMCOLS - 1;
        }

        let mut x = (*er).x - 1;
        while x <= ex {
            if x >= 0 {
                TRYP.x = x;
                let mut y = (*er).y - 1;
                while y <= ey {
                    TRYP.y = y;
                    if diag_ok(er, &raw mut TRYP) == FALSE {
                        y += 1;
                        continue;
                    }
                    let ch = winat(y, x);
                    if step_ok(ch as c_int) != 0 {
                        // If it is a scroll, it might be a scare monster scroll
                        // so we need to look it up to see what type it is.
                        if ch == SCROLL {
                            let mut obj = lvl_obj;
                            while !obj.is_null() {
                                if y == (*thing_o(obj)).o_pos.y && x == (*thing_o(obj)).o_pos.x {
                                    break;
                                }
                                obj = (*thing_o(obj)).l_next;
                            }
                            if !obj.is_null() && (*thing_o(obj)).o_which == S_SCARE {
                                y += 1;
                                continue;
                            }
                        }
                        // It can also be a Xeroc, which we shouldn't step on.
                        let obj = moat_at(y, x);
                        if !obj.is_null() && (*thing_t(obj)).t_type == 'X' as c_char {
                            y += 1;
                            continue;
                        }
                        // If we didn't find any scrolls at this place or it
                        // wasn't a scare scroll, then this place counts.
                        thisdist = dist(y, x, (*ee).y, (*ee).x);
                        if thisdist < curdist {
                            plcnt = 1;
                            CH_RET = TRYP;
                            curdist = thisdist;
                        } else if thisdist == curdist && rnd(plcnt + 1) == 0 {
                            // C's rnd(++plcnt) bumps plcnt then draws in [0, plcnt).
                            plcnt += 1;
                            CH_RET = TRYP;
                            curdist = thisdist;
                        }
                    }
                    y += 1;
                }
            }
            x += 1;
        }
    }
    if curdist != 0 && !coord_eq(CH_RET, hero_pos()) {
        TRUE
    } else {
        FALSE
    }
}

/// roomin:
/// Find what room some coordinates are in. NULL means they aren't
/// in any room.
///
/// Uses globals: places (via flat), passages, rooms, msg.
#[no_mangle]
pub unsafe extern "C" fn roomin(cp: *mut CCoord) -> *mut CRoom {
    if cp.is_null() {
        return std::ptr::null_mut();
    }
    let fp = flat_at((*cp).y, (*cp).x);
    if (fp & F_PASS) != 0 {
        return &raw mut passages[(fp & F_PNUM) as usize] as *mut CRoom;
    }

    for rp in rooms.iter_mut() {
        if (*cp).x <= rp.r_pos.x + rp.r_max.x
            && rp.r_pos.x <= (*cp).x
            && (*cp).y <= rp.r_pos.y + rp.r_max.y
            && rp.r_pos.y <= (*cp).y
        {
            return rp as *mut CRoom;
        }
    }

    msg_str(&format!(
        "in some bizarre place ({}, {})",
        (*cp).y,
        (*cp).x
    ));
    if MASTER {
        abort();
    }
    std::ptr::null_mut()
}

/// diag_ok:
/// Check to see if the move is legal if it is diagonal
///
/// Uses globals: places (via chat).
#[no_mangle]
pub unsafe extern "C" fn diag_ok(sp: *mut CCoord, ep: *mut CCoord) -> c_uchar {
    if (*ep).x < 0 || (*ep).x >= NUMCOLS || (*ep).y <= 0 || (*ep).y >= NUMLINES - 1 {
        return FALSE;
    }
    if (*ep).x == (*sp).x || (*ep).y == (*sp).y {
        return TRUE;
    }
    if step_ok(chat_at((*ep).y, (*sp).x) as c_int) != 0
        && step_ok(chat_at((*sp).y, (*ep).x) as c_int) != 0
    {
        TRUE
    } else {
        FALSE
    }
}

/// cansee:
/// Returns true if the hero can see a certain coordinate.
///
/// Uses globals: player, hero, proom, places (via flat/chat).
#[no_mangle]
pub unsafe extern "C" fn cansee(y: c_int, x: c_int) -> c_uchar {
    if player_has(ISBLIND) {
        return FALSE;
    }
    if dist(y, x, hero_pos().y, hero_pos().x) < LAMPDIST {
        if (flat_at(y, x) & F_PASS) != 0 {
            if y != hero_pos().y
                && x != hero_pos().x
                && step_ok(chat_at(y, hero_pos().x) as c_int) == 0
                && step_ok(chat_at(hero_pos().y, x) as c_int) == 0
            {
                return FALSE;
            }
        }
        return TRUE;
    }
    // We can only see if the hero in the same room as
    // the coordinate and the room is lit or if it is close.
    CANSEE_TP.y = y;
    CANSEE_TP.x = x;
    let rer = roomin(&raw mut CANSEE_TP);
    if rer == (*thing_t(&raw mut player)).t_room && ((*rer).r_flags & ISDARK) == 0 {
        TRUE
    } else {
        FALSE
    }
}

/// find_dest:
/// find the proper destination for the monster
///
/// Uses globals: monsters, hero, proom, lvl_obj, mlist.
#[no_mangle]
pub unsafe extern "C" fn find_dest(tp: *mut CThing) -> *mut CCoord {
    let prob = monsters[((*thing_t(tp)).t_type as i32 - 'A' as i32) as usize].m_carry;
    if prob <= 0 || (*thing_t(tp)).t_room == (*thing_t(&raw mut player)).t_room || see_monst(tp) != FALSE {
        return hero_ptr();
    }
    let mut obj = lvl_obj;
    while !obj.is_null() {
        if (*thing_o(obj)).o_type == SCROLL as c_int && (*thing_o(obj)).o_which == S_SCARE {
            obj = (*thing_o(obj)).l_next;
            continue;
        }
        if roomin(&raw mut (*thing_o(obj)).o_pos) == (*thing_t(tp)).t_room && rnd(100) < prob {
            let mut m = mlist;
            while !m.is_null() {
                if (*thing_t(m)).t_dest == &raw mut (*thing_o(obj)).o_pos {
                    break;
                }
                m = (*thing_t(m)).l_next;
            }
            if m.is_null() {
                return &raw mut (*thing_o(obj)).o_pos;
            }
        }
        obj = (*thing_o(obj)).l_next;
    }
    hero_ptr()
}

/// dist:
/// Calculate the "distance" between to points.  Actually,
/// this calculates d^2, not d, but that's good enough for
/// our purposes, since it's only used comparitively.
#[no_mangle]
pub unsafe extern "C" fn dist(y1: c_int, x1: c_int, y2: c_int, x2: c_int) -> c_int {
    (x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)
}

/// dist_cp:
/// Call dist() with appropriate arguments for coord pointers
#[no_mangle]
pub unsafe extern "C" fn dist_cp(c1: *mut CCoord, c2: *mut CCoord) -> c_int {
    dist((*c1).y, (*c1).x, (*c2).y, (*c2).x)
}
