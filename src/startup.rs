//! Process startup sequence, ported from `src/c/main.c`.

use std::io::Write;

use crate::command::command;
use crate::config::GameConfig;
use crate::daemon::{fuse, start_daemon, Daemon};
use crate::entity::chase::roomin;
use crate::entity::player::{MonsterFlags, Thing, ThingMonster};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::init::{init_colors, init_materials, init_names, init_player, init_probs, init_stones};
use crate::level::new_level;
use crate::machdep::{getltchars, init_check, open_score, playltchars, resetltchars, setup};
use crate::mdport::{
    md_gethomedir, md_getpid, md_getusername, md_hasclreol, md_init, md_normaluser, md_shellescape,
    md_tstpresume, md_tstpsignal,
};
use crate::options::parse_opts;
use crate::rip::{death, death_monst, score};
use crate::rnd::{rnd, set_seed};
use crate::save::restore;
use crate::ui::input::{self, readchar, wait_for};
use crate::ui::output::{self, msg_str, status};
use crate::ui::runtime;
use crate::ui::Window;
use glam::IVec2;

const MAXSTR: usize = 1024;
const AFTER: i32 = 2;
const WANDERTIME: i32 = 70;
const INV_CLEAR: i32 = 2;
const SIGINT: i32 = 2;

/// Flushes the process stdout stream (replaces the C `fflush(stdout)` calls).
#[inline]
fn flush_stdout() {
    let _ = std::io::stdout().flush();
}

use crate::globals::{after, count, dnum, in_shell, inv_type, jump, master_mode_enabled, mpos, noscore, oldpos, oldrp, playing, purse, q_comm, running, see_floor, seed, terse, to_death, wizard};


#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn arg_at(argv: *mut *mut u8, index: usize) -> *mut u8 {
    *argv.add(index)
}

// ── Game control functions ported from src/c/main.c ─────────────────────────

/// endit:
/// Exit the program abnormally.
///
/// No globals used directly.
pub unsafe extern "C" fn endit(sig: i32) {
    let _ = sig;
    fatal("Okay, bye bye!\n");
}

/// fatal:
/// Exit the program, printing a message.
///
/// No globals used directly.
pub unsafe fn fatal(s: &str) {
    output::write_text_at(
        IVec2::new(0, GameConfig::SCREEN_LINES - 2),
        s,
    );
    output::refresh();
    runtime::shutdown();
    my_exit(0);
}

/// roll:
/// Roll a number of dice.
///
/// No globals used directly (uses rnd()).
pub unsafe fn roll(mut number: i32, sides: i32) -> i32 {
    let mut dtotal = 0;

    while number > 0 {
        dtotal += rnd(sides) + 1;
        number -= 1;
    }
    dtotal
}

/// tstp:
/// Handle stop and start signals.
pub unsafe extern "C" fn tstp(ignored: i32) {
    let _ = ignored;

    /*
     * leave nicely
     */
    let old_cursor = output::window_cursor(Window::Curscr);
    runtime::move_physical_cursor(
        IVec2::new(GameConfig::SCREEN_COLS - 1, 0),
        IVec2::new(0, GameConfig::SCREEN_LINES - 1),
    );
    runtime::shutdown();
    resetltchars();
    flush_stdout();
    md_tstpsignal();

    /*
     * start back up again
     */
    md_tstpresume();
    input::set_raw_mode(true);
    input::set_echo(false);
    input::set_keypad(Window::Stdscr, true);
    playltchars();
    output::set_clear_on_refresh(Window::Curscr, true);
    output::refresh_window(Window::Curscr);
    runtime::move_physical_cursor(output::window_cursor(Window::Curscr), old_cursor);
    output::move_cursor(old_cursor);
    flush_stdout();
}

/// playit:
/// The main loop of the program.  Loop until the game is over,
/// refreshing things and looking at the proper times.
///
/// Uses globals: terse, jump, see_floor, inv_type, oldpos, oldrp,
/// hero, playing, running.
pub unsafe fn playit() {
    /*
     * set up defaults for slow terminals
     */
    if runtime::baud_rate() <= 1200 {
        terse = true as u8;
        jump = true as u8;
        see_floor = false as u8;
    }

    if md_hasclreol() != 0 {
        inv_type = INV_CLEAR;
    }

    /*
     * parse environment declaration of options
     */
    let c_options = std::env::var("ROGUEOPTS").ok();
    if let Some(options) = c_options.as_ref() {
        parse_opts(options);
    }

    oldpos = crate::game::PLAYER.pos();
    let mut hero_pos = crate::game::PLAYER.pos();
    oldrp = roomin(&raw mut hero_pos);
    while playing != false as u8 {
        command(); /* Command execution */
    }
    endit(0);
}

/// quit:
/// Have player make certain, then exit.
///
/// Uses globals: q_comm, mpos, purse, count, to_death.
pub unsafe extern "C" fn quit(sig: i32) {
    let _ = sig;

    /*
     * Reset the signal in case we got here via an interrupt
     */
    if q_comm == false as u8 {
        mpos = 0;
    }
    let old_cursor = output::window_cursor(Window::Curscr);
    msg_str("really quit?");
    if readchar() == b'y' as i32 {
        libc::signal(libc::SIGINT, leave as libc::sighandler_t);
        output::clear_screen();
        let line = format!(
            "You quit with {} gold pieces",
            std::ptr::addr_of!(purse).read()
        );
        output::write_text_at(IVec2::new(0, GameConfig::SCREEN_LINES - 2), &line);
        output::move_cursor(IVec2::new(0, GameConfig::SCREEN_LINES - 1));
        output::refresh();
        score(purse, 1, 0);
        my_exit(0);
    } else {
        output::move_cursor(IVec2::new(0, 0));
        output::clear_to_end_of_line();
        status();
        output::move_cursor(old_cursor);
        output::refresh();
        mpos = 0;
        count = 0;
        to_death = false as u8;
    }
}

/// leave:
/// Leave quickly, but curteously.
pub unsafe extern "C" fn leave(sig: i32) {
    let _ = sig;

    if !runtime::is_shutdown() {
        runtime::move_physical_cursor(
            IVec2::new(GameConfig::SCREEN_COLS - 1, 0),
            IVec2::new(0, GameConfig::SCREEN_LINES - 1),
        );
        runtime::shutdown();
    }

    let _ = std::io::stdout().write_all(b"\n");
    my_exit(0);
}

/// shell:
/// Let them escape for a while.
///
/// Uses globals: in_shell, after.
pub unsafe fn shell() {
    /*
     * Set the terminal back to original mode
     */
    output::move_cursor(IVec2::new(0, GameConfig::SCREEN_LINES - 1));
    output::refresh();
    runtime::shutdown();
    resetltchars();
    let _ = std::io::stdout().write_all(b"\n");
    in_shell = true as u8;
    after = false as u8;
    flush_stdout();
    /*
     * Fork and do a shell
     */
    md_shellescape();

    print!("\n[Press return to continue]");
    let _ = std::io::stdout().flush();
    input::set_echo(false);
    input::set_raw_mode(true);
    input::set_keypad(Window::Stdscr, true);
    playltchars();
    in_shell = false as u8;
    wait_for('\n');
    output::set_clear_on_refresh(Window::Stdscr, true);
}

/// my_exit:
/// Leave the process properly.
///
/// No globals used directly.
pub unsafe fn my_exit(st: i32) -> ! {
    resetltchars();
    if !runtime::is_shutdown() {
        input::set_echo(true);
        runtime::shutdown();
    }
    flush_stdout();
    let _ = std::io::stderr().flush();
    std::process::exit(st);
}

/// The game entry point. `args` mirrors the process `argv` (including the
/// program name at index 0); `src/bin/rogue.rs` calls this with
/// `std::env::args()`.
pub unsafe fn rogue_main(args: &[String]) -> i32 {
    md_init();

    let mut argv: Vec<String> = args.to_vec();
    if master_mode_enabled != 0 && argv.len() >= 2 && argv[1].is_empty() {
        wizard = 1;
        crate::game::PLAYER.add_flag(MonsterFlags::SEEMONST);
        argv.remove(1);
    }
    let argc = argv.len() as i32;

    let home_dir = md_gethomedir();
    crate::globals::set_home(home_dir.clone());
    // Default save file: "<home>rogue.save".
    let save_name = format!("{}rogue.save", home_dir);
    crate::globals::set_file_name(save_name);

    let options = std::env::var("ROGUEOPTS").ok();
    if let Some(options) = options.as_ref() {
        parse_opts(options);
    }
    if options.is_none() || crate::globals::whoami().is_empty() {
        let username = md_getusername();
        crate::globals::set_whoami(crate::options::filter_printable(&username));
    }

    let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i32).unwrap_or(0);
    let clock_seed = now_secs + md_getpid();
    dnum = if master_mode_enabled != 0 && wizard != 0 {
        std::env::var("SEED")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(clock_seed)
    } else {
        clock_seed
    };
    seed = dnum;
    set_seed(seed);
    open_score();
    md_normaluser();

    if argc == 2 {
        let argument = argv[1].as_str();
        if argument == "-s" {
            noscore = 1;
            score(0, -1, 0);
            return 0;
        }
        if argument == "-d" {
            dnum = rnd(100);
            while dnum > 1 {
                dnum -= 1;
                rnd(100);
            }
            purse = rnd(100) + 1;
            crate::game::set_current_depth(rnd(100) + 1);
            runtime::initialize();
            getltchars();
            death(death_monst());
            return 0;
        }
    }

    init_check();
    if argc == 2 && restore(&argv[1]) == 0 {
        my_exit(1);
    }

    if master_mode_enabled != 0 && wizard != 0 {
        print!(
            "Hello {}, welcome to dungeon #{}",
            crate::globals::whoami(),
            std::ptr::addr_of!(dnum).read()
        );
    } else {
        print!(
            "Hello {}, just a moment while I dig the dungeon...",
            crate::globals::whoami()
        );
    }
    std::io::stdout()
        .flush()
        .expect("failed to flush startup message");
    runtime::initialize();
    // Reject terminals smaller than the fixed game grid. The physical size is
    // unavailable on some backends; in that case keep the legacy permissive
    // behaviour and continue.
    if let Some(size) = crate::ui::physical_size() {
        if size.y < GameConfig::SCREEN_LINES || size.x < GameConfig::SCREEN_COLS {
            runtime::shutdown();
            eprintln!(
                "Sorry, the screen must be at least {}x{}",
                GameConfig::SCREEN_LINES,
                GameConfig::SCREEN_COLS
            );
            eprintln!("Current terminal size: {}x{}", size.x, size.y);
            my_exit(1);
        }
    }

    crate::globals::init_inv_t_names();
    crate::globals::init_trap_names();
    init_probs();
    init_player();
    init_names();
    init_colors();
    init_stones();
    init_materials();
    setup();
    output::set_line_optimization(Window::Stdscr, true);
    if master_mode_enabled != 0 {
        noscore = wizard;
    }
    new_level();
    start_daemon(Daemon::Runners, 0, AFTER);
    start_daemon(Daemon::Doctor, 0, AFTER);
    fuse(Daemon::Swander, 0, WANDERTIME, AFTER);
    start_daemon(Daemon::Stomach, 0, AFTER);
    playit();
    0
}
