//! Process startup sequence, ported from `src/c/main.c`.

use std::io::Write;

use crate::command_dispatch::{do_command, CommandState};
use crate::config::GameConfig;
use crate::daemon::{fuse, start_daemon, Daemon};
use crate::entity::chase::roomin;
use crate::entity::player::MonsterFlags;
use crate::init::{init_colors, init_materials, init_names, init_player, init_probs, init_stones};
use crate::level::new_level;
use crate::machdep::{init_check, open_score, setup};
use crate::options::parse_opts;
use crate::rip::{death, death_monst, score};
use crate::rnd::{rnd, set_seed};
use crate::save::restore;
use crate::ui::input::{self, readchar, wait_for};
use crate::ui::output::{self, msg_str, status};
use crate::ui::runtime;
use glam::IVec2;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn install_exit_signal_handlers() {
    #[cfg(unix)]
    unsafe {
        let exit_handler = libc::exit as libc::sighandler_t;
        libc::signal(libc::SIGHUP, libc::SIG_DFL);
        for signal in [
            libc::SIGQUIT,
            libc::SIGILL,
            libc::SIGTRAP,
            libc::SIGABRT,
            libc::SIGFPE,
            libc::SIGBUS,
            libc::SIGSEGV,
            libc::SIGSYS,
            libc::SIGTERM,
            libc::SIGINT,
        ] {
            libc::signal(signal, exit_handler);
        }
    }
}

fn drop_privileges() {
    #[cfg(unix)]
    unsafe {
        let real_gid = libc::getgid();
        let real_uid = libc::getuid();

        #[cfg(any(target_os = "linux", target_os = "android"))]
        let group_error = libc::setresgid((-1i32) as libc::gid_t, real_gid, real_gid) != 0;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let group_error = libc::setregid(real_gid, real_gid) != 0;
        if group_error {
            eprintln!("Could not drop setgid privileges.  Aborting.");
            std::process::exit(1);
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        let user_error = libc::setresuid((-1i32) as libc::uid_t, real_uid, real_uid) != 0;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let user_error = libc::setreuid(real_uid, real_uid) != 0;
        if user_error {
            eprintln!("Could not drop setuid privileges.  Aborting.");
            std::process::exit(1);
        }
    }
}

use crate::game::globals::{
    after, count, dnum, get_dnum, get_purse, in_shell, inv_type, jump, master_mode_enabled, mpos,
    noscore, oldpos, oldrp, playing, purse, q_comm, running, see_floor, seed, terse, to_death,
    wizard,
};

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
    output::write_text_at(IVec2::new(0, GameConfig::SCREEN_LINES - 2), s);
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
    let old_cursor = output::window_cursor();
    runtime::shutdown();
    flush_stdout();
    #[cfg(unix)]
    libc::kill(0, libc::SIGTSTP);

    /*
     * start back up again
     */
    #[cfg(unix)]
    libc::signal(libc::SIGTSTP, tstp as libc::sighandler_t);
    input::enable_raw_mode();
    output::refresh_window();
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
    inv_type = INV_CLEAR;

    /*
     * parse environment declaration of options
     */
    let c_options = std::env::var("ROGUEOPTS").ok();
    if let Some(options) = c_options.as_ref() {
        parse_opts(options);
    }

    oldpos = crate::game::PLAYER.pos();
    let hero_pos = crate::game::PLAYER.pos();
    oldrp = roomin(hero_pos);
    start_daemon(Daemon::UiRender, 0, AFTER);
    Daemon::UiRender.run(0);
    let mut command_state = CommandState::default();
    while playing != false as u8 {
        do_command(&mut command_state); /* Command execution */
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
    let old_cursor = output::window_cursor();
    msg_str("really quit?");
    if readchar() == b'y' as i32 {
        libc::signal(libc::SIGINT, leave as libc::sighandler_t);
        output::clear_screen();
        let line = format!("You quit with {} gold pieces", get_purse());
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
    let _ = std::io::stdout().write_all(b"\n");
    in_shell = true as u8;
    after = false as u8;
    flush_stdout();
    /*
     * Fork and do a shell
     */
    #[cfg(unix)]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
        let _ = std::process::Command::new(shell).arg("-i").status();
    }

    print!("\n[Press return to continue]");
    let _ = std::io::stdout().flush();
    input::enable_raw_mode();
    in_shell = false as u8;
    wait_for('\n');
}

/// my_exit:
/// Leave the process properly.
///
/// No globals used directly.
pub unsafe fn my_exit(st: i32) -> ! {
    if !runtime::is_shutdown() {
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
    install_exit_signal_handlers();

    let mut argv: Vec<String> = args.to_vec();
    if master_mode_enabled != 0 && argv.len() >= 2 && argv[1].is_empty() {
        wizard = 1;
        crate::game::PLAYER.add_flag(MonsterFlags::SEEMONST);
        argv.remove(1);
    }
    let argc = argv.len() as i32;

    let mut home_dir = std::env::var("HOME").unwrap_or_default();
    if !home_dir.is_empty() && !home_dir.ends_with('/') {
        home_dir.push('/');
    }
    crate::game::globals::set_home(home_dir.clone());
    // Default save file: "<home>rogue.save".
    let save_name = format!("{}rogue.save", home_dir);
    crate::game::globals::set_file_name(save_name);

    let options = std::env::var("ROGUEOPTS").ok();
    if let Some(options) = options.as_ref() {
        parse_opts(options);
    }
    if options.is_none() || crate::game::globals::whoami().is_empty() {
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "nobody".to_owned());
        crate::game::globals::set_whoami(crate::options::filter_printable(&username));
    }

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i32)
        .unwrap_or(0);
    let clock_seed = now_secs + std::process::id() as i32;
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
    drop_privileges();

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
            crate::game::globals::whoami(),
            get_dnum()
        );
    } else {
        print!(
            "Hello {}, just a moment while I dig the dungeon...",
            crate::game::globals::whoami()
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

    crate::game::globals::init_inv_t_names();
    crate::game::globals::init_trap_names();
    init_probs();
    init_player();
    init_names();
    init_colors();
    init_stones();
    init_materials();
    setup();
    if master_mode_enabled != 0 {
        noscore = wizard;
    }
    new_level();
    start_daemon(Daemon::Runners, 0, AFTER);
    start_daemon(Daemon::Doctor, 0, AFTER);
    fuse(Daemon::Swander, 0, WANDERTIME, AFTER);
    start_daemon(Daemon::Stomach, 0, AFTER);
    start_daemon(Daemon::RingEffects, 0, AFTER);
    playit();
    0
}
