use crate::rnd::rnd;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint, c_void};
use std::ptr;

use crate::draw::place_at;

/// Potion and status-effect handling for the Rust FFI bridge.
/// These helpers implement the C-side potion logic so the game can call
/// them through exported C entry points.
const NUMCOLS: c_int = 80;
const NUMLINES: c_int = 24;

const POTION: c_int = '!' as c_int;
const SCROLL: c_int = '?' as c_int;
const WEAPON: c_int = ')' as c_int;
const ARMOR: c_int = ']' as c_int;
const RING: c_int = '=' as c_int;
const STICK: c_int = '/' as c_int;
const AMULET: c_int = ',' as c_int;
const FOOD: c_int = ':' as c_int;
const MAGIC: c_int = '$' as c_int;
const STAIRS: c_int = '%' as c_int;
const FLOOR: c_int = '.' as c_int;
const PASSAGE: c_int = '#' as c_int;
const SPACE: c_int = ' ' as c_int;
const H_WALL: c_int = '-' as c_int;
const V_WALL: c_int = '|' as c_int;
const TRAP: c_int = '^' as c_int;

const LEFT: usize = 0;
const RIGHT: usize = 1;

const ISHUH: c_short = 0o0001000;
const ISHALU: c_short = 0o0004000;
const CANSEE: c_short = 0o0000002;
const ISBLIND: c_short = 0o0000004;
const ISLEVIT: c_short = 0o0000010;
const ISRUN: c_short = 0o020000;
const ISINVIS: c_short = 0o0002000;
const SEEMONST: c_short = 0o040000;
const ISCURSED: c_int = 0o000001;
const ISPROT: c_int = 0o000040;

const R_ADDSTR: c_int = 1;
const R_SUSTSTR: c_int = 2;

const P_CONFUSE: c_int = 0;
const P_LSD: c_int = 1;
const P_POISON: c_int = 2;
const P_STRENGTH: c_int = 3;
const P_SEEINVIS: c_int = 4;
const P_HEALING: c_int = 5;
const P_MFIND: c_int = 6;
const P_TFIND: c_int = 7;
const P_RAISE: c_int = 8;
const P_XHEAL: c_int = 9;
const P_HASTE: c_int = 10;
const P_RESTORE: c_int = 11;
const P_BLIND: c_int = 12;
const P_LEVIT: c_int = 13;
const MAXPOTIONS: usize = 14;

const HUHDURATION: c_int = 20;
const SEEDURATION: c_int = 850;
const HEALTIME: c_int = 30;
const BEFORE: c_int = 1;
const AFTER: c_int = 2;

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

/// Data structures mirrored from the C game so Rust can interact with
/// the same in-memory layout expected by the FFI boundary.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CCoord {
    pub x: c_int,
    pub y: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CStats {
    pub s_str: c_uint,
    pub s_exp: c_int,
    pub s_lvl: c_int,
    pub s_arm: c_int,
    pub s_hpt: c_int,
    pub s_dmg: [c_char; 13],
    pub s_maxhp: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CThingMonster {
    pub l_next: *mut CThing,
    pub l_prev: *mut CThing,
    pub t_pos: CCoord,
    pub t_turn: c_uchar,
    pub t_type: c_char,
    pub t_disguise: c_char,
    pub t_oldch: c_char,
    pub t_dest: *mut CCoord,
    pub t_flags: c_short,
    pub t_stats: CStats,
    pub t_room: *mut c_void,
    pub t_pack: *mut CThing,
    pub t_reserved: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CThingObject {
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

#[repr(C)]
pub union CThing {
    pub t: CThingMonster,
    pub o: CThingObject,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CPlace {
    pub p_monst: *mut CThing,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CObjInfo {
    pub oi_name: *mut c_char,
    pub oi_prob: c_int,
    pub oi_worth: c_int,
    pub oi_guess: *mut c_char,
    pub oi_know: c_uchar,
}

#[repr(C)]
struct PACT {
    pa_flags: c_short,
    pa_daemon: *const c_void,
    pa_time: c_int,
    pa_high: *const c_char,
    pa_straight: *const c_char,
}

/// External C symbols that provide game state, UI helpers, and gameplay
/// primitives used by the potion effects.
unsafe extern "C" {
    static mut terse: c_uchar;
    static mut after: c_uchar;
    static mut seenstairs: c_uchar;
    static mut fruit: [c_char; 1024];
    static mut prbuf: [c_char; 2048];
    static mut player: CThing;
    static mut cur_weapon: *mut CThing;
    static mut cur_ring: [*mut CThing; 2];
    static mut lvl_obj: *mut CThing;
    static mut mlist: *mut CThing;
    static mut places: [CPlace; 32 * 80];
    static mut hw: *mut c_void;
    static mut pot_info: [CObjInfo; MAXPOTIONS];
    static mut max_stats: CStats;
    static mut stairs: CCoord;
    static mut e_levels: [c_int; 21];

    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn leave_pack(obj: *mut CThing, newobj: c_uchar, all: c_uchar) -> *mut CThing;
    fn discard(item: *mut CThing);
    fn msg(fmt: *const c_char, ...);
    fn addmsg(fmt: *const c_char, ...);
    fn endmsg() -> c_int;
    fn roll(num: c_int, sides: c_int) -> c_int;
    fn chg_str(amt: c_int);
    fn add_str(sp: *mut c_uint, amt: c_int);
    fn pick_color(col: *const c_char) -> *mut c_char;
    fn choose_str(ts: *const c_char, ns: *const c_char) -> *mut c_char;
    fn snprintf(s: *mut c_char, n: usize, fmt: *const c_char, ...) -> c_int;
    fn show_win(message: *const c_char);
    fn call_it(info: *mut CObjInfo);
    fn look(wakeup: c_uchar);
    fn status();
    fn check_level();
    fn come_down();
    fn add_haste(potion: c_uchar) -> c_uchar;
    fn unconfuse();
    fn unsee();
    fn sight();
    fn land();
    fn visuals();
    fn start_daemon(func: *const c_void, arg: c_int, typ: c_int);
    fn fuse(func: *const c_void, arg: c_int, time: c_int, typ: c_int);
    fn lengthen(func: *const c_void, xtime: c_int);
    fn spread(nm: c_int) -> c_int;
    fn see_monst(mp: *mut CThing) -> c_uchar;
    fn wclear(win: *mut c_void) -> c_int;
    fn wmove(win: *mut c_void, y: c_int, x: c_int) -> c_int;
    fn waddch(win: *mut c_void, ch: c_uint) -> c_int;
    fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int;
    fn r#move(y: c_int, x: c_int) -> c_int;
    fn addch(ch: c_uint) -> c_int;
    fn standout();
    fn standend();
    fn inch() -> c_int;
}

/// Cast a generic thing pointer to the monster portion of the union.
#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

/// Cast a generic thing pointer to the object portion of the union.
#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn hero() -> CCoord {
    (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn player_has(flag: c_short) -> bool {
    ((*thing_t(&raw mut player)).t_flags & flag) != 0
}

#[inline]
unsafe fn thing_has(tp: *mut CThing, flag: c_short) -> bool {
    ((*thing_t(tp)).t_flags & flag) != 0
}

#[inline]
unsafe fn ring_is(which: usize, ring_type: c_int) -> bool {
    let ring = cur_ring[which];
    !ring.is_null() && (*thing_o(ring)).o_which == ring_type
}

#[inline]
unsafe fn next_thing(tp: *mut CThing) -> *mut CThing {
    (*thing_o(tp)).l_next
}

#[inline]
unsafe fn moat(y: c_int, x: c_int) -> *mut CThing {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_monst
}

#[inline]
unsafe fn is_magic_local(obj: *mut CThing) -> bool {
    match (*thing_o(obj)).o_type {
        ARMOR => (((*thing_o(obj)).o_flags & ISPROT) != 0)
            || (*thing_o(obj)).o_arm != 0,
        WEAPON => (*thing_o(obj)).o_hplus != 0 || (*thing_o(obj)).o_dplus != 0,
        POTION | SCROLL | STICK | RING | AMULET => true,
        _ => false,
    }
}

/// Shared implementation for potion effects that need the normal fuse/flag
/// setup and knowledge tracking used by the C version.
unsafe fn do_pot_impl(type_id: c_int, knowit: c_uchar) {
    let (flags, daemon, base_time, high_msg, straight_msg) = {
        let taste_ptr = (&raw mut prbuf) as *mut [c_char; 2048] as *mut c_char as *const c_char;
        match type_id {
            P_CONFUSE => (
                ISHUH,
                unconfuse as *const c_void,
                HUHDURATION,
                c"what a tripy feeling!".as_ptr(),
                c"wait, what's going on here. Huh? What? Who?".as_ptr(),
            ),
            P_LSD => (
                ISHALU,
                come_down as *const c_void,
                SEEDURATION,
                c"Oh, wow!  Everything seems so cosmic!".as_ptr(),
                c"Oh, wow!  Everything seems so cosmic!".as_ptr(),
            ),
            P_SEEINVIS => (
                CANSEE,
                unsee as *const c_void,
                SEEDURATION,
                taste_ptr,
                taste_ptr,
            ),
            P_BLIND => (
                ISBLIND,
                sight as *const c_void,
                SEEDURATION,
                c"oh, bummer!  Everything is dark!  Help!".as_ptr(),
                c"a cloak of darkness falls around you".as_ptr(),
            ),
            P_LEVIT => (
                ISLEVIT,
                land as *const c_void,
                HEALTIME,
                c"oh, wow!  You're floating in the air!".as_ptr(),
                c"you start to float in the air".as_ptr(),
            ),
            _ => (0, ptr::null(), 0, ptr::null(), ptr::null()),
        }
    };

    if type_id >= 0 && (type_id as usize) < MAXPOTIONS && (*pot_info.as_mut_ptr().add(type_id as usize)).oi_know == 0 {
        (*pot_info.as_mut_ptr().add(type_id as usize)).oi_know = knowit;
    }

    if flags == 0 || daemon.is_null() {
        return;
    }

    let t = spread(base_time);
    if !player_has(flags) {
        (*thing_t(&raw mut player)).t_flags |= flags;
        fuse(daemon, 0, t, AFTER);
        look(FALSE);
    } else {
        lengthen(daemon, t);
    }
    msg(choose_str(high_msg, straight_msg));
}

/// quaff:
/// Quaff a potion from the pack.
#[no_mangle]
pub unsafe extern "C" fn quaff() {
    let obj = get_item(c"quaff".as_ptr(), POTION);
    let mut tp: *mut CThing;
    let mut mp: *mut CThing;
    let discardit;
    let mut show = false;
    let trip = player_has(ISHALU);

    if obj.is_null() {
        return;
    }
    if (*thing_o(obj)).o_type != POTION {
        if terse == 0 {
            msg(c"yuk! Why would you want to drink that?".as_ptr());
        } else {
            msg(c"that's undrinkable".as_ptr());
        }
        return;
    }
    if obj == cur_weapon {
        cur_weapon = ptr::null_mut();
    }

    discardit = (*thing_o(obj)).o_count == 1;
    leave_pack(obj, FALSE, FALSE);

    match (*thing_o(obj)).o_which {
        P_CONFUSE => do_pot_impl(P_CONFUSE, if trip { FALSE } else { TRUE }),
        P_POISON => {
            (*pot_info.as_mut_ptr().add(P_POISON as usize)).oi_know = TRUE;
            if ring_is(LEFT, R_SUSTSTR) || ring_is(RIGHT, R_SUSTSTR) {
                msg(c"you feel momentarily sick".as_ptr());
            } else {
                chg_str(-(rnd(3) + 1));
                msg(c"you feel very sick now".as_ptr());
                come_down();
            }
        }
        P_HEALING => {
            let stats = thing_t(&raw mut player);
            (*pot_info.as_mut_ptr().add(P_HEALING as usize)).oi_know = TRUE;
            (*stats).t_stats.s_hpt += roll((*stats).t_stats.s_lvl, 4);
            if (*stats).t_stats.s_hpt > (*stats).t_stats.s_maxhp {
                (*stats).t_stats.s_maxhp += 1;
                (*stats).t_stats.s_hpt = (*stats).t_stats.s_maxhp;
            }
            sight();
            msg(c"you begin to feel better".as_ptr());
        }
        P_STRENGTH => {
            (*pot_info.as_mut_ptr().add(P_STRENGTH as usize)).oi_know = TRUE;
            chg_str(1);
            msg(c"you feel stronger, now.  What bulging muscles!".as_ptr());
        }
        P_MFIND => {
            (*thing_t(&raw mut player)).t_flags |= SEEMONST;
            fuse(turn_see as *const c_void, TRUE as c_int, HUHDURATION, AFTER);
            if turn_see(FALSE) == 0 {
                msg(c"you have a %s feeling for a moment, then it passes".as_ptr(), choose_str(c"normal".as_ptr(), c"strange".as_ptr()));
            }
        }
        P_TFIND => {
            if !lvl_obj.is_null() {
                wclear(hw);
                tp = lvl_obj;
                while !tp.is_null() {
                    if is_magic_local(tp) {
                        show = true;
                        wmove(hw, (*thing_o(tp)).o_pos.y, (*thing_o(tp)).o_pos.x);
                        waddch(hw, MAGIC as c_uint);
                        (*pot_info.as_mut_ptr().add(P_TFIND as usize)).oi_know = TRUE;
                    }
                    tp = next_thing(tp);
                }
                mp = mlist;
                while !mp.is_null() {
                    tp = (*thing_t(mp)).t_pack;
                    while !tp.is_null() {
                        if is_magic_local(tp) {
                            show = true;
                            wmove(hw, (*thing_t(mp)).t_pos.y, (*thing_t(mp)).t_pos.x);
                            waddch(hw, MAGIC as c_uint);
                        }
                        tp = next_thing(tp);
                    }
                    mp = next_thing(mp);
                }
            }
            if show {
                (*pot_info.as_mut_ptr().add(P_TFIND as usize)).oi_know = TRUE;
                show_win(c"You sense the presence of magic on this level.--More--".as_ptr());
            } else {
                msg(c"you have a %s feeling for a moment, then it passes".as_ptr(), choose_str(c"normal".as_ptr(), c"strange".as_ptr()));
            }
        }
        P_LSD => {
            if !trip {
                if player_has(SEEMONST) {
                    turn_see(FALSE);
                }
                start_daemon(visuals as *const c_void, 0, BEFORE);
                seenstairs = seen_stairs();
            }
            do_pot_impl(P_LSD, TRUE);
        }
        P_SEEINVIS => {
            let _ = snprintf(
                (&raw mut prbuf) as *mut [c_char; 2048] as *mut c_char,
                prbuf.len(),
                c"this potion tastes like %s juice".as_ptr(),
                fruit.as_ptr(),
            );
            show = player_has(CANSEE);
            do_pot_impl(P_SEEINVIS, FALSE);
            if !show {
                invis_on();
            }
            sight();
        }
        P_RAISE => {
            (*pot_info.as_mut_ptr().add(P_RAISE as usize)).oi_know = TRUE;
            msg(c"you suddenly feel much more skillful".as_ptr());
            raise_level();
        }
        P_XHEAL => {
            let stats = thing_t(&raw mut player);
            (*pot_info.as_mut_ptr().add(P_XHEAL as usize)).oi_know = TRUE;
            (*stats).t_stats.s_hpt += roll((*stats).t_stats.s_lvl, 8);
            if (*stats).t_stats.s_hpt > (*stats).t_stats.s_maxhp {
                if (*stats).t_stats.s_hpt > (*stats).t_stats.s_maxhp + (*stats).t_stats.s_lvl + 1 {
                    (*stats).t_stats.s_maxhp += 1;
                }
                (*stats).t_stats.s_maxhp += 1;
                (*stats).t_stats.s_hpt = (*stats).t_stats.s_maxhp;
            }
            sight();
            come_down();
            msg(c"you begin to feel much better".as_ptr());
        }
        P_HASTE => {
            (*pot_info.as_mut_ptr().add(P_HASTE as usize)).oi_know = TRUE;
            after = FALSE;
            if add_haste(TRUE) != 0 {
                msg(c"you feel yourself moving much faster".as_ptr());
            }
        }
        P_RESTORE => {
            let stats = thing_t(&raw mut player);
            if ring_is(LEFT, R_ADDSTR) {
                add_str(&mut (*stats).t_stats.s_str, -(*thing_o(cur_ring[LEFT])).o_arm);
            }
            if ring_is(RIGHT, R_ADDSTR) {
                add_str(&mut (*stats).t_stats.s_str, -(*thing_o(cur_ring[RIGHT])).o_arm);
            }
            if (*stats).t_stats.s_str < max_stats.s_str {
                (*stats).t_stats.s_str = max_stats.s_str;
            }
            if ring_is(LEFT, R_ADDSTR) {
                add_str(&mut (*stats).t_stats.s_str, (*thing_o(cur_ring[LEFT])).o_arm);
            }
            if ring_is(RIGHT, R_ADDSTR) {
                add_str(&mut (*stats).t_stats.s_str, (*thing_o(cur_ring[RIGHT])).o_arm);
            }
            msg(c"hey, this tastes great.  It make you feel warm all over".as_ptr());
        }
        P_BLIND => do_pot_impl(P_BLIND, TRUE),
        P_LEVIT => do_pot_impl(P_LEVIT, TRUE),
        _ => {
            msg(c"what an odd tasting potion!".as_ptr());
            return;
        }
    }

    status();
    call_it(&mut pot_info[(*thing_o(obj)).o_which as usize]);
    if discardit {
        discard(obj);
    }
}

/// is_magic:
/// Returns true if an object radiates magic.
#[no_mangle]
pub unsafe extern "C" fn is_magic(obj: *mut CThing) -> c_uchar {
    if obj.is_null() {
        return 0;
    }
    if is_magic_local(obj) {
        1
    } else {
        0
    }
}

/// invis_on:
/// Turn on the ability to see invisible.
#[no_mangle]
pub unsafe extern "C" fn invis_on() {
    let mut mp = mlist;
    (*thing_t(&raw mut player)).t_flags |= CANSEE;
    while !mp.is_null() {
        if thing_has(mp, ISINVIS) && see_monst(mp) != 0 && !player_has(ISHALU) {
            mvaddch((*thing_t(mp)).t_pos.y, (*thing_t(mp)).t_pos.x, (*thing_t(mp)).t_disguise as c_uint);
        }
        mp = next_thing(mp);
    }
}

/// turn_see:
/// Put on or off seeing monsters on this level.
#[no_mangle]
pub unsafe extern "C" fn turn_see(turn_off: c_uchar) -> c_uchar {
    let mut mp = mlist;
    let mut add_new = 0;

    while !mp.is_null() {
        r#move((*thing_t(mp)).t_pos.y, (*thing_t(mp)).t_pos.x);
        let can_see = see_monst(mp) != 0;
        if turn_off != 0 {
            if !can_see {
                addch((*thing_t(mp)).t_oldch as c_uint);
            }
        } else {
            if !can_see {
                standout();
            }
            if !player_has(ISHALU) {
                addch((*thing_t(mp)).t_type as c_uint);
            } else {
                addch((rnd(26) + 'A' as c_int) as c_uint);
            }
            if !can_see {
                standend();
                add_new += 1;
            }
        }
        mp = next_thing(mp);
    }

    if turn_off != 0 {
        (*thing_t(&raw mut player)).t_flags &= !SEEMONST;
    } else {
        (*thing_t(&raw mut player)).t_flags |= SEEMONST;
    }

    if add_new != 0 { 1 } else { 0 }
}

/// seen_stairs:
/// Return true if the player has seen the stairs.
#[no_mangle]
pub unsafe extern "C" fn seen_stairs() -> c_uchar {
    let tp: *mut CThing;

    r#move(stairs.y, stairs.x);
    if inch() == STAIRS {
        return 1;
    }
    if hero().x == stairs.x && hero().y == stairs.y {
        return 1;
    }

    tp = moat(stairs.y, stairs.x);
    if !tp.is_null() {
        if see_monst(tp) != 0 && thing_has(tp, ISRUN) {
            return 1;
        }
        if player_has(SEEMONST) && (*thing_t(tp)).t_oldch as c_int == STAIRS {
            return 1;
        }
    }

    0
}

/// raise_level:
/// The player just magically went up a level.
#[no_mangle]
pub unsafe extern "C" fn raise_level() {
    (*thing_t(&raw mut player)).t_stats.s_exp = e_levels[(*thing_t(&raw mut player)).t_stats.s_lvl as usize - 1] + 1;
    check_level();
}

/// do_pot:
/// Do a potion with the standard fuse/flag setup.
#[no_mangle]
pub unsafe extern "C" fn do_pot(type_id: c_int, knowit: c_uchar) {
    do_pot_impl(type_id, knowit);
}
