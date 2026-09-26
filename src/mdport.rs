//! Machine-dependent portability layer, ported from `src/c/mdport.c`.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.
//!
//! mdport.c was written by Nicholas J. Kisseberth (C) 2005.
//!
//! This module provides the `md_*` machine-dependent functions formerly
//! implemented in `src/c/mdport.c`.  The port targets POSIX (Linux/macOS)
//! and retains the same C ABI so existing Rust callers keep working.

use std::os::raw::{c_int, c_uint, c_void};

use crate::save::auto_save;
use crate::startup::{endit, quit, tstp};
use crate::ui::input;
use crate::ui::output;

/// Reads a NUL-terminated byte string from a raw pointer into an owned Rust
/// `String`, without using any C string type.
unsafe fn c_ptr_to_string(p: *const u8) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf8_lossy(std::slice::from_raw_parts(p, len)).into_owned()
}

/// Curses key codes used by the keypad/arrow-key reader. These mirror the
/// ncurses public header (`keys.h`) values so behaviour is identical to the
/// original C `mdport.c` key translation. The arrow keys reuse the derived
/// integer codes; the remaining numeric literals are the ncurses keypad
/// constants for this game's arrow/keypad mapping.
const KEY_DOWN: c_int = 0o402; // 258
const KEY_UP: c_int = 0o403; // 259
const KEY_LEFT: c_int = 0o404; // 260
const KEY_RIGHT: c_int = 0o405; // 261
const KEY_HOME: c_int = 0o406; // 262
const KEY_BACKSPACE: c_int = 0o407; // 263
const KEY_NPAGE: c_int = 0o522; // 338
const KEY_PPAGE: c_int = 0o523; // 339
const KEY_LL: c_int = 0o545; // 357
const KEY_A1: c_int = 0o534; // 348
const KEY_A3: c_int = 0o536; // 350
const KEY_B2: c_int = 0o541; // 353
const KEY_C1: c_int = 0o542; // 354
const KEY_C3: c_int = 0o544; // 356
const KEY_END: c_int = 0o550; // 360

// Extended keypad codes not covered above; values match ncurses `keys.h`.
const KEY_B1: c_int = 353; // keypad lower-left
const KEY_B3: c_int = 354; // keypad lower-right
const KEY_A2: c_int = 355; // keypad up
const KEY_C2: c_int = 356; // keypad down
const KEY_SUP: c_int = 337; // shift up
const KEY_SDOWN: c_int = 336; // shift down
const KEY_SEND: c_int = 0o551; // 361
const KEY_SHOME: c_int = 0o552; // 362
const KEY_SLEFT: c_int = 0o553; // 363
const KEY_SNEXT: c_int = 0o556; // 366
const KEY_SPREVIOUS: c_int = 0o557; // 367
const KEY_SRIGHT: c_int = 0o560; // 368
const KEY_EOL: c_int = 0o600; // 384
const ERR: c_int = -1;

// -------------------------------------------------------------------------
// Signal handling
// -------------------------------------------------------------------------

/// md_onsignal_default:
/// Restore default signal disposition for common termination signals.
#[no_mangle]
pub unsafe extern "C" fn md_onsignal_default() {
    #[cfg(unix)]
    {
        libc::signal(libc::SIGHUP, libc::SIG_DFL);
        libc::signal(libc::SIGQUIT, libc::SIG_DFL);
        libc::signal(libc::SIGILL, libc::SIG_DFL);
        libc::signal(libc::SIGTRAP, libc::SIG_DFL);
        libc::signal(libc::SIGABRT, libc::SIG_DFL);
        libc::signal(libc::SIGFPE, libc::SIG_DFL);
        libc::signal(libc::SIGBUS, libc::SIG_DFL);
        libc::signal(libc::SIGSEGV, libc::SIG_DFL);
        libc::signal(libc::SIGSYS, libc::SIG_DFL);
        libc::signal(libc::SIGTERM, libc::SIG_DFL);
    }
}

/// md_onsignal_exit:
/// Arrange for signals to exit the program.
#[no_mangle]
pub unsafe extern "C" fn md_onsignal_exit() {
    #[cfg(unix)]
    {
        let exit_h: libc::sighandler_t = libc::exit as libc::sighandler_t;
        libc::signal(libc::SIGHUP, libc::SIG_DFL);
        libc::signal(libc::SIGQUIT, exit_h);
        libc::signal(libc::SIGILL, exit_h);
        libc::signal(libc::SIGTRAP, exit_h);
        libc::signal(libc::SIGABRT, exit_h);
        libc::signal(libc::SIGFPE, exit_h);
        libc::signal(libc::SIGBUS, exit_h);
        libc::signal(libc::SIGSEGV, exit_h);
        libc::signal(libc::SIGSYS, exit_h);
        libc::signal(libc::SIGTERM, exit_h);
        libc::signal(libc::SIGINT, exit_h);
    }
}

/// md_onsignal_autosave:
/// Arrange for signals to auto-save the game.
unsafe fn md_onsignal_autosave() {
    // The auto-save handlers (auto_save, endit, quit) are Rust `#[no_mangle]`
    // functions; wire them up to the signals on Unix.
    #[cfg(unix)]
    {
        libc::signal(libc::SIGHUP, auto_save as libc::sighandler_t);
        libc::signal(libc::SIGQUIT, endit as libc::sighandler_t);
        libc::signal(libc::SIGILL, auto_save as libc::sighandler_t);
        libc::signal(libc::SIGTRAP, auto_save as libc::sighandler_t);
        libc::signal(libc::SIGABRT, auto_save as libc::sighandler_t);
        libc::signal(libc::SIGFPE, auto_save as libc::sighandler_t);
        libc::signal(libc::SIGBUS, auto_save as libc::sighandler_t);
        libc::signal(libc::SIGSEGV, auto_save as libc::sighandler_t);
        libc::signal(libc::SIGSYS, auto_save as libc::sighandler_t);
        libc::signal(libc::SIGTERM, auto_save as libc::sighandler_t);
        libc::signal(libc::SIGINT, quit as libc::sighandler_t);
    }
}

/// md_ignoreallsignals:
/// Ignore all signals.
#[no_mangle]
pub unsafe extern "C" fn md_ignoreallsignals() {
    // libc::NSIG is not exposed by the Rust libc crate; 32 matches the
    // `#ifndef NSIG  #define NSIG 32` fallback in the original mdport.c.
    for sig in 0..32 {
        libc::signal(sig, libc::SIG_IGN);
    }
}

/// md_init:
/// Perform machine-dependent startup initialization.
#[no_mangle]
pub unsafe extern "C" fn md_init() {
    #[cfg(unix)]
    {
        // ESCDELAY is a curses global; the ncurses crate exposes set_escdelay().
        input::set_escape_delay(64);
    }
    md_onsignal_exit();
}

/// md_hasclreol:
/// Return true if the terminal supports clear-to-end-of-line.
#[no_mangle]
pub unsafe extern "C" fn md_hasclreol() -> c_int {
    // The ncurses crate doesn't expose clr_eol/CE directly.  Assume the
    // terminal supports it (all common terminals do).
    1
}

/// md_putchar:
/// Output a single character.
unsafe fn md_putchar(c: c_int) {
    libc::putchar(c);
}

// -------------------------------------------------------------------------
// Standout / raw-mode output
// -------------------------------------------------------------------------

/// md_raw_standout:
/// Turn on standout (reverse-video) output.
#[no_mangle]
pub unsafe extern "C" fn md_raw_standout() {
    output::set_standout(true);
}

/// md_raw_standend:
/// Turn off standout (reverse-video) output.
#[no_mangle]
pub unsafe extern "C" fn md_raw_standend() {
    output::set_standout(false);
}

// -------------------------------------------------------------------------
// File operations
// -------------------------------------------------------------------------

/// md_unlink_open_file:
/// Unlink an open file.  On POSIX there is nothing special to do beyond
/// unlinking the path.
#[no_mangle]
pub unsafe fn md_unlink_open_file(file: &str, _inf: *mut c_void) -> c_int {
    let bytes = crate::ffi::to_c_bytes(file);
    libc::unlink(bytes.as_ptr() as *const libc::c_char)
}

/// md_unlink:
/// Remove a file.
#[no_mangle]
pub unsafe fn md_unlink(file: &str) -> c_int {
    let bytes = crate::ffi::to_c_bytes(file);
    libc::unlink(bytes.as_ptr() as *const libc::c_char)
}

/// md_chmod:
/// Change file permissions.
#[no_mangle]
pub unsafe fn md_chmod(filename: &str, mode: c_int) -> c_int {
    let bytes = crate::ffi::to_c_bytes(filename);
    libc::chmod(bytes.as_ptr() as *const libc::c_char, mode as libc::mode_t)
}

// -------------------------------------------------------------------------
// User / process identity
// -------------------------------------------------------------------------

/// md_normaluser:
/// Drop setuid/setgid privileges so the game runs as the real user.
///
/// Mirrors the original mdport.c: each platform uses exactly one
/// privilege-dropping call (the most capable one available) with -1
/// (keep current) for the real uid/gid slot.
#[no_mangle]
pub unsafe extern "C" fn md_normaluser() {
    #[cfg(unix)]
    {
        let realgid = libc::getgid();
        let realuid = libc::getuid();

        // Drop group privileges (one call, R/E/S all set to real gid).
        // `-1` means "keep current real id"; cast to the unsigned type so the
        // value wraps to the same sentinel the C version passes.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let gerr = libc::setresgid((-1i32) as libc::gid_t, realgid, realgid) != 0;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let gerr = libc::setregid(realgid, realgid) != 0;
        if gerr {
            libc::perror(c"Could not drop setgid privileges.  Aborting.".as_ptr());
            libc::exit(1);
        }

        // Drop user privileges (one call, R/E/S all set to real uid).
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let uerr = libc::setresuid((-1i32) as libc::uid_t, realuid, realuid) != 0;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let uerr = libc::setreuid(realuid, realuid) != 0;
        if uerr {
            libc::perror(c"Could not drop setuid privileges.  Aborting.".as_ptr());
            libc::exit(1);
        }
    }
}

/// md_getuid:
/// Return the real user id.
#[no_mangle]
pub unsafe extern "C" fn md_getuid() -> c_uint {
    #[cfg(unix)]
    {
        libc::getuid() as c_uint
    }
    #[cfg(not(unix))]
    {
        42
    }
}

/// md_getpid:
/// Return the process id.
#[no_mangle]
pub unsafe extern "C" fn md_getpid() -> c_int {
    #[cfg(unix)]
    {
        libc::getpid() as c_int
    }
    #[cfg(not(unix))]
    {
        0
    }
}

// -------------------------------------------------------------------------
// User / environment helpers
// -------------------------------------------------------------------------

/// md_getusername:
/// Return the login name of the current user.
#[no_mangle]
pub unsafe fn md_getusername() -> String {
    #[cfg(unix)]
    {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_name.is_null() {
            let name = c_ptr_to_string((*pw).pw_name as *const u8);
            if !name.is_empty() {
                return name;
            }
        }
    }

    for var in ["USERNAME", "LOGNAME", "USER"] {
        if let Ok(value) = std::env::var(var) {
            if !value.is_empty() {
                return value;
            }
        }
    }
    "nobody".to_string()
}

/// md_gethomedir:
/// Return the home directory of the current user, with a trailing slash.
#[no_mangle]
pub unsafe fn md_gethomedir() -> String {
    let mut home: Option<String> = None;

    #[cfg(unix)]
    {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_dir.is_null() {
            let dir = c_ptr_to_string((*pw).pw_dir as *const u8);
            // A bare "/" is not a useful home directory; fall back to $HOME.
            if !dir.is_empty() && dir != "/" {
                home = Some(dir);
            }
        }
    }

    let mut home = home
        .or_else(|| std::env::var("HOME").ok())
        .unwrap_or_default();
    if !home.is_empty() && !home.ends_with('/') {
        home.push('/');
    }
    home
}

/// md_sleep:
/// Sleep for the given number of seconds.
#[no_mangle]
pub unsafe extern "C" fn md_sleep(s: c_int) {
    #[cfg(unix)]
    {
        libc::sleep(s as c_uint);
    }
}

/// md_getshell:
/// Return the user's login shell.
#[no_mangle]
pub unsafe fn md_getshell() -> String {
    #[cfg(unix)]
    {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_shell.is_null() {
            let sh = c_ptr_to_string((*pw).pw_shell as *const u8);
            if !sh.is_empty() {
                return sh;
            }
        }
    }

    for var in ["COMSPEC", "SHELL", "SystemRoot"] {
        if let Ok(value) = std::env::var(var) {
            if !value.is_empty() {
                return value;
            }
        }
    }
    "/bin/sh".to_string()
}

/// md_shellescape:
/// Escape to a shell; return the exit status of the shell.
#[no_mangle]
pub unsafe extern "C" fn md_shellescape() -> c_int {
    #[cfg(unix)]
    {
        let sh = md_getshell();
        let mut pid = libc::fork();
        while pid < 0 {
            libc::sleep(1);
            pid = libc::fork();
        }

        let mut ret_status: c_int = 0;

        if pid == 0 {
            // Shell process: drop privileges then exec the shell.
            md_normaluser();
            let shell_bytes = crate::ffi::to_c_bytes(&sh);
            libc::execl(
                shell_bytes.as_ptr() as *const libc::c_char,
                c"shell".as_ptr(),
                c"-i".as_ptr(),
                std::ptr::null::<libc::c_char>(),
            );
            libc::perror(c"No shelly".as_ptr());
            libc::_exit(-1);
        } else {
            // Application: ignore interrupt/quit while the shell runs.
            let myend = libc::signal(libc::SIGINT, libc::SIG_IGN);
            let myquit = libc::signal(libc::SIGQUIT, libc::SIG_IGN);
            while libc::wait(&mut ret_status) != pid {
                // spin
            }
            libc::signal(libc::SIGINT, myquit);
            libc::signal(libc::SIGQUIT, myend);
        }
        ret_status
    }
    #[cfg(not(unix))]
    {
        0
    }
}

// -------------------------------------------------------------------------
// Filesystem helpers
// -------------------------------------------------------------------------

/// directory_exists:
/// Return 1 if the given path is a directory, 0 otherwise.
unsafe fn directory_exists(dirname: &str) -> c_int {
    match std::fs::metadata(dirname) {
        Ok(md) => {
            if md.is_dir() {
                1
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

/// md_getrealname:
/// Return the real (login) name for the given uid, or the numeric uid
/// string if no passwd entry exists.
unsafe fn md_getrealname(uid: c_int) -> String {
    #[cfg(unix)]
    {
        let pw = libc::getpwuid(uid as libc::uid_t);
        if !pw.is_null() && !(*pw).pw_name.is_null() {
            return c_ptr_to_string((*pw).pw_name as *const u8);
        }
    }
    uid.to_string()
}

// -------------------------------------------------------------------------
// Tty character helpers
// -------------------------------------------------------------------------

/// md_erasechar:
/// Return the terminal erase character.
unsafe fn md_erasechar() -> c_int {
    input::erase_key() as c_int
}

/// md_killchar:
/// Return the terminal kill character.
unsafe fn md_killchar() -> c_int {
    input::kill_key() as c_int
}

/// md_dsuspchar:
/// Return the terminal delete-suspend character.
#[no_mangle]
pub unsafe extern "C" fn md_dsuspchar() -> c_int {
    // No portable POSIX VDSUSP; use 0 (which the caller treats as "disabled").
    0
}

/// md_setdsuspchar:
/// Set the terminal delete-suspend character.
#[no_mangle]
pub unsafe extern "C" fn md_setdsuspchar(_c: c_int) -> c_int {
    0
}

/// md_suspchar:
/// Return the terminal suspend character.
#[no_mangle]
pub unsafe extern "C" fn md_suspchar() -> c_int {
    #[cfg(unix)]
    {
        let mut attr = std::mem::zeroed::<libc::termios>();
        // STDIN_FILENO === 0 on POSIX.
        if libc::tcgetattr(0, &mut attr) == 0 {
            return attr.c_cc[libc::VSUSP] as c_int;
        }
        0
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// md_setsuspchar:
/// Set the terminal suspend character.
unsafe fn md_setsuspchar(_c: c_int) -> c_int {
    // Changing the suspend char is rarely needed; keep the ncurses setting.
    0
}

// -------------------------------------------------------------------------
// Cursor / keypad support
// -------------------------------------------------------------------------

const M_NORMAL: c_int = 0;
const M_ESC: c_int = 1;
const M_KEYPAD: c_int = 2;
const M_TRAIL: c_int = 3;

/// md_readchar:
/// Read a character, translating cursor/keypad escape sequences into the
/// classic rogue movement commands (h j k l y u b n, plus Ctrl-modified runs).
#[no_mangle]
pub unsafe extern "C" fn md_readchar() -> c_int {
    let mut ch = 0;
    let mut lastch = 0;
    let mut mode = M_NORMAL;
    let mut mode2 = M_NORMAL;

    loop {
        ch = input::read_raw_key();

        if ch == ERR {
            // Timed out waiting for a valid sequence: flush and treat as ESC.
            mode = M_NORMAL;
            input::set_raw_mode(false);
            input::set_raw_mode(true);
            ch = 27;
            break;
        }

        if mode == M_TRAIL {
            // msys console: '^' prefix means modified.
            if ch == '^' as c_int {
                ch = ctrl_upcase(lastch);
            }
            // cygwin/telnet: '~' suffix means normal.
            if ch == '~' as c_int {
                ch = (lastch as u8).to_ascii_lowercase() as c_int;
            }
            if mode2 == M_ESC {
                ch = ctrl_upcase(ch);
            }
            break;
        }

        if mode == M_ESC {
            if ch == 27 {
                mode2 = M_ESC;
                continue;
            }
            if ch == 'F' as c_int || ch == 'O' as c_int || ch == '[' as c_int {
                mode = M_KEYPAD;
                continue;
            }

            // Cygwin / PuTTY: cooked cursor keys.
            match ch {
                KEY_LEFT => ch = ctrl('H'),
                KEY_RIGHT => ch = ctrl('L'),
                KEY_UP => ch = ctrl('K'),
                KEY_DOWN => ch = ctrl('J'),
                KEY_HOME => ch = ctrl('Y'),
                KEY_PPAGE => ch = ctrl('U'),
                KEY_NPAGE => ch = ctrl('N'),
                KEY_END => ch = ctrl('B'),
                _ => {}
            }
            break;
        }

        if mode == M_KEYPAD {
            match ch {
                // Interix: shift-left/shift-right.
                0x5E => ch = ctrl('H'), // '^'
                0x24 => ch = ctrl('L'), // '$'
                // Interix: home.
                0x48 => ch = 'y' as c_int, // 'H'
                // Interix: ctrl-keypad.
                1 => ch = ctrl('K'),
                2 => ch = ctrl('J'),
                3 => ch = ctrl('L'),
                4 => ch = ctrl('H'),
                263 => ch = ctrl('Y'),
                19 => ch = ctrl('U'),
                20 => ch = ctrl('N'),
                21 => ch = ctrl('B'),
                // Cygwin: keypad 5.
                0x47 => ch = '.' as c_int, // 'G'
                // Cygwin: ctrl-home/page.
                0x37 => {
                    // '7'
                    lastch = 'Y' as c_int;
                    mode = M_TRAIL;
                }
                0x35 => {
                    // '5'
                    lastch = 'U' as c_int;
                    mode = M_TRAIL;
                }
                0x36 => {
                    // '6'
                    lastch = 'N' as c_int;
                    mode = M_TRAIL;
                }
                // Win32 telnet / PuTTY: home/end.
                0x31 => {
                    // '1'
                    lastch = 'y' as c_int;
                    mode = M_TRAIL;
                }
                0x34 => {
                    // '4'
                    lastch = 'b' as c_int;
                    mode = M_TRAIL;
                }
                // PuTTY ESC O sequences.
                0x44 => ch = ctrl('H'),    // 'D'
                0x43 => ch = ctrl('L'),    // 'C'
                0x41 => ch = ctrl('K'),    // 'A'
                0x42 => ch = ctrl('J'),    // 'B'
                0x74 => ch = 'h' as c_int, // 't'
                0x76 => ch = 'l' as c_int, // 'v'
                0x78 => ch = 'k' as c_int, // 'x'
                0x72 => ch = 'j' as c_int, // 'r'
                0x77 => ch = 'y' as c_int, // 'w'
                0x79 => ch = 'u' as c_int, // 'y'
                0x73 => ch = 'n' as c_int, // 's'
                0x71 => ch = 'b' as c_int, // 'q'
                0x75 => ch = '.' as c_int, // 'u'
                _ => {}
            }

            if mode != M_KEYPAD {
                continue;
            }
        }

        if ch == 27 {
            input::set_input_timeout(1);
            mode = M_ESC;
            continue;
        }

        // Handle cooked curses keys.
        match ch {
            KEY_LEFT => ch = 'h' as c_int,
            KEY_DOWN => ch = 'j' as c_int,
            KEY_UP => ch = 'k' as c_int,
            KEY_RIGHT => ch = 'l' as c_int,
            KEY_HOME => ch = 'y' as c_int,
            KEY_PPAGE => ch = 'u' as c_int,
            KEY_END => ch = 'b' as c_int,
            KEY_LL => ch = 'b' as c_int,
            KEY_NPAGE => ch = 'n' as c_int,
            KEY_B1 => ch = 'h' as c_int,
            KEY_C2 => ch = 'j' as c_int,
            KEY_A2 => ch = 'k' as c_int,
            KEY_B3 => ch = 'l' as c_int,
            KEY_A1 => ch = 'y' as c_int,
            KEY_A3 => ch = 'u' as c_int,
            KEY_C1 => ch = 'b' as c_int,
            KEY_C3 => ch = 'n' as c_int,
            // next should be '.', but there is a problem with putty/linux
            KEY_B2 => ch = 'u' as c_int,
            KEY_SRIGHT => ch = ctrl('L'),
            KEY_SLEFT => ch = ctrl('H'),
            KEY_SUP => ch = ctrl('K'),
            KEY_SDOWN => ch = ctrl('J'),
            KEY_SHOME => ch = ctrl('Y'),
            KEY_SPREVIOUS => ch = ctrl('U'),
            KEY_SEND => ch = ctrl('B'),
            KEY_SNEXT => ch = ctrl('N'),
            0x146 => ch = ctrl('K'),
            0x145 => ch = ctrl('J'),
            KEY_EOL => ch = ctrl('B'),
            _ => {}
        }

        break;
    }

    input::set_raw_mode(false);
    input::set_raw_mode(true);

    ch & 0x7F
}

/// ctrl(c): return the control character for c.
#[inline]
fn ctrl(c: char) -> c_int {
    (c as u8 & 0x1f) as c_int
}

/// ctrl_upcase(c): CTRL(toupper(c)).
#[inline]
fn ctrl_upcase(c: c_int) -> c_int {
    let up = (c as u8).to_ascii_uppercase();
    ctrl(up as char)
}

// -------------------------------------------------------------------------
// Load average and checkout timer
// -------------------------------------------------------------------------

unsafe extern "C" {
    fn getloadavg(loadavg: *mut f64, nelem: c_int) -> c_int;
}

/// md_loadav:
/// Fill `avg` (3 doubles) with the 1/5/15 minute load averages.
unsafe fn md_loadav(avg: *mut f64) {
    if avg.is_null() {
        return;
    }
    let mut a = [0.0f64; 3];
    if getloadavg(a.as_mut_ptr(), 3) < 0 {
        a = [0.0; 3];
    }
    for i in 0..3 {
        *avg.add(i) = a[i];
    }
}

/// md_start_checkout_timer:
/// Start the SIGALRM-based checkout timer.
///
/// The original C implementation wired SIGALRM to the `checkout()` handler,
/// which lived in mach_dep.c under `#ifdef CHECKTIME`.  CHECKTIME is not
/// enabled in the standard build, so we only need the exported symbol; the
/// alarm is not armed.
unsafe fn md_start_checkout_timer(_time: c_int) {
    // CHECKTIME is disabled in the standard build; keep SIGALRM at its
    // default disposition so no reference to the removed `checkout()` is
    // emitted.
    #[cfg(unix)]
    {
        libc::signal(libc::SIGALRM, libc::SIG_DFL);
    }
}

/// md_stop_checkout_timer:
/// Disable the SIGALRM checkout timer.
unsafe fn md_stop_checkout_timer() {
    #[cfg(unix)]
    {
        libc::signal(libc::SIGALRM, libc::SIG_IGN);
    }
}

// -------------------------------------------------------------------------
// Job-control signal helpers
// -------------------------------------------------------------------------

/// md_tstphold:
/// Hold (ignore) SIGTSTP so the process can't be suspended.
#[no_mangle]
pub unsafe extern "C" fn md_tstphold() {
    #[cfg(unix)]
    {
        libc::signal(libc::SIGTSTP, libc::SIG_IGN);
    }
}

/// md_tstpresume:
/// Restore the SIGTSTP handler to the game's tstp() function.
#[no_mangle]
pub unsafe extern "C" fn md_tstpresume() {
    #[cfg(unix)]
    {
        libc::signal(libc::SIGTSTP, tstp as libc::sighandler_t);
    }
}

/// md_tstpsignal:
/// Send SIGTSTP to the process group to actually suspend.
#[no_mangle]
pub unsafe extern "C" fn md_tstpsignal() {
    #[cfg(unix)]
    {
        libc::kill(0, libc::SIGTSTP);
    }
}
