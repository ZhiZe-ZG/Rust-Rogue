//! Process startup sequence, ported from `src/c/main.c`.

use std::ffi::{CStr, CString};
use std::io::Write;
use std::os::raw::{c_char, c_int, c_uchar};

use crate::command::command;
use crate::config::GameConfig;
use crate::daemon::{fuse, start_daemon, Daemon};
use crate::entity::chase::roomin;
use crate::entity::player::{MonsterFlags, Thing, ThingMonster};
use crate::ffi::{exit, fflush, printf, putchar, setbuf, signal, time, CFile};
use crate::init::{init_colors, init_materials, init_names, init_player, init_probs, init_stones};
use crate::level::new_level;
use crate::machdep::{getltchars, init_check, open_score, playltchars, resetltchars, setup};
use crate::mdport::{
    md_gethomedir, md_getpid, md_getusername, md_hasclreol, md_init, md_normaluser, md_shellescape,
    md_tstpresume, md_tstpsignal,
};
use crate::options::{parse_opts, strucpy};
use crate::rip::{death, death_monst, score};
use crate::rnd::{rnd, set_seed};
use crate::save::restore;
use crate::ui::input::{self, readchar, wait_for};
use crate::ui::output::{self, msg_str, status};
use crate::ui::runtime;
use crate::ui::Window;
use glam::IVec2;

const MAXSTR: usize = 1024;
const AFTER: c_int = 2;
const WANDERTIME: c_int = 70;
const INV_CLEAR: c_int = 2;
const BUFSIZ: usize = 8192;
const SIGINT: c_int = 2;

/// Static buffer used by `leave()` to discard pending stdout output.
static mut LEAVE_BUF: [c_char; BUFSIZ] = [0; BUFSIZ];

#[cfg(target_os = "macos")]
unsafe extern "C" {
    static mut __stdoutp: *mut CFile;
    static mut __stderrp: *mut CFile;
}

#[cfg(not(target_os = "macos"))]
unsafe extern "C" {
    static mut stdout: *mut CFile;
    static mut stderr: *mut CFile;
}

#[inline]
unsafe fn c_stdout() -> *mut CFile {
    #[cfg(target_os = "macos")]
    {
        __stdoutp
    }
    #[cfg(not(target_os = "macos"))]
    {
        stdout
    }
}

#[inline]
unsafe fn c_stderr() -> *mut CFile {
    #[cfg(target_os = "macos")]
    {
        __stderrp
    }
    #[cfg(not(target_os = "macos"))]
    {
        stderr
    }
}

unsafe extern "C" {
    static mut dnum: c_int;
    static mut file_name: [c_char; MAXSTR];
    static mut home: [c_char; MAXSTR];
    static master_mode_enabled: c_uchar;
    static mut noscore: c_int;
    static mut purse: c_int;
    static mut seed: c_int;
    static mut whoami: [c_char; MAXSTR];
    static mut wizard: c_int;

    // ── Game-control globals from extern.c ────────────────────────────────
    static mut after: c_uchar;
    static mut count: c_int;
    static mut in_shell: c_uchar;
    static mut inv_type: c_int;
    static mut jump: c_uchar;
    static mut mpos: c_int;
    static mut oldpos: IVec2;
    static mut oldrp: Option<usize>;
    static mut playing: c_uchar;
    static mut q_comm: c_uchar;
    static mut running: c_uchar;
    static mut see_floor: c_uchar;
    static mut terse: c_uchar;
    static mut to_death: c_uchar;
}

#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn arg_at(argv: *mut *mut c_char, index: usize) -> *mut c_char {
    *argv.add(index)
}

// ── Game control functions ported from src/c/main.c ─────────────────────────

/// endit:
/// Exit the program abnormally.
///
/// No globals used directly.
#[no_mangle]
pub unsafe extern "C" fn endit(sig: c_int) {
    let _ = sig;
    fatal(c"Okay, bye bye!\n".as_ptr() as *mut c_char);
}

/// fatal:
/// Exit the program, printing a message.
///
/// No globals used directly.
#[no_mangle]
pub unsafe extern "C" fn fatal(s: *mut c_char) {
    output::write_text_at(
        IVec2::new(0, GameConfig::SCREEN_LINES - 2),
        &CStr::from_ptr(s).to_string_lossy(),
    );
    output::refresh();
    runtime::shutdown();
    my_exit(0);
}

/// roll:
/// Roll a number of dice.
///
/// No globals used directly (uses rnd()).
#[no_mangle]
pub unsafe extern "C" fn roll(mut number: c_int, sides: c_int) -> c_int {
    let mut dtotal = 0;

    while number > 0 {
        dtotal += rnd(sides) + 1;
        number -= 1;
    }
    dtotal
}

/// tstp:
/// Handle stop and start signals.
#[no_mangle]
pub unsafe extern "C" fn tstp(ignored: c_int) {
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
    fflush(c_stdout());
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
    fflush(c_stdout());
}

/// playit:
/// The main loop of the program.  Loop until the game is over,
/// refreshing things and looking at the proper times.
///
/// Uses globals: terse, jump, see_floor, inv_type, oldpos, oldrp,
/// hero, playing, running.
#[no_mangle]
pub unsafe extern "C" fn playit() {
    /*
     * set up defaults for slow terminals
     */
    if runtime::baud_rate() <= 1200 {
        terse = true as c_uchar;
        jump = true as c_uchar;
        see_floor = false as c_uchar;
    }

    if md_hasclreol() != 0 {
        inv_type = INV_CLEAR;
    }

    /*
     * parse environment declaration of options
     */
    let c_options = std::env::var_os("ROGUEOPTS")
        .and_then(|value| CString::new(value.into_encoded_bytes()).ok());
    if let Some(options) = c_options.as_ref() {
        parse_opts(options.as_ptr() as *mut c_char);
    }

    oldpos = crate::game::PLAYER.pos();
    let mut hero_pos = crate::game::PLAYER.pos();
    oldrp = roomin(&raw mut hero_pos);
    while playing != false as c_uchar {
        command(); /* Command execution */
    }
    endit(0);
}

/// quit:
/// Have player make certain, then exit.
///
/// Uses globals: q_comm, mpos, purse, count, to_death.
#[no_mangle]
pub unsafe extern "C" fn quit(sig: c_int) {
    let _ = sig;

    /*
     * Reset the signal in case we got here via an interrupt
     */
    if q_comm == false as c_uchar {
        mpos = 0;
    }
    let old_cursor = output::window_cursor(Window::Curscr);
    msg_str("really quit?");
    if readchar() == b'y' as c_int {
        signal(SIGINT, leave as usize);
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
        to_death = false as c_uchar;
    }
}

/// leave:
/// Leave quickly, but curteously.
#[no_mangle]
pub unsafe extern "C" fn leave(sig: c_int) {
    let _ = sig;

    setbuf(
        c_stdout(),
        std::ptr::addr_of_mut!(LEAVE_BUF).cast::<c_char>(),
    ); /* throw away pending output */

    if !runtime::is_shutdown() {
        runtime::move_physical_cursor(
            IVec2::new(GameConfig::SCREEN_COLS - 1, 0),
            IVec2::new(0, GameConfig::SCREEN_LINES - 1),
        );
        runtime::shutdown();
    }

    putchar(b'\n' as c_int);
    my_exit(0);
}

/// shell:
/// Let them escape for a while.
///
/// Uses globals: in_shell, after.
#[no_mangle]
pub unsafe extern "C" fn shell() {
    /*
     * Set the terminal back to original mode
     */
    output::move_cursor(IVec2::new(0, GameConfig::SCREEN_LINES - 1));
    output::refresh();
    runtime::shutdown();
    resetltchars();
    putchar(b'\n' as c_int);
    in_shell = true as c_uchar;
    after = false as c_uchar;
    fflush(c_stdout());
    /*
     * Fork and do a shell
     */
    md_shellescape();

    printf(c"\n[Press return to continue]".as_ptr());
    fflush(c_stdout());
    input::set_echo(false);
    input::set_raw_mode(true);
    input::set_keypad(Window::Stdscr, true);
    playltchars();
    in_shell = false as c_uchar;
    wait_for('\n');
    output::set_clear_on_refresh(Window::Stdscr, true);
}

/// my_exit:
/// Leave the process properly.
///
/// No globals used directly.
#[no_mangle]
pub unsafe extern "C" fn my_exit(st: c_int) -> ! {
    resetltchars();
    if !runtime::is_shutdown() {
        input::set_echo(true);
        runtime::shutdown();
    }
    fflush(c_stdout());
    fflush(c_stderr());
    exit(st);
}

/// The C-ABI entry point, kept for the (legacy) `make` link path.
/// A native Rust `main` in `src/bin/rogue.rs` calls this with
/// argv/envp built from `std::env::args_os`.
#[no_mangle]
pub unsafe extern "C" fn rogue_main(
    mut argc: c_int,
    mut argv: *mut *mut c_char,
    envp: *mut *mut c_char,
) -> c_int {
    md_init();

    if master_mode_enabled != 0 && argc >= 2 && *arg_at(argv, 1) == 0 {
        wizard = 1;
        crate::game::PLAYER.add_flag(MonsterFlags::SEEMONST);
        argv = argv.add(1);
        argc -= 1;
    }

    let home_dir = md_gethomedir();
    let home_len = CStr::from_ptr(home_dir)
        .to_bytes_with_nul()
        .len()
        .min(MAXSTR);
    std::ptr::copy_nonoverlapping(
        home_dir,
        std::ptr::addr_of_mut!(home).cast::<c_char>(),
        home_len,
    );
    std::ptr::copy_nonoverlapping(
        home_dir,
        std::ptr::addr_of_mut!(file_name).cast::<c_char>(),
        home_len,
    );
    let save_name = b"rogue.save\0";
    let name_start = home_len.saturating_sub(1);
    std::ptr::copy_nonoverlapping(
        save_name.as_ptr() as *const c_char,
        std::ptr::addr_of_mut!(file_name)
            .cast::<c_char>()
            .add(name_start),
        save_name.len(),
    );

    let options = std::env::var_os("ROGUEOPTS")
        .and_then(|value| CString::new(value.into_encoded_bytes()).ok());
    if let Some(options) = options.as_ref() {
        parse_opts(options.as_ptr() as *mut c_char);
    }
    if options.is_none() || whoami[0] == 0 {
        let username = md_getusername();
        strucpy(
            std::ptr::addr_of_mut!(whoami).cast::<c_char>(),
            username,
            CStr::from_ptr(username).to_bytes().len() as c_int,
        );
    }

    let clock_seed = time(std::ptr::null_mut()) as c_int + md_getpid();
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
        let argument = CStr::from_ptr(arg_at(argv, 1)).to_bytes();
        if argument == b"-s" {
            noscore = 1;
            score(0, -1, 0);
            return 0;
        }
        if argument == b"-d" {
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
    if argc == 2 && restore(arg_at(argv, 1), envp) == 0 {
        my_exit(1);
    }

    if master_mode_enabled != 0 && wizard != 0 {
        print!(
            "Hello {}, welcome to dungeon #{}",
            CStr::from_ptr(std::ptr::addr_of!(whoami).cast::<c_char>()).to_string_lossy(),
            std::ptr::addr_of!(dnum).read()
        );
    } else {
        print!(
            "Hello {}, just a moment while I dig the dungeon...",
            CStr::from_ptr(std::ptr::addr_of!(whoami).cast::<c_char>()).to_string_lossy()
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
