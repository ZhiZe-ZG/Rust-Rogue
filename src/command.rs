//! Player command reading and dispatch.
//!
//! Ported from `src/c/command.c` to Rust.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use crate::config::GameConfig;
use crate::daemon::{do_daemons, do_fuses};
use crate::draw::{add_pass, look};
use crate::entity::chase::{diag_ok, see_monst};
use crate::entity::player::{
    do_move, do_run, Thing, ThingMonster, ThingObject, MonsterFlags, ObjectFlags,
};
use crate::game::PLAYER;
use crate::globals::{pot_info, ring_info, scr_info, ws_info, CObjInfo};
use crate::help::{help, identify};
use crate::item::armor::{take_off, wear};
use crate::item::pack::{add_pack, get_item, inventory, pick_up, picky_inven};
use crate::item::potions::{quaff, raise_level, turn_see};
use crate::item::rings::{ring_off, ring_on, RingType};
use crate::item::scrolls::read_scroll;
use crate::item::sticks::do_zap;
use crate::item::thing_list::new_item;
use crate::item::things::{discovered, drop, inv_name};
use crate::item::weapons::{init_weapon, missile, wield};
use crate::level::new_level;
use crate::misc::{eat, get_dir};
use crate::options::{get_str, option};
use crate::rip::total_winner;
use crate::rnd::rnd;
use crate::save::save_game;
use crate::startup::{quit, shell};
use crate::ui::input::readchar;
use crate::ui::output::{self, addmsg_str, endmsg, msg_str, status};
use crate::ui::Window;
use crate::wizard::{create_obj, show_map, teleport, whatis};
use glam::IVec2;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

// ─── Constants ────────────────────────────────────────────────────────────────

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

// Map flags
const F_REAL: c_char = 0x10u8 as c_char;
const F_SEEN: c_char = 0x40u8 as c_char;
const F_TMASK: c_char = 0x07u8 as c_char;

// Escape
const ESCAPE: c_int = 27;

// get_str() return codes
const NORM: c_int = 0;

// Weapon/armor kinds for the wizard ('^I' = CTRL-I) cheat
const TWOSWORD: c_int = 5;
const PLATE_MAIL: c_int = 7;

// Delayed-action phases
const BEFORE: c_int = 1;
const AFTER: c_int = 2;

/// CTRL(c) macro from rogue.h: `c & 037`.
///
/// Precomputed constants are used in `match` patterns (Rust does not allow
/// function calls in patterns, even for `const fn`s).
const CTRL_A: u8 = b'A' & 0x1f; // 0x01
const CTRL_B: u8 = b'B' & 0x1f; // 0x02
const CTRL_C: u8 = b'C' & 0x1f; // 0x03
const CTRL_D: u8 = b'D' & 0x1f; // 0x04
const CTRL_E: u8 = b'E' & 0x1f; // 0x05
const CTRL_F: u8 = b'F' & 0x1f; // 0x06
const CTRL_G: u8 = b'G' & 0x1f; // 0x07
const CTRL_H: u8 = b'H' & 0x1f; // 0x08
const CTRL_I: u8 = b'I' & 0x1f; // 0x09
const CTRL_J: u8 = b'J' & 0x1f; // 0x0a
const CTRL_K: u8 = b'K' & 0x1f; // 0x0b
const CTRL_L: u8 = b'L' & 0x1f; // 0x0c
const CTRL_N: u8 = b'N' & 0x1f; // 0x0e
const CTRL_P: u8 = b'P' & 0x1f; // 0x10
const CTRL_R: u8 = b'R' & 0x1f; // 0x12
const CTRL_T: u8 = b'T' & 0x1f; // 0x14
const CTRL_U: u8 = b'U' & 0x1f; // 0x15
const CTRL_W: u8 = b'W' & 0x1f; // 0x17
const CTRL_X: u8 = b'X' & 0x1f; // 0x18
const CTRL_Y: u8 = b'Y' & 0x1f; // 0x19
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

// ─── Static locals for command() ─────────────────────────────────────────────

static mut COUNTCH: c_char = 0;
static mut DIRECTION: c_char = 0;
static mut NEWCOUNT: c_uchar = false as c_uchar;

// ─── Extern C globals ─────────────────────────────────────────────────────────

unsafe extern "C" {
    static mut after: c_uchar;
    static mut again: c_uchar;
    static mut amulet: c_uchar;
    static mut count: c_int;
    static mut delta: IVec2;
    static mut dir_ch: c_char;
    static mut dnum: c_int;
    static mut door_stop: c_uchar;
    static mut firstmove: c_uchar;
    static mut food_left: c_int;
    static mut has_hit: c_uchar;
    static mut huh: [c_char; MAXSTR];
    static mut inpack: c_int;
    static mut inv_describe: c_uchar;
    static mut jump: c_uchar;
    static mut kamikaze: c_uchar;
    static mut l_last_comm: c_char;
    static mut l_last_dir: c_char;
    static mut l_last_pick: *mut Thing;
    static mut last_comm: c_char;
    static mut last_dir: c_char;
    static mut last_pick: *mut Thing;
    static mut lastscore: c_int;
    static mut max_hit: c_int;
    static mut move_on: c_uchar;
    static mut mpos: c_int;
    static mut no_command: c_int;
    static mut noscore: c_int;
    static mut prbuf: [c_char; 2 * MAXSTR];
    static mut purse: c_int;
    static mut q_comm: c_uchar;
    static mut release: *mut c_char;
    static mut runch: c_char;
    static mut running: c_uchar;
    static mut save_msg: c_uchar;
    static mut seenstairs: c_uchar;
    static mut stat_msg: c_uchar;
    static mut take: c_char;
    static mut terse: c_uchar;
    static mut to_death: c_uchar;
    static mut tr_name: [*mut c_char; GameConfig::TRAP_KIND_COUNT as usize];
    static mut r_stones: [*mut c_char; 14];
    static mut p_colors: [*mut c_char; 14];
    static mut s_names: [*mut c_char; 18];
    static mut ws_made: [*mut c_char; 14];
    static mut wizard: c_int;
}

// ─── Extern C functions called from this module ───────────────────────────────

unsafe extern "C" {
    fn free(ptr: *mut c_void);
    fn malloc(size: usize) -> *mut c_void;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strlen(s: *const c_char) -> usize;
}

// ─── Module-local helpers ─────────────────────────────────────────────────────

#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

#[inline]
fn hero_pos() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    crate::game::PLAYER.has_flag(flag)
}

#[inline]
unsafe fn moat_at(y: c_int, x: c_int) -> *mut Thing {
    crate::game::monster_at(y, x)
}

#[inline]
unsafe fn isring(ring: *mut Thing, ring_type: RingType) -> bool {
    !ring.is_null() && RingType::from_raw((*thing_o(ring)).o_which) == Some(ring_type)
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
/// tr_name, stat_msg, inpack, food_left, equipment, inv_describe.
#[no_mangle]
pub unsafe extern "C" fn command() {
    let mut ch: u8;
    let mut ntimes: c_int = 1; // Number of player moves
    let mut mp: *mut Thing;

    if player_has(MonsterFlags::HASTE) {
        ntimes += 1;
    }

    /*
     * Let the daemons start up
     */
    do_daemons(BEFORE);
    do_fuses(BEFORE);

    while ntimes > 0 {
        ntimes -= 1;
        again = false as c_uchar;
        if has_hit != 0 {
            look(false as c_uchar);
            endmsg();
            has_hit = false as c_uchar;
        }

        /*
         * these are illegal things for the player to be, so if any are
         * set, someone's been poking in memory
         */
        if player_has(
            MonsterFlags::SLOW
                | MonsterFlags::GREED
                | MonsterFlags::INVIS
                | MonsterFlags::REGEN
                | MonsterFlags::TARGET,
        ) {
            std::process::exit(1);
        }

        look(true as c_uchar);
        if running == 0 {
            door_stop = false as c_uchar;
        }
        status();
        lastscore = purse;
        let hero = hero_pos();
        output::move_cursor(IVec2::new(hero.x, hero.y));
        if !((running != 0 || count != 0) && jump != 0) {
            output::refresh(); // Draw screen
        }
        take = 0;
        after = true as c_uchar;

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
                move_on = false as c_uchar;
                if mpos != 0 {
                    // Erase message if it's there
                    msg_str("");
                }
            }
        } else {
            ch = b'.';
        }

        if no_command != 0 {
            no_command -= 1;
            if no_command == 0 {
                crate::game::PLAYER.add_flag(MonsterFlags::RUN);
                msg_str("you can move again");
            }
        } else {
            /*
             * check for prefixes
             */
            NEWCOUNT = false as c_uchar;
            if ch.is_ascii_digit() {
                count = 0;
                NEWCOUNT = true as c_uchar;
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
                        let mut obj = crate::game::with_current_level(|level| level.items.head());
                        let mut found = false;
                        while !obj.is_null() {
                            if (*thing_o(obj)).o_pos.y == hero.y
                                && (*thing_o(obj)).o_pos.x == hero.x
                            {
                                found = true;
                                break;
                            }
                            obj = crate::entity::player::thing_next(obj);
                        }

                        if found {
                            if levit_check() == 0 {
                                pick_up((*thing_o(obj)).o_type as c_char);
                            }
                        } else {
                            if terse == 0 {
                                addmsg_str("there is ");
                            }
                            addmsg_str("nothing here");
                            if terse == 0 {
                                addmsg_str(" to pick up");
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
                    v if v == ctrl(b'H')
                        || v == ctrl(b'J')
                        || v == ctrl(b'K')
                        || v == ctrl(b'L')
                        || v == ctrl(b'Y')
                        || v == ctrl(b'U')
                        || v == ctrl(b'B')
                        || v == ctrl(b'N') =>
                    {
                        if !player_has(MonsterFlags::BLIND) {
                            door_stop = true as c_uchar;
                            firstmove = true as c_uchar;
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
                            kamikaze = true as c_uchar;
                        }
                        if get_dir() == 0 {
                            after = false as c_uchar;
                        } else {
                            let hero = hero_pos();
                            delta.y += hero.y;
                            delta.x += hero.x;
                            mp = moat_at(delta.y, delta.x);
                            if mp.is_null() || (see_monst(mp) == 0 && !player_has(MonsterFlags::SEEMONST))
                            {
                                if terse == 0 {
                                    addmsg_str("I see ");
                                }
                                msg_str("no monster there");
                                after = false as c_uchar;
                            } else if {
                                let mut hero_copy = hero_pos();
                                diag_ok(&raw mut hero_copy, &raw mut delta) != 0
                            } {
                                to_death = true as c_uchar;
                                max_hit = 0;
                                (*thing_t(mp)).t_flags.insert(MonsterFlags::TARGET);
                                runch = dir_ch;
                                ch = dir_ch as u8;
                                continue 'dispatch;
                            }
                        }
                    }
                    b't' => {
                        if get_dir() == 0 {
                            after = false as c_uchar;
                        } else {
                            missile(delta.y, delta.x);
                        }
                    }
                    b'a' => {
                        if last_comm == 0 {
                            msg_str("you haven't typed a command yet");
                            after = false as c_uchar;
                        } else {
                            ch = last_comm as u8;
                            again = true as c_uchar;
                            continue 'dispatch;
                        }
                    }
                    b'q' => quaff(),
                    b'Q' => {
                        after = false as c_uchar;
                        q_comm = true as c_uchar;
                        quit(0);
                        q_comm = false as c_uchar;
                    }
                    b'i' => {
                        after = false as c_uchar;
                        inventory(crate::game::PLAYER.pack(), 0);
                    }
                    b'I' => {
                        after = false as c_uchar;
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
                        after = false as c_uchar;
                    }
                    b'c' => {
                        call();
                        after = false as c_uchar;
                    }
                    b'>' => {
                        after = false as c_uchar;
                        d_level();
                    }
                    b'<' => {
                        after = false as c_uchar;
                        u_level();
                    }
                    b'?' => {
                        after = false as c_uchar;
                        help();
                    }
                    b'/' => {
                        after = false as c_uchar;
                        identify();
                    }
                    b's' => search(),
                    b'z' => {
                        if get_dir() != 0 {
                            do_zap();
                        } else {
                            after = false as c_uchar;
                        }
                    }
                    b'D' => {
                        after = false as c_uchar;
                        discovered();
                    }
                    CTRL_P => {
                        after = false as c_uchar;
                        msg_str(&CStr::from_ptr(huh.as_ptr()).to_string_lossy());
                    }
                    CTRL_R => {
                        after = false as c_uchar;
                        output::set_clear_on_refresh(Window::Curscr, true);
                        output::refresh_window(Window::Curscr);
                    }
                    b'v' => {
                        after = false as c_uchar;
                        msg_str(&format!(
                            "version {}. (mctesq was here)",
                            CStr::from_ptr(release).to_string_lossy()
                        ));
                    }
                    b'S' => {
                        after = false as c_uchar;
                        save_game();
                    }
                    b'.' => {
                        // Rest command
                    }
                    b' ' => {
                        after = false as c_uchar; // "Legal" illegal command
                    }
                    b'^' => {
                        after = false as c_uchar;
                        if get_dir() != 0 {
                            let hero = hero_pos();
                            delta.y += hero.y;
                            delta.x += hero.x;
                            if terse == 0 {
                                addmsg_str("You have found ");
                            }
                            if !crate::draw::is_trap_cell(delta.y, delta.x) {
                                msg_str("no trap there");
                            } else if player_has(MonsterFlags::HALU) {
                                msg_str(
                                    &CStr::from_ptr(
                                        tr_name[rnd(GameConfig::TRAP_KIND_COUNT) as usize],
                                    )
                                    .to_string_lossy(),
                                );
                            } else {
                                msg_str(
                                    &CStr::from_ptr(
                                        tr_name
                                            [crate::draw::trap_kind_at(delta.y, delta.x) as usize],
                                    )
                                    .to_string_lossy(),
                                );
                                crate::draw::set_seen_at(delta.y, delta.x);
                            }
                        }
                    }
                    b'+' => {
                        // Wizard toggle (was the `when '+'` arm under `#ifdef MASTER`)
                        after = false as c_uchar;
                        if MASTER {
                            if wizard != 0 {
                                wizard = 0;
                                turn_see(true as c_uchar);
                                msg_str("not wizard any more");
                            } else {
                                wizard = 1;
                                noscore = 1;
                                turn_see(false as c_uchar);
                                msg_str(&format!(
                                    "you are suddenly as smart as Ken Arnold in dungeon #{}",
                                    dnum
                                ));
                            }
                        }
                    }
                    v if v == ESCAPE as u8 => {
                        door_stop = false as c_uchar;
                        count = 0;
                        after = false as c_uchar;
                        again = false as c_uchar;
                    }
                    b'm' => {
                        move_on = true as c_uchar;
                        if get_dir() == 0 {
                            after = false as c_uchar;
                        } else {
                            ch = dir_ch as u8;
                            COUNTCH = dir_ch;
                            continue 'dispatch;
                        }
                    }
                    b')' => {
                        current(
                            PLAYER.weapon(),
                            c"wielding".as_ptr(),
                            std::ptr::null_mut(),
                        );
                    }
                    b']' => {
                        current(PLAYER.armor(), c"wearing".as_ptr(), std::ptr::null_mut());
                    }
                    b'=' => {
                        current(
                            PLAYER.left_ring(),
                            c"wearing".as_ptr(),
                            if terse != 0 {
                                c"(L)".as_ptr()
                            } else {
                                c"on left hand".as_ptr()
                            },
                        );
                        current(
                            PLAYER.right_ring(),
                            c"wearing".as_ptr(),
                            if terse != 0 {
                                c"(R)".as_ptr()
                            } else {
                                c"on right hand".as_ptr()
                            },
                        );
                    }
                    b'@' => {
                        stat_msg = true as c_uchar;
                        status();
                        stat_msg = false as c_uchar;
                        after = false as c_uchar;
                    }
                    _ => {
                        after = false as c_uchar;
                        if MASTER && wizard != 0 {
                            match ch {
                                b'|' => {
                                    let hero = hero_pos();
                                    msg_str(&format!("@ {},{}", hero.y, hero.x));
                                }
                                b'C' => create_obj(),
                                b'$' => {
                                    msg_str(&format!("inpack = {}", inpack));
                                }
                                CTRL_G => {
                                    let _ = inventory(crate::game::with_current_level(|level| level.items.head()), 0);
                                }
                                CTRL_W => whatis(false as c_uchar, 0),
                                CTRL_D => {
                                    crate::game::set_current_depth(
                                        crate::game::current_depth() + 1,
                                    );
                                    new_level();
                                }
                                CTRL_A => {
                                    crate::game::set_current_depth(
                                        crate::game::current_depth() - 1,
                                    );
                                    new_level();
                                }
                                CTRL_F => show_map(),
                                CTRL_T => teleport(),
                                CTRL_E => {
                                    msg_str(&format!("food left: {}", food_left));
                                }
                                CTRL_C => add_pass(),
                                CTRL_X => {
                                    turn_see(if player_has(MonsterFlags::SEEMONST) {
                                        true as c_uchar
                                    } else {
                                        false as c_uchar
                                    });
                                }
                                CTRL_TILDE => {
                                    let item = get_item(c"charge".as_ptr(), STICK as c_int);
                                    if !item.is_null() {
                                        (*thing_o(item)).o_arm = 10000;
                                    }
                                }
                                CTRL_I => {
                                    let mut obj: *mut Thing;

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
                                    add_pack(obj, true as c_uchar);
                                    PLAYER.set_weapon(obj);
                                    /*
                                     * And his suit of armor
                                     */
                                    obj = new_item();
                                    (*thing_o(obj)).o_type = ARMOR as c_int;
                                    (*thing_o(obj)).o_which = PLATE_MAIL;
                                    (*thing_o(obj)).o_arm = -5;
                                    (*thing_o(obj)).o_flags.insert(ObjectFlags::KNOW);
                                    (*thing_o(obj)).o_count = 1;
                                    (*thing_o(obj)).o_group = 0;
                                    PLAYER.set_armor(obj);
                                    add_pack(obj, true as c_uchar);
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
            door_stop = false as c_uchar;
        }
        if after == 0 {
            ntimes += 1;
        }
    }

    do_daemons(AFTER);
    do_fuses(AFTER);
    if isring(PLAYER.left_ring(), RingType::Searching) {
        search();
    } else if isring(PLAYER.left_ring(), RingType::Teleport) && rnd(50) == 0 {
        teleport();
    }
    if isring(PLAYER.right_ring(), RingType::Searching) {
        search();
    } else if isring(PLAYER.right_ring(), RingType::Teleport) && rnd(50) == 0 {
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
    save_msg = false as c_uchar;
    count = 0;
    msg_str(&format!(
        "illegal command '{}'",
        output::format_key(ch as u8)
    ));
    save_msg = true as c_uchar;
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

    if player_has(MonsterFlags::HALU) {
        probinc += 3;
    }
    if player_has(MonsterFlags::BLIND) {
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
                match crate::game::tile_at(y, x) {
                    crate::tile::Tile::Wall | crate::tile::Tile::HiddenDoor => {
                        if rnd(5 + probinc) == 0 {
                            crate::draw::reveal_secret_at(y, x);
                            msg_str("a secret door");
                            found = true;
                            count = false as c_uchar as c_int;
                            running = false as c_uchar;
                        }
                    }
                    crate::tile::Tile::Trap(_) => {
                        if rnd(2 + probinc) == 0 {
                            crate::level::with_current_level_mut(|current| {
                                current.reveal_trap(y as usize, x as usize);
                            });
                            if terse == 0 {
                                addmsg_str("you found ");
                            }
                            if player_has(MonsterFlags::HALU) {
                                msg_str(
                                    &CStr::from_ptr(
                                        tr_name[rnd(GameConfig::TRAP_KIND_COUNT) as usize],
                                    )
                                    .to_string_lossy(),
                                );
                            } else {
                                msg_str(
                                    &CStr::from_ptr(
                                        tr_name[crate::draw::trap_kind_at(y, x) as usize],
                                    )
                                    .to_string_lossy(),
                                );
                            }
                            found = true;
                            count = false as c_uchar as c_int;
                            running = false as c_uchar;
                        }
                    }
                    crate::tile::Tile::Empty => {
                        if rnd(3 + probinc) == 0 {
                            crate::draw::reveal_secret_at(y, x);
                            found = true;
                            count = false as c_uchar as c_int;
                            running = false as c_uchar;
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
        look(false as c_uchar);
    }
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
    if crate::game::tile_at(hero.y, hero.x) != crate::tile::Tile::Stairs {
        msg_str("I see no way down");
    } else {
        crate::game::set_current_depth(crate::game::current_depth() + 1);
        seenstairs = false as c_uchar;
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
    if crate::game::tile_at(hero.y, hero.x) == crate::tile::Tile::Stairs {
        if amulet != 0 {
            crate::game::set_current_depth(crate::game::current_depth() - 1);
            if crate::game::current_depth() == 0 {
                total_winner();
            }
            new_level();
            msg_str("you feel a wrenching sensation in your gut");
        } else {
            msg_str("your way is magically blocked");
        }
    } else {
        msg_str("I see no way up");
    }
}

/// levit_check:
/// Check to see if she's levitating, and if she is, print an
/// appropriate message.
///
/// Uses globals: player.
#[no_mangle]
pub unsafe extern "C" fn levit_check() -> c_uchar {
    if !player_has(MonsterFlags::LEVIT) {
        return false as c_uchar;
    }
    msg_str("You can't.  You're floating off the ground!");
    true as c_uchar
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

    let otype = (*thing_o(obj)).o_type as u8;

    if otype == FOOD as u8 {
        msg_str("you can't call that anything");
        return;
    }

    // Weapons and armor store their player-assigned name directly in `o_label`.
    if otype != RING as u8 && otype != POTION as u8 && otype != SCROLL as u8 && otype != STICK as u8
    {
        if let Some(elsewise) = (*thing_o(obj)).o_label.as_ref() {
            if terse == 0 {
                addmsg_str("Was ");
            }
            msg_str(&format!("called \"{}\"", elsewise));
        }

        if terse != 0 {
            msg_str("call it: ");
        } else {
            msg_str("what do you want to call it? ");
        }

        match (*thing_o(obj)).o_label.as_ref() {
            Some(elsewise) => {
                strcpy(prbuf.as_mut_ptr(), elsewise.as_ptr().cast::<c_char>());
            }
            None => {
                prbuf[0] = 0;
            }
        }
        if get_str(prbuf.as_mut_ptr().cast(), Window::Stdscr) == NORM {
            let text = CStr::from_ptr(prbuf.as_ptr()).to_string_lossy().into_owned();
            (*thing_o(obj)).o_label = Some(text);
        }
        return;
    }

    // Magic items keep their call-name in the obj-info table entry.
    let op = match otype {
        x if x == RING as u8 => ring_info.as_mut_ptr().add((*thing_o(obj)).o_which as usize),
        x if x == POTION as u8 => pot_info.as_mut_ptr().add((*thing_o(obj)).o_which as usize),
        x if x == SCROLL as u8 => scr_info.as_mut_ptr().add((*thing_o(obj)).o_which as usize),
        _ => ws_info.as_mut_ptr().add((*thing_o(obj)).o_which as usize),
    };

    let mut elsewise: *mut c_char = match otype {
        x if x == RING as u8 => r_stones[(*thing_o(obj)).o_which as usize],
        x if x == POTION as u8 => p_colors[(*thing_o(obj)).o_which as usize],
        x if x == SCROLL as u8 => s_names[(*thing_o(obj)).o_which as usize],
        _ => ws_made[(*thing_o(obj)).o_which as usize],
    };

    if let Some(guess) = &(*op).oi_guess {
        elsewise = guess.as_ptr().cast::<c_char>().cast_mut();
    }

    if (*op).oi_know {
        msg_str("that has already been identified");
        return;
    }

    if (*op).oi_guess.is_some() {
        if terse == 0 {
            addmsg_str("Was ");
        }
        msg_str(&format!(
            "called \"{}\"",
            CStr::from_ptr(elsewise).to_string_lossy()
        ));
    }

    if terse != 0 {
        msg_str("call it: ");
    } else {
        msg_str("what do you want to call it? ");
    }

    if elsewise.is_null() {
        prbuf[0] = 0;
    } else {
        strcpy(prbuf.as_mut_ptr(), elsewise);
    }
    if get_str(prbuf.as_mut_ptr().cast(), Window::Stdscr) == NORM {
        let text = CStr::from_ptr(prbuf.as_ptr()).to_string_lossy().into_owned();
        (*op).oi_guess = Some(text);
    }
}

// ─── current() ────────────────────────────────────────────────────────────────

/// current:
/// Print the current weapon/armor.
///
/// Uses globals: after, terse, inv_describe.
#[no_mangle]
pub unsafe extern "C" fn current(cur: *mut Thing, how: *const c_char, where_: *const c_char) {
    after = false as c_uchar;
    if !cur.is_null() {
        if terse == 0 {
            addmsg_str(&format!(
                "you are {} (",
                CStr::from_ptr(how).to_string_lossy()
            ));
        }
        inv_describe = false as c_uchar;
        addmsg_str(&format!(
            "{}) {}",
            (*thing_o(cur)).o_packch as char,
            CStr::from_ptr(inv_name(cur, true as c_uchar)).to_string_lossy()
        ));
        inv_describe = true as c_uchar;
        if !where_.is_null() {
            addmsg_str(&format!(" {}", CStr::from_ptr(where_).to_string_lossy()));
        }
        endmsg();
    } else {
        if terse == 0 {
            addmsg_str("you are ");
        }
        addmsg_str(&format!(
            "{} nothing",
            CStr::from_ptr(how).to_string_lossy()
        ));
        if !where_.is_null() {
            addmsg_str(&format!(" {}", CStr::from_ptr(where_).to_string_lossy()));
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
    let mut obj = crate::game::with_current_level(|level| level.items.head());
    while !obj.is_null() {
        msg_str(&format!(
            "{}) {}",
            (*thing_o(obj)).o_type as u8 as char,
            CStr::from_ptr(inv_name(obj, false as c_uchar)).to_string_lossy()
        ));
        obj = crate::entity::player::thing_next(obj);
    }
}
