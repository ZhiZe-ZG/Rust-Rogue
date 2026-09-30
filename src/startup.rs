//! Process startup sequence, ported from `src/c/main.c`.

use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crate::command_line_parameter::CommandLineParameter;
use crate::command_dispatch::{do_command, CommandState};
use crate::config::GameConfig;
use crate::daemon::{fuse, start_daemon, Daemon};
use crate::entity::chase::roomin;
use crate::entity::player::MonsterFlags;
use crate::game::globals::{
    after, count, get_purse, in_shell, inv_type, master_mode_enabled, mpos,
    noscore, oldpos, oldrp, playing, purse, q_comm, seed, to_death, wizard,
};
use crate::init::{init_colors, init_materials, init_names, init_player, init_probs, init_stones};
use crate::level::new_level;
use crate::machdep::{init_check, open_score, setup};
use crate::options::parse_opts;
use crate::rip::score;
use crate::rnd::{rnd, set_seed};
use crate::save::{restore, RestoreError};
use crate::ui::input::{self, readchar, wait_for};
use crate::ui::output::{self, msg_str, status};
use crate::ui::terminal;
use glam::IVec2;
use std::time::{SystemTime, UNIX_EPOCH};

const AFTER: i32 = 2;
const WANDERTIME: i32 = 70;
const INV_CLEAR: i32 = 2;
static EXIT_ON_INTERRUPT: AtomicBool = AtomicBool::new(false);
static REQUESTED_EXIT: AtomicI32 = AtomicI32::new(-1);

pub(crate) fn request_exit(code: i32) {
    let _ = REQUESTED_EXIT.compare_exchange(-1, code, Ordering::Relaxed, Ordering::Relaxed);
}

pub(crate) fn exit_requested() -> bool {
    REQUESTED_EXIT.load(Ordering::Relaxed) >= 0
}

fn requested_exit_code() -> Option<i32> {
    match REQUESTED_EXIT.load(Ordering::Relaxed) {
        -1 => None,
        code => Some(code),
    }
}

#[derive(Debug)]
pub enum StartupError {
    PrivilegeDrop {
        operation: &'static str,
        source: io::Error,
    },
    SignalHandlers(io::Error),
    StartupOutput(io::Error),
    Restore(RestoreError),
    TerminalTooSmall {
        actual: IVec2,
        required: IVec2,
    },
}

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrivilegeDrop { operation, source } => {
                write!(formatter, "could not {operation}: {source}")
            }
            Self::SignalHandlers(source) => {
                write!(formatter, "could not install signal handlers: {source}")
            }
            Self::StartupOutput(source) => {
                write!(formatter, "could not flush startup message: {source}")
            }
            Self::Restore(source) => write!(formatter, "could not restore game: {source}"),
            Self::TerminalTooSmall { actual, required } => write!(
                formatter,
                "terminal is {}x{}; at least {}x{} is required",
                actual.x, actual.y, required.x, required.y
            ),
        }
    }
}

impl Error for StartupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PrivilegeDrop { source, .. }
            | Self::SignalHandlers(source)
            | Self::StartupOutput(source) => Some(source),
            Self::Restore(source) => Some(source),
            Self::TerminalTooSmall { .. } => None,
        }
    }
}

fn install_signal_handlers() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use signal_hook::consts::{SIGINT, SIGTSTP};
        use signal_hook::iterator::Signals;

        let mut signals = Signals::new([SIGINT, SIGTSTP])?;
        std::thread::Builder::new()
            .name("rogue-signals".to_owned())
            .spawn(move || {
                for signal in signals.forever() {
                    match signal {
                        SIGTSTP => suspend_terminal(),
                        SIGINT if EXIT_ON_INTERRUPT.load(Ordering::Relaxed) => request_exit(0),
                        SIGINT => request_exit(128 + signal),
                        _ => {}
                    }
                }
            })?;
    }
    Ok(())
}

#[cfg(unix)]
fn suspend_terminal() {
    use signal_hook::consts::SIGTSTP;

    let old_cursor = terminal::UI.cursor_pos();
    output::flush_now();
    let _ = std::io::stdout().flush();
    let _ = signal_hook::low_level::emulate_default_handler(SIGTSTP);
    terminal::UI.raw();
    output::refresh();
    terminal::UI.move_cursor(old_cursor);
    let _ = std::io::stdout().flush();
}

fn drop_privileges() -> Result<(), StartupError> {
    #[cfg(unix)]
    unsafe {
        let real_gid = libc::getgid();
        let real_uid = libc::getuid();

        #[cfg(any(target_os = "linux", target_os = "android"))]
        let group_error = libc::setresgid((-1i32) as libc::gid_t, real_gid, real_gid) != 0;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let group_error = libc::setregid(real_gid, real_gid) != 0;
        if group_error {
            return Err(StartupError::PrivilegeDrop {
                operation: "drop setgid privileges",
                source: io::Error::last_os_error(),
            });
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        let user_error = libc::setresuid((-1i32) as libc::uid_t, real_uid, real_uid) != 0;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let user_error = libc::setreuid(real_uid, real_uid) != 0;
        if user_error {
            return Err(StartupError::PrivilegeDrop {
                operation: "drop setuid privileges",
                source: io::Error::last_os_error(),
            });
        }
    }
    Ok(())
}

/// Roll a number of dice.
pub fn roll(mut number: i32, sides: i32) -> i32 {
    let mut dtotal = 0;

    while number > 0 {
        dtotal += rnd(sides) + 1;
        number -= 1;
    }
    dtotal
}

/// playit:
/// The main loop of the program.  Loop until the game is over,
/// refreshing things and looking at the proper times.
///
/// Uses globals: inv_type, oldpos, oldrp, playing.
pub unsafe fn main_loop_step() {
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
    while playing != false as u8 && !exit_requested() {
        do_command(&mut command_state); /* Command execution */
    }
}

/// quit:
/// Have player make certain, then exit.
///
/// Uses globals: q_comm, mpos, purse, count, to_death.
pub unsafe fn quit() {
    /*
     * Reset the signal in case we got here via an interrupt
     */
    if q_comm == false as u8 {
        mpos = 0;
    }
    let old_cursor = crate::ui::terminal::UI.cursor_pos();
    msg_str("really quit?");
    if readchar() == b'y' as i32 {
        crate::ui::terminal::UI.clear();
        let line = format!("You quit with {} gold pieces", get_purse());
        crate::ui::terminal::UI.write_text_at(IVec2::new(0, GameConfig::SCREEN_LINES - 2), &line);
        crate::ui::terminal::UI.move_cursor(IVec2::new(0, GameConfig::SCREEN_LINES - 1));
        output::refresh();
        EXIT_ON_INTERRUPT.store(true, Ordering::Relaxed);
        score(purse, 1, 0);
        request_exit(0);
    } else {
        crate::ui::terminal::UI.move_cursor(IVec2::new(0, 0));
        crate::ui::terminal::UI.clear_to_end_of_line();
        status();
        crate::ui::terminal::UI.move_cursor(old_cursor);
        output::refresh();
        mpos = 0;
        count = 0;
        to_death = false as u8;
    }
}

/// shell:
/// Let them escape for a while.
///
/// Uses globals: in_shell, after.
pub unsafe fn shell() {
    /*
     * Set the terminal back to original mode
     */
    crate::ui::terminal::UI.move_cursor(IVec2::new(0, GameConfig::SCREEN_LINES - 1));
    output::refresh();
    output::flush_now();
    let _ = std::io::stdout().write_all(b"\n");
    in_shell = true as u8;
    after = false as u8;
    let _ = std::io::stdout().flush();
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
    terminal::UI.raw();
    in_shell = false as u8;
    wait_for('\n');
}


/// The game entry point. `args` mirrors the process `argv` (including the
/// program name at index 0); `src/bin/rogue.rs` calls this with
/// `std::env::args()`.
pub unsafe fn rogue_main(parameter: CommandLineParameter) -> ! {
    let exit_code = match run_startup(parameter) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("rogue: {error}");
            1
        }
    };
    terminal::UI.deinit_terminal();
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::process::exit(exit_code);
}

unsafe fn run_startup(parameter: CommandLineParameter) -> Result<i32, StartupError> {
    // Init terminal
    terminal::UI.init_terminal();
    let mut restore_target = parameter.restore.or(parameter.save_file);
    if master_mode_enabled != 0 && restore_target.as_deref() == Some("") {
        wizard = 1;
        crate::game::PLAYER.add_flag(MonsterFlags::SEEMONST);
        restore_target = None;
    }
    
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
    seed = parameter.seed.unwrap_or(clock_seed);
    set_seed(seed);
    open_score();
    drop_privileges()?;
    install_signal_handlers().map_err(StartupError::SignalHandlers)?;

    init_check();
    if let Some(save_file) = restore_target {
        restore(&save_file).map_err(StartupError::Restore)?;
        return Ok(requested_exit_code().unwrap_or(0));
    }

    std::io::stdout()
        .flush()
        .map_err(StartupError::StartupOutput)?;
    // Reject terminals smaller than the fixed game grid. The physical size is
    // unavailable on some backends; in that case keep the legacy permissive
    // behaviour and continue.
    if let Some(size) = terminal::UI.physical_size() {
        if size.y < GameConfig::SCREEN_LINES || size.x < GameConfig::SCREEN_COLS {
            output::flush_now();
            return Err(StartupError::TerminalTooSmall {
                actual: size,
                required: IVec2::new(GameConfig::SCREEN_COLS, GameConfig::SCREEN_LINES),
            });
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
    main_loop_step();
    Ok(requested_exit_code().unwrap_or(0))
}

