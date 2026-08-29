//! Player command reading and dispatch.
//!
//! Ported from `src/c/command.c` to Rust.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.


use crate::player::{CCoord, CPlace, CThing, CThingMonster, CThingObject};
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint, c_void};

// ─── Constants ────────────────────────────────────────────────────────────────

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

const MAXSTR: usize = 1024;

// Glyphs
const PASSAGE: c_char = b'#' as c_char;
const DOOR: c_char = b'+' as c_char;
const FLOOR: c_char = b'.' as c_char;
const TRAP: c_char = b'^' as c_char;
const STAIRS: c_char = b'%' as c_char;
const GOLD: c_char = b'*' as c_char;
const POTION: c_char = b'!' as c_char;
const SCROLL: c_char = b'?' as c_char;
const FOOD: c_char = b':' as c_char;
const WEAPON: c_char = b')' as c_char;
const ARMOR: c_char = b']' as c_char;
const AMULET: c_char = b',' as c_char;
const RING: c_char = b'=' as c_char;
const STICK: c_char = b'/' as c_char;

// Object "types" used by get_item()
const CALLABLE: c_int = -1;

// Player flags
const ISBLIND: c_short = 0o0000004;
const ISGREED: c_short = 0o0000040;
const ISHASTE: c_short = 0o0000100;
const ISTARGET: c_short = 0o000200;
const ISHALU: c_short = 0o0004000;
const ISINVIS: c_short = 0o002000;
const ISLEVIT: c_short = 0o0000010;
const ISREGEN: c_short = 0o010000;
const ISRUN: c_short = 0o020000;
// C's `#define ISSLOW 0100000` == 0o100000 (32768) overflows a signed short;
// the Rust port stores the wrapped bit pattern like the C engine does.
const ISSLOW: c_short = 0o100000u16 as c_short;
const SEEMONST: c_short = 0o040000;

// Map flags
const F_REAL: c_char = 0x10u8 as c_char;
const F_SEEN: c_char = 0x40u8 as c_char;
const F_TMASK: c_char = 0x07u8 as c_char;

// Trap count
const NTRAPS: c_int = 8;

// Ring types
const R_SEARCH: c_int = 3;
const R_TELEPORT: c_int = 11;
const LEFT: usize = 0;
const RIGHT: usize = 1;

// Escape
const ESCAPE: c_int = 27;

// get_str() return codes
const NORM: c_int = 0;

// Weapon/armor kinds for the wizard ('^I' = CTRL-I) cheat
const TWOSWORD: c_int = 5;
const PLATE_MAIL: c_int = 7;
const ISKNOW: c_int = 0o0000002;

// Delayed-action phases
const BEFORE: c_int = 1;
const AFTER: c_int = 2;

/// CTRL(c) macro from rogue.h: `c & 037`.
///
/// Precomputed constants are used in `match` patterns (Rust does not allow
/// function calls in patterns, even for `const fn`s).
const CTRL_A: u8 = b'A' & 0x1f;    // 0x01
const CTRL_B: u8 = b'B' & 0x1f;    // 0x02
const CTRL_C: u8 = b'C' & 0x1f;    // 0x03
const CTRL_D: u8 = b'D' & 0x1f;    // 0x04
const CTRL_E: u8 = b'E' & 0x1f;    // 0x05
const CTRL_F: u8 = b'F' & 0x1f;    // 0x06
const CTRL_G: u8 = b'G' & 0x1f;    // 0x07
const CTRL_H: u8 = b'H' & 0x1f;    // 0x08
const CTRL_I: u8 = b'I' & 0x1f;    // 0x09
const CTRL_J: u8 = b'J' & 0x1f;    // 0x0a
const CTRL_K: u8 = b'K' & 0x1f;    // 0x0b
const CTRL_L: u8 = b'L' & 0x1f;    // 0x0c
const CTRL_N: u8 = b'N' & 0x1f;    // 0x0e
const CTRL_P: u8 = b'P' & 0x1f;    // 0x10
const CTRL_R: u8 = b'R' & 0x1f;    // 0x12
const CTRL_T: u8 = b'T' & 0x1f;    // 0x14
const CTRL_U: u8 = b'U' & 0x1f;    // 0x15
const CTRL_W: u8 = b'W' & 0x1f;    // 0x17
const CTRL_X: u8 = b'X' & 0x1f;    // 0x18
const CTRL_Y: u8 = b'Y' & 0x1f;    // 0x19
const CTRL_TILDE: u8 = b'~' & 0x1f; // CTRL-~ (0x1e)

/// Inline evaluator for CTRL(c) used in expression position.
#[inline]
const fn ctrl(c: u8) -> u8 {
    c & 0x1f
}

/// Wizard-mode: the preprocessor conditional in the C code is replaced by a
/// runtime check on the `wizard` global.  All wizard helpers are always
/// compiled in, matching the style used by wizard.rs, chase.rs and friends.
const MASTER: bool = true;

// ─── C ABI structs ────────────────────────────────────────────────────────────

/// Mirrors the C `struct h_list` used by the help table.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CHList {
    pub h_ch: c_char,
    pub h_desc: *mut c_char,
    pub h_print: c_uchar,
}

/// Mirrors the C `struct obj_info`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CObjInfo {
    pub oi_name: *mut c_char,
    pub oi_prob: c_int,
    pub oi_worth: c_int,
    pub oi_guess: *mut c_char,
    pub oi_know: c_uchar,
}

// ─── Static locals for command() ─────────────────────────────────────────────

static mut COUNTCH: c_char = 0;
static mut DIRECTION: c_char = 0;
static mut NEWCOUNT: c_uchar = FALSE;

/// Identify table (static in the C `identify()`).  Uses `&'static str` so the
/// table can live in an immutable `static` (raw pointers are not `Sync`).
struct IdentItem {
    ch: u8,
    desc: &'static str,
}

static IDENT_LIST: [IdentItem; 18] = [
    IdentItem { ch: b'|', desc: "wall of a room" },
    IdentItem { ch: b'-', desc: "wall of a room" },
    IdentItem { ch: GOLD as u8, desc: "gold" },
    IdentItem { ch: STAIRS as u8, desc: "a staircase" },
    IdentItem { ch: DOOR as u8, desc: "door" },
    IdentItem { ch: FLOOR as u8, desc: "room floor" },
    IdentItem { ch: b'@', desc: "you" },
    IdentItem { ch: PASSAGE as u8, desc: "passage" },
    IdentItem { ch: TRAP as u8, desc: "trap" },
    IdentItem { ch: POTION as u8, desc: "potion" },
    IdentItem { ch: SCROLL as u8, desc: "scroll" },
    IdentItem { ch: FOOD as u8, desc: "food" },
    IdentItem { ch: WEAPON as u8, desc: "weapon" },
    IdentItem { ch: b' ', desc: "solid rock" },
    IdentItem { ch: ARMOR as u8, desc: "armor" },
    IdentItem { ch: AMULET as u8, desc: "the Amulet of Yendor" },
    IdentItem { ch: RING as u8, desc: "ring" },
    IdentItem { ch: STICK as u8, desc: "wand or staff" },
];

// ─── Extern C globals ─────────────────────────────────────────────────────────

unsafe extern "C" {
    static mut after: c_uchar;
    static mut again: c_uchar;
    static mut amulet: c_uchar;
    static mut count: c_int;
    static mut cur_armor: *mut CThing;
    static mut cur_ring: [*mut CThing; 2];
    static mut cur_weapon: *mut CThing;
    static mut curscr: *mut c_void;
    static mut delta: CCoord;
    static mut dir_ch: c_char;
    static mut dnum: c_int;
    static mut door_stop: c_uchar;
    static mut firstmove: c_uchar;
    static mut food_left: c_int;
    static mut has_hit: c_uchar;
    static mut helpstr: [CHList; 80];
    static mut huh: [c_char; MAXSTR];
    static mut hw: *mut c_void;
    static mut inpack: c_int;
    static mut inv_describe: c_uchar;
    static mut jump: c_uchar;
    static mut kamikaze: c_uchar;
    static mut l_last_comm: c_char;
    static mut l_last_dir: c_char;
    static mut l_last_pick: *mut CThing;
    static mut last_comm: c_char;
    static mut last_dir: c_char;
    static mut last_pick: *mut CThing;
    static mut lastscore: c_int;
    static mut level: c_int;
    static mut lower_msg: c_uchar;
    static mut lvl_obj: *mut CThing;
    static mut max_hit: c_int;
    static mut move_on: c_uchar;
    static mut mpos: c_int;
    static mut no_command: c_int;
    static mut noscore: c_int;
    static mut places: [CPlace; 32 * 80];
    static mut player: CThing;
    static mut prbuf: [c_char; 2 * MAXSTR];
    static mut purse: c_int;
    static mut q_comm: c_uchar;
    static mut release: *mut c_char;
    static mut runch: c_char;
    static mut running: c_uchar;
    static mut save_msg: c_uchar;
    static mut seenstairs: c_uchar;
    static mut stat_msg: c_uchar;
    static mut stdscr: *mut c_void;
    static mut take: c_char;
    static mut terse: c_uchar;
    static mut to_death: c_uchar;
    static mut monsters: [crate::monsters::CMonster; 26];
    static mut tr_name: [*mut c_char; NTRAPS as usize];
    static mut r_stones: [*mut c_char; 14];
    static mut p_colors: [*mut c_char; 14];
    static mut s_names: [*mut c_char; 18];
    static mut ws_made: [*mut c_char; 14];
    static mut ring_info: [CObjInfo; 14];
    static mut pot_info: [CObjInfo; 14];
    static mut scr_info: [CObjInfo; 18];
    static mut ws_info: [CObjInfo; 14];
    static mut wizard: c_int;
    static mut LINES: c_int;
    static mut COLS: c_int;
}

// ─── Extern C functions called from this module ───────────────────────────────

unsafe extern "C" {
    fn addmsg(fmt: *const c_char, ...);
    fn add_pack(obj: *mut CThing, all: c_uchar);
    fn add_pass();
    fn clearok(win: *mut c_void, bf: c_uchar) -> c_int;
    fn create_obj();
    fn diag_ok(sp: *mut CCoord, ep: *mut CCoord) -> c_uchar;
    fn discovered();
    fn do_daemons(flag: c_int);
    fn do_fuses(flag: c_int);
    fn do_move(dy: c_int, dx: c_int);
    fn do_run(ch: c_char);
    fn do_zap();
    fn drop();
    fn eat();
    fn endmsg() -> c_int;
    fn free(ptr: *mut c_void);
    fn get_dir() -> c_uchar;
    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn get_str(s: *mut c_char, win: *mut c_void) -> c_int;
    fn init_weapon(obj: *mut CThing, which: c_int);
    fn inventory(list: *mut CThing, item_type: c_int) -> c_uchar;
    fn inv_name(obj: *mut CThing, drop: c_uchar) -> *mut c_char;
    fn isupper(c: c_int) -> c_int;
    fn look(wakeup: c_uchar);
    fn malloc(size: usize) -> *mut c_void;
    fn missile(ydelta: c_int, xdelta: c_int);
    #[link_name = "move"]
    fn move_(y: c_int, x: c_int);
    fn msg(fmt: *const c_char, ...);
    fn new_item() -> *mut CThing;
    fn new_level();
    fn option();
    fn pick_up(ch: c_char);
    fn picky_inven();
    fn quaff();
    fn quit(sig: c_int);
    fn raise_level();
    fn read_scroll();
    fn readchar() -> c_int;
    fn refresh() -> c_int;
    fn rnd(range: c_int) -> c_int;
    fn save_game();
    fn see_monst(mp: *mut CThing) -> c_uchar;
    fn shell();
    fn show_map();
    fn status();
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strlen(s: *const c_char) -> usize;
    fn take_off();
    fn teleport();
    fn total_winner();
    fn touchwin(win: *mut c_void) -> c_int;
    fn turn_see(turn_off: c_uchar) -> c_uchar;
    fn unctrl(ch: c_int) -> *mut c_char;
    fn wait_for(ch: c_int);
    fn waddstr(win: *mut c_void, s: *const c_char) -> c_int;
    fn wclear(win: *mut c_void) -> c_int;
    fn wear();
    fn whatis(insist: c_uchar, item_type: c_int);
    fn wield();
    fn wmove(win: *mut c_void, y: c_int, x: c_int) -> c_int;
    fn wrefresh(win: *mut c_void) -> c_int;
    fn ring_on();
    fn ring_off();
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
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    crate::draw::chat_at(y, x)
}

#[inline]
unsafe fn moat_at(y: c_int, x: c_int) -> *mut CThing {
    crate::game::monster_at(y, x)
}

#[inline]
unsafe fn isring(hand: usize, ring_type: c_int) -> bool {
    !cur_ring[hand].is_null() && (*thing_o(cur_ring[hand])).o_which == ring_type
}

// ─── command() ────────────────────────────────────────────────────────────────

/// command:
/// Process the user commands.
///
/// Uses globals: player, has_hit, running, door_stop, lastscore,
/// purse, hero, jump, take, after, wizard, noscore, no_command,
/// count, move_on, mpos, runch, to_death, countch, l_last_comm /
/// last_comm / last_dir / last_pick (via reset_last/last_*), lvl_obj,
/// terse, mlist (via moat), max_hit, mp/t_flags (via to_death),
/// dir_ch, delta, q_comm, huh, release, amulet, level, seenstairs,
/// tr_name, stat_msg, inpack, food_left, cur_weapon, cur_armor,
/// cur_ring, inv_describe.
#[no_mangle]
pub unsafe extern "C" fn command() {
    let mut ch: u8;
    let mut ntimes: c_int = 1; // Number of player moves
    let mut mp: *mut CThing;

    if player_has(ISHASTE) {
        ntimes += 1;
    }

    /*
     * Let the daemons start up
     */
    do_daemons(BEFORE);
    do_fuses(BEFORE);

    while ntimes > 0 {
        ntimes -= 1;
        again = FALSE;
        if has_hit != 0 {
            endmsg();
            has_hit = FALSE;
        }

        /*
         * these are illegal things for the player to be, so if any are
         * set, someone's been poking in memory
         */
        if player_has(ISSLOW | ISGREED | ISINVIS | ISREGEN | ISTARGET) {
            std::process::exit(1);
        }

        look(TRUE);
        if running == 0 {
            door_stop = FALSE;
        }
        status();
        lastscore = purse;
        let hero = hero_pos();
        move_(hero.y, hero.x);
        if !((running != 0 || count != 0) && jump != 0) {
            refresh(); // Draw screen
        }
        take = 0;
        after = TRUE;

        /*
         * Read command or continue run
         */
        if MASTER && wizard != 0 {
            noscore = 1;
        }

        if no_command == 0 {
            if running != 0 || to_death != 0 {
                ch = runch as u8;
            } else if count != 0 {
                ch = COUNTCH as u8;
            } else {
                ch = readchar() as u8;
                move_on = FALSE;
                if mpos != 0 {
                    // Erase message if it's there
                    msg(c"".as_ptr());
                }
            }
        } else {
            ch = b'.';
        }

        if no_command != 0 {
            no_command -= 1;
            if no_command == 0 {
                let tp = thing_t(&raw mut player);
                (*tp).t_flags = (((*tp).t_flags as c_short) | ISRUN as c_short) as c_short;
                msg(c"you can move again".as_ptr());
            }
        } else {
            /*
             * check for prefixes
             */
            NEWCOUNT = FALSE;
            if ch.is_ascii_digit() {
                count = 0;
                NEWCOUNT = TRUE;
                while ch.is_ascii_digit() {
                    count = count * 10 + (ch - b'0') as c_int;
                    if count > 255 {
                        count = 255;
                    }
                    ch = readchar() as u8;
                }
                COUNTCH = ch as c_char;
                /*
                 * turn off count for commands which don't make sense
                 * to repeat
                 */
                if !matches!(
                    ch,
                    CTRL_B
                        | CTRL_H
                        | CTRL_J
                        | CTRL_K
                        | CTRL_L
                        | CTRL_N
                        | CTRL_U
                        | CTRL_Y
                        | b'.'
                        | b'a'
                        | b'b'
                        | b'h'
                        | b'j'
                        | b'k'
                        | b'l'
                        | b'm'
                        | b'n'
                        | b'q'
                        | b'r'
                        | b's'
                        | b't'
                        | b'u'
                        | b'y'
                        | b'z'
                        | b'B'
                        | b'C'
                        | b'H'
                        | b'I'
                        | b'J'
                        | b'K'
                        | b'L'
                        | b'N'
                        | b'U'
                        | b'Y'
                        | CTRL_D
                        | CTRL_A
                ) {
                    count = 0;
                }
            }

            /*
             * execute a command
             */
            if count != 0 && running == 0 {
                count -= 1;
            }
            if ch != b'a' && ch != ESCAPE as u8 && running == 0 && count == 0 && to_death == 0 {
                l_last_comm = last_comm;
                l_last_dir = last_dir;
                l_last_pick = last_pick;
                last_comm = ch as c_char;
                last_dir = b'\0' as c_char;
                last_pick = std::ptr::null_mut();
            }

            // ── Command dispatch ────────────────────────────────────────────
            // The C code uses `goto over` from a few arms; we emulate it with
            // a labelled loop: arms that re-dispatch set `ch` and `continue`.
            'dispatch: loop {
                match ch {
                    b',' => {
                        let hero = hero_pos();
                        let mut obj = lvl_obj;
                        let mut found = false;
                        while !obj.is_null() {
                            if (*thing_o(obj)).o_pos.y == hero.y && (*thing_o(obj)).o_pos.x == hero.x
                            {
                                found = true;
                                break;
                            }
                            obj = (*thing_t(obj)).l_next;
                        }

                        if found {
                            if levit_check() == 0 {
                                pick_up((*thing_o(obj)).o_type as c_char);
                            }
                        } else {
                            if terse == 0 {
                                addmsg(c"there is ".as_ptr());
                            }
                            addmsg(c"nothing here".as_ptr());
                            if terse == 0 {
                                addmsg(c" to pick up".as_ptr());
                            }
                            endmsg();
                        }
                    }
                    b'!' => {
                        shell();
                    }
                    b'h' => do_move(0, -1),
                    b'j' => do_move(1, 0),
                    b'k' => do_move(-1, 0),
                    b'l' => do_move(0, 1),
                    b'y' => do_move(-1, -1),
                    b'u' => do_move(-1, 1),
                    b'b' => do_move(1, -1),
                    b'n' => do_move(1, 1),
                    b'H' => do_run(b'h' as c_char),
                    b'J' => do_run(b'j' as c_char),
                    b'K' => do_run(b'k' as c_char),
                    b'L' => do_run(b'l' as c_char),
                    b'Y' => do_run(b'y' as c_char),
                    b'U' => do_run(b'u' as c_char),
                    b'B' => do_run(b'b' as c_char),
                    b'N' => do_run(b'n' as c_char),
                    v
                        if v == ctrl(b'H')
                            || v == ctrl(b'J')
                            || v == ctrl(b'K')
                            || v == ctrl(b'L')
                            || v == ctrl(b'Y')
                            || v == ctrl(b'U')
                            || v == ctrl(b'B')
                            || v == ctrl(b'N') =>
                    {
                        if !player_has(ISBLIND) {
                            door_stop = TRUE;
                            firstmove = TRUE;
                        }
                        if count != 0 && NEWCOUNT == 0 {
                            ch = DIRECTION as u8;
                        } else {
                            // ('A' - CTRL('A')) == 64
                            ch = ch.wrapping_add(64);
                            DIRECTION = ch as c_char;
                        }
                        continue 'dispatch;
                    }
                    b'f' | b'F' => {
                        if ch == b'F' {
                            kamikaze = TRUE;
                        }
                        if get_dir() == 0 {
                            after = FALSE;
                        } else {
                            let hero = hero_pos();
                            delta.y += hero.y;
                            delta.x += hero.x;
                            mp = moat_at(delta.y, delta.x);
                            if mp.is_null() || (see_monst(mp) == 0 && !player_has(SEEMONST)) {
                                if terse == 0 {
                                    addmsg(c"I see ".as_ptr());
                                }
                                msg(c"no monster there".as_ptr());
                                after = FALSE;
                            } else if diag_ok(hero_ptr(), &raw mut delta) != 0 {
                                to_death = TRUE;
                                max_hit = 0;
                                (*thing_t(mp)).t_flags =
                                    ((*thing_t(mp)).t_flags as c_short | ISTARGET as c_short)
                                        as c_short;
                                runch = dir_ch;
                                ch = dir_ch as u8;
                                continue 'dispatch;
                            }
                        }
                    }
                    b't' => {
                        if get_dir() == 0 {
                            after = FALSE;
                        } else {
                            missile(delta.y, delta.x);
                        }
                    }
                    b'a' => {
                        if last_comm == 0 {
                            msg(c"you haven't typed a command yet".as_ptr());
                            after = FALSE;
                        } else {
                            ch = last_comm as u8;
                            again = TRUE;
                            continue 'dispatch;
                        }
                    }
                    b'q' => quaff(),
                    b'Q' => {
                        after = FALSE;
                        q_comm = TRUE;
                        quit(0);
                        q_comm = FALSE;
                    }
                    b'i' => {
                        after = FALSE;
                        inventory((*thing_t(&raw mut player)).t_pack, 0);
                    }
                    b'I' => {
                        after = FALSE;
                        picky_inven();
                    }
                    b'd' => drop(),
                    b'r' => read_scroll(),
                    b'e' => eat(),
                    b'w' => wield(),
                    b'W' => wear(),
                    b'T' => take_off(),
                    b'P' => ring_on(),
                    b'R' => ring_off(),
                    b'o' => {
                        option();
                        after = FALSE;
                    }
                    b'c' => {
                        call();
                        after = FALSE;
                    }
                    b'>' => {
                        after = FALSE;
                        d_level();
                    }
                    b'<' => {
                        after = FALSE;
                        u_level();
                    }
                    b'?' => {
                        after = FALSE;
                        help();
                    }
                    b'/' => {
                        after = FALSE;
                        identify();
                    }
                    b's' => search(),
                    b'z' => {
                        if get_dir() != 0 {
                            do_zap();
                        } else {
                            after = FALSE;
                        }
                    }
                    b'D' => {
                        after = FALSE;
                        discovered();
                    }
                    CTRL_P => {
                        after = FALSE;
                        msg(c"%s".as_ptr(), huh.as_mut_ptr());
                    }
                    CTRL_R => {
                        after = FALSE;
                        clearok(curscr, TRUE);
                        wrefresh(curscr);
                    }
                    b'v' => {
                        after = FALSE;
                        msg(c"version %s. (mctesq was here)".as_ptr(), release);
                    }
                    b'S' => {
                        after = FALSE;
                        save_game();
                    }
                    b'.' => {
                        // Rest command
                    }
                    b' ' => {
                        after = FALSE; // "Legal" illegal command
                    }
                    b'^' => {
                        after = FALSE;
                        if get_dir() != 0 {
                            let hero = hero_pos();
                            delta.y += hero.y;
                            delta.x += hero.x;
                            if terse == 0 {
                                addmsg(c"You have found ".as_ptr());
                            }
                            if !crate::draw::is_trap_cell(delta.y, delta.x) {
                                msg(c"no trap there".as_ptr());
                            } else if player_has(ISHALU) {
                                msg(c"%s".as_ptr(), tr_name[rnd(NTRAPS) as usize]);
                            } else {
                                msg(
                                    c"%s".as_ptr(),
                                    tr_name[crate::draw::trap_kind_at(delta.y, delta.x) as usize],
                                );
                                crate::draw::set_seen_at(delta.y, delta.x);
                            }
                        }
                    }
                    b'+' => {
                        // Wizard toggle (was the `when '+'` arm under `#ifdef MASTER`)
                        after = FALSE;
                        if MASTER {
                            if wizard != 0 {
                                wizard = 0;
                                turn_see(TRUE);
                                msg(c"not wizard any more".as_ptr());
                            } else {
                                wizard = 1;
                                noscore = 1;
                                turn_see(FALSE);
                                msg(
                                    c"you are suddenly as smart as Ken Arnold in dungeon #%d"
                                        .as_ptr(),
                                    dnum,
                                );
                            }
                        }
                    }
                    v if v == ESCAPE as u8 => {
                        door_stop = FALSE;
                        count = 0;
                        after = FALSE;
                        again = FALSE;
                    }
                    b'm' => {
                        move_on = TRUE;
                        if get_dir() == 0 {
                            after = FALSE;
                        } else {
                            ch = dir_ch as u8;
                            COUNTCH = dir_ch;
                            continue 'dispatch;
                        }
                    }
                    b')' => {
                        current(cur_weapon, c"wielding".as_ptr(), std::ptr::null_mut());
                    }
                    b']' => {
                        current(cur_armor, c"wearing".as_ptr(), std::ptr::null_mut());
                    }
                    b'=' => {
                        current(
                            cur_ring[LEFT],
                            c"wearing".as_ptr(),
                            if terse != 0 {
                                c"(L)".as_ptr()
                            } else {
                                c"on left hand".as_ptr()
                            },
                        );
                        current(
                            cur_ring[RIGHT],
                            c"wearing".as_ptr(),
                            if terse != 0 {
                                c"(R)".as_ptr()
                            } else {
                                c"on right hand".as_ptr()
                            },
                        );
                    }
                    b'@' => {
                        stat_msg = TRUE;
                        status();
                        stat_msg = FALSE;
                        after = FALSE;
                    }
                    _ => {
                        after = FALSE;
                        if MASTER && wizard != 0 {
                            match ch {
                                b'|' => {
                                    let hero = hero_pos();
                                    msg(c"@ %d,%d".as_ptr(), hero.y, hero.x);
                                }
                                b'C' => create_obj(),
                                b'$' => msg(c"inpack = %d".as_ptr(), inpack),
                                CTRL_G => {
                                    let _ = inventory(lvl_obj, 0);
                                }
                                CTRL_W => whatis(FALSE, 0),
                                CTRL_D => {
                                    level += 1;
                                    new_level();
                                }
                                CTRL_A => {
                                    level -= 1;
                                    new_level();
                                }
                                CTRL_F => show_map(),
                                CTRL_T => teleport(),
                                CTRL_E => msg(c"food left: %d".as_ptr(), food_left),
                                CTRL_C => add_pass(),
                                CTRL_X => {
                                    turn_see(if player_has(SEEMONST) { TRUE } else { FALSE });
                                }
                                CTRL_TILDE => {
                                    let item = get_item(c"charge".as_ptr(), STICK as c_int);
                                    if !item.is_null() {
                                        (*thing_o(item)).o_arm = 10000;
                                    }
                                }
                                CTRL_I => {
                                    let mut obj: *mut CThing;

                                    for _ in 0..9 {
                                        raise_level();
                                    }
                                    /*
                                     * Give him a sword (+1,+1)
                                     */
                                    obj = new_item();
                                    init_weapon(obj, TWOSWORD);
                                    (*thing_o(obj)).o_hplus = 1;
                                    (*thing_o(obj)).o_dplus = 1;
                                    add_pack(obj, TRUE);
                                    cur_weapon = obj;
                                    /*
                                     * And his suit of armor
                                     */
                                    obj = new_item();
                                    (*thing_o(obj)).o_type = ARMOR as c_int;
                                    (*thing_o(obj)).o_which = PLATE_MAIL;
                                    (*thing_o(obj)).o_arm = -5;
                                    (*thing_o(obj)).o_flags = (*thing_o(obj)).o_flags | ISKNOW;
                                    (*thing_o(obj)).o_count = 1;
                                    (*thing_o(obj)).o_group = 0;
                                    cur_armor = obj;
                                    add_pack(obj, TRUE);
                                }
                                b'*' => pr_list(),
                                _ => illcom(ch as c_int),
                            }
                        } else {
                            illcom(ch as c_int);
                        }
                    }
                }
                break; // Fall out of the dispatch loop; C's `break` out of switch.
            }
        }

        /*
         * If he ran into something to take, let him pick it up.
         */
        if take != 0 {
            pick_up(take);
        }
        if running == 0 {
            door_stop = FALSE;
        }
        if after == 0 {
            ntimes += 1;
        }
    }

    do_daemons(AFTER);
    do_fuses(AFTER);
    if isring(LEFT, R_SEARCH) {
        search();
    } else if isring(LEFT, R_TELEPORT) && rnd(50) == 0 {
        teleport();
    }
    if isring(RIGHT, R_SEARCH) {
        search();
    } else if isring(RIGHT, R_TELEPORT) && rnd(50) == 0 {
        teleport();
    }
}

// ─── illcom() ─────────────────────────────────────────────────────────────────

/// illcom:
/// What to do with an illegal command.
///
/// Uses globals: save_msg, count.
#[no_mangle]
pub unsafe extern "C" fn illcom(ch: c_int) {
    save_msg = FALSE;
    count = 0;
    msg(c"illegal command '%s'".as_ptr(), unctrl(ch));
    save_msg = TRUE;
}

// ─── search() ─────────────────────────────────────────────────────────────────

/// search:
/// Player gropes about him to find hidden things.
///
/// Uses globals: hero, player, places (via chat/flat), count, running,
/// terse, tr_name.
#[no_mangle]
pub unsafe extern "C" fn search() {
    let hero = hero_pos();
    let ey = hero.y + 1;
    let ex = hero.x + 1;
    let mut probinc: c_int = 0;
    let mut found = false;

    if player_has(ISHALU) {
        probinc += 3;
    }
    if player_has(ISBLIND) {
        probinc += 2;
    }

    let mut y = hero.y - 1;
    while y <= ey {
        let mut x = hero.x - 1;
        while x <= ex {
            if y == hero.y && x == hero.x {
                x += 1;
                continue;
            }
            let flags = crate::draw::flat_at(y, x);
            if (flags as u8 & F_REAL as u8) == 0 {
                match chat_at(y, x) as u8 {
                    b'|' | b'-' => {
                        if rnd(5 + probinc) == 0 {
                            crate::draw::reveal_secret_at(y, x);
                            msg(c"a secret door".as_ptr());
                            found = true;
                            count = FALSE as c_int;
                            running = FALSE;
                        }
                    }
                    b'.' => {
                        if rnd(2 + probinc) == 0 {
                            crate::draw::reveal_trap_at(y, x);
                            if terse == 0 {
                                addmsg(c"you found ".as_ptr());
                            }
                            if player_has(ISHALU) {
                                msg(c"%s".as_ptr(), tr_name[rnd(NTRAPS) as usize]);
                            } else {
                                msg(
                                    c"%s".as_ptr(),
                                    tr_name[crate::draw::trap_kind_at(y, x) as usize],
                                );
                            }
                            found = true;
                            count = FALSE as c_int;
                            running = FALSE;
                        }
                    }
                    b' ' => {
                        if rnd(3 + probinc) == 0 {
                            crate::draw::reveal_secret_at(y, x);
                            found = true;
                            count = FALSE as c_int;
                            running = FALSE;
                        }
                    }
                    _ => {}
                }
            }
            x += 1;
        }
        y += 1;
    }

    if found {
        look(FALSE);
    }
}

// ─── help() ───────────────────────────────────────────────────────────────────

/// help:
/// Give single character help, or the whole mess if he wants it.
///
/// Uses globals: mpos, helpstr, lower_msg, hw.
#[no_mangle]
pub unsafe extern "C" fn help() {
    let mut helpch: c_char;
    let mut numprint: c_int;
    let mut cnt: c_int;

    msg(c"character you want help for (* for all): ".as_ptr());
    helpch = readchar() as c_char;
    mpos = 0;

    /*
     * If it's not a *, print the right help string
     * or an error if he typed a funny character.
     */
    if helpch != b'*' as c_char {
        move_(0, 0);
        let mut i = 0;
        while i < helpstr.len() && !helpstr[i].h_desc.is_null() {
            if helpstr[i].h_ch == helpch {
                lower_msg = TRUE;
                msg(
                    c"%s%s".as_ptr(),
                    unctrl(helpstr[i].h_ch as c_int),
                    helpstr[i].h_desc,
                );
                lower_msg = FALSE;
                return;
            }
            i += 1;
        }
        msg(c"unknown character '%s'".as_ptr(), unctrl(helpch as c_int));
        return;
    }

    /*
     * Here we print help for everything.
     * Then wait before we return to command mode
     */
    numprint = 0;
    let mut i = 0;
    while i < helpstr.len() && !helpstr[i].h_desc.is_null() {
        if helpstr[i].h_print != 0 {
            numprint += 1;
        }
        i += 1;
    }
    if numprint & 01 != 0 {
        numprint += 1;
    }
    numprint /= 2;
    if numprint > LINES - 1 {
        numprint = LINES - 1;
    }

    wclear(hw);
    cnt = 0;
    let mut i = 0;
    while i < helpstr.len() && !helpstr[i].h_desc.is_null() && cnt < numprint * 2 {
        if helpstr[i].h_print != 0 {
            wmove(hw, cnt % numprint, if cnt >= numprint { COLS / 2 } else { 0 });
            if helpstr[i].h_ch != 0 {
                waddstr(hw, unctrl(helpstr[i].h_ch as c_int));
            }
            waddstr(hw, helpstr[i].h_desc);
            cnt += 1;
        }
        i += 1;
    }
    wmove(hw, LINES - 1, 0);
    waddstr(hw, c"--Press space to continue--".as_ptr());
    wrefresh(hw);
    wait_for(b' ' as c_int);
    clearok(stdscr, TRUE);
    msg(c"".as_ptr());
    touchwin(stdscr);
    wrefresh(stdscr);
}

// ─── identify() ───────────────────────────────────────────────────────────────

/// identify:
/// Tell the player what a certain thing is.
///
/// Uses globals: mpos, monsters.
#[no_mangle]
pub unsafe extern "C" fn identify() {
    let mut ch: c_int;
    let mut str_: *const c_char;

    msg(c"what do you want identified? ".as_ptr());
    ch = readchar();
    mpos = 0;
    if ch == ESCAPE {
        msg(c"".as_ptr());
        return;
    }

    if isupper(ch) != 0 {
        str_ = monsters[(ch - b'A' as c_int) as usize].m_name;
    } else {
        str_ = c"unknown character".as_ptr();
        for hp in IDENT_LIST.iter() {
            if hp.ch as c_int == ch {
                str_ = hp.desc.as_ptr() as *const c_char;
                break;
            }
        }
    }
    msg(c"'%s': %s".as_ptr(), unctrl(ch), str_);
}

// ─── d_level() / u_level() / levit_check() ────────────────────────────────────

/// d_level:
/// He wants to go down a level.
///
/// Uses globals: hero, places (via chat), level, seenstairs.
#[no_mangle]
pub unsafe extern "C" fn d_level() {
    if levit_check() != 0 {
        return;
    }
    let hero = hero_pos();
    if chat_at(hero.y, hero.x) != STAIRS {
        msg(c"I see no way down".as_ptr());
    } else {
        level += 1;
        seenstairs = FALSE;
        new_level();
    }
}

/// u_level:
/// He wants to go up a level.
///
/// Uses globals: hero, places (via chat), amulet, level.
#[no_mangle]
pub unsafe extern "C" fn u_level() {
    if levit_check() != 0 {
        return;
    }
    let hero = hero_pos();
    if chat_at(hero.y, hero.x) == STAIRS {
        if amulet != 0 {
            level -= 1;
            if level == 0 {
                total_winner();
            }
            new_level();
            msg(c"you feel a wrenching sensation in your gut".as_ptr());
        } else {
            msg(c"your way is magically blocked".as_ptr());
        }
    } else {
        msg(c"I see no way up".as_ptr());
    }
}

/// levit_check:
/// Check to see if she's levitating, and if she is, print an
/// appropriate message.
///
/// Uses globals: player.
#[no_mangle]
pub unsafe extern "C" fn levit_check() -> c_uchar {
    if !player_has(ISLEVIT) {
        return FALSE;
    }
    msg(c"You can't.  You're floating off the ground!".as_ptr());
    TRUE
}

// ─── call() ───────────────────────────────────────────────────────────────────

/// call:
/// Allow a user to call a potion, scroll, or ring something.
///
/// Uses globals: ring_info, r_stones, pot_info, p_colors, scr_info,
/// s_names, ws_info, ws_made, terse, prbuf.
#[no_mangle]
pub unsafe extern "C" fn call() {
    let obj = get_item(c"call".as_ptr(), CALLABLE);

    // Make certain that it's something that we want to wear
    if obj.is_null() {
        return;
    }

    let mut op: *mut CObjInfo = std::ptr::null_mut();
    let mut guess: *mut *mut c_char = std::ptr::null_mut();
    let mut know: *mut c_uchar = std::ptr::null_mut();
    let mut elsewise: *mut c_char = std::ptr::null_mut();

    match (*thing_o(obj)).o_type as u8 {
        x if x == RING as u8 => {
            op = ring_info.as_mut_ptr().add((*thing_o(obj)).o_which as usize);
            elsewise = r_stones[(*thing_o(obj)).o_which as usize];
            know = &raw mut (*op).oi_know;
            guess = &raw mut (*op).oi_guess;
            if !(*guess).is_null() {
                elsewise = *guess;
            }
        }
        x if x == POTION as u8 => {
            op = pot_info.as_mut_ptr().add((*thing_o(obj)).o_which as usize);
            elsewise = p_colors[(*thing_o(obj)).o_which as usize];
            know = &raw mut (*op).oi_know;
            guess = &raw mut (*op).oi_guess;
            if !(*guess).is_null() {
                elsewise = *guess;
            }
        }
        x if x == SCROLL as u8 => {
            op = scr_info.as_mut_ptr().add((*thing_o(obj)).o_which as usize);
            elsewise = s_names[(*thing_o(obj)).o_which as usize];
            know = &raw mut (*op).oi_know;
            guess = &raw mut (*op).oi_guess;
            if !(*guess).is_null() {
                elsewise = *guess;
            }
        }
        x if x == STICK as u8 => {
            op = ws_info.as_mut_ptr().add((*thing_o(obj)).o_which as usize);
            elsewise = ws_made[(*thing_o(obj)).o_which as usize];
            know = &raw mut (*op).oi_know;
            guess = &raw mut (*op).oi_guess;
            if !(*guess).is_null() {
                elsewise = *guess;
            }
        }
        x if x == FOOD as u8 => {
            msg(c"you can't call that anything".as_ptr());
            return;
        }
        _ => {
            guess = &raw mut (*thing_o(obj)).o_label;
            know = std::ptr::null_mut();
            elsewise = (*thing_o(obj)).o_label;
        }
    }

    if !know.is_null() && *know != 0 {
        msg(c"that has already been identified".as_ptr());
        return;
    }
    if !elsewise.is_null() && !guess.is_null() && elsewise == *guess {
        if terse == 0 {
            addmsg(c"Was ".as_ptr());
        }
        msg(c"called \"%s\"".as_ptr(), elsewise);
    }

    if terse != 0 {
        msg(c"call it: ".as_ptr());
    } else {
        msg(c"what do you want to call it? ".as_ptr());
    }

    if elsewise.is_null() {
        prbuf[0] = 0;
    } else {
        strcpy(prbuf.as_mut_ptr(), elsewise);
    }
    if get_str(prbuf.as_mut_ptr(), stdscr) == NORM {
        if !(*guess).is_null() {
            free(*guess as *mut c_void);
        }
        let len = strlen(prbuf.as_ptr()) + 1;
        let buf = malloc(len) as *mut c_char;
        if !buf.is_null() {
            strcpy(buf, prbuf.as_ptr());
            *guess = buf;
        }
    }
}

// ─── current() ────────────────────────────────────────────────────────────────

/// current:
/// Print the current weapon/armor.
///
/// Uses globals: after, terse, inv_describe.
#[no_mangle]
pub unsafe extern "C" fn current(cur: *mut CThing, how: *const c_char, where_: *const c_char) {
    after = FALSE;
    if !cur.is_null() {
        if terse == 0 {
            addmsg(c"you are %s (".as_ptr(), how);
        }
        inv_describe = FALSE;
        addmsg(c"%c) %s".as_ptr(), (*thing_o(cur)).o_packch as c_uint, inv_name(cur, TRUE));
        inv_describe = TRUE;
        if !where_.is_null() {
            addmsg(c" %s".as_ptr(), where_);
        }
        endmsg();
    } else {
        if terse == 0 {
            addmsg(c"you are ".as_ptr());
        }
        addmsg(c"%s nothing".as_ptr(), how);
        if !where_.is_null() {
            addmsg(c" %s".as_ptr(), where_);
        }
        endmsg();
    }
}

// ─── pr_list() ────────────────────────────────────────────────────────────────

/// pr_list:
/// Wizard command to list the objects on the current level.
///
/// Uses globals: lvl_obj, mlist.
#[no_mangle]
pub unsafe extern "C" fn pr_list() {
    let mut obj = lvl_obj;
    while !obj.is_null() {
        msg(
            c"%c) %s".as_ptr(),
            (*thing_o(obj)).o_type as c_uint,
            inv_name(obj, FALSE),
        );
        obj = (*thing_t(obj)).l_next;
    }
}