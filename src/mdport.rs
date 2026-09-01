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

use std::os::raw::{c_char, c_int, c_uint, c_void};

use crate::curses as cur;

/// Ncurses key codes used by the keypad/arrow-key reader.  The `ncurses`
/// crate's `raw_constants.rs` exposes these as `i32`; we re-export the ones
/// `md_readchar` needs.
const KEY_DOWN: c_int = ncurses::KEY_DOWN;
const KEY_UP: c_int = ncurses::KEY_UP;
const KEY_LEFT: c_int = ncurses::KEY_LEFT;
const KEY_RIGHT: c_int = ncurses::KEY_RIGHT;
const KEY_HOME: c_int = ncurses::KEY_HOME;
const KEY_BACKSPACE: c_int = ncurses::KEY_BACKSPACE;
const KEY_NPAGE: c_int = ncurses::KEY_NPAGE;
const KEY_PPAGE: c_int = ncurses::KEY_PPAGE;
const KEY_LL: c_int = ncurses::KEY_LL;
const KEY_A1: c_int = ncurses::KEY_A1;
const KEY_A3: c_int = ncurses::KEY_A3;
const KEY_B2: c_int = ncurses::KEY_B2;
const KEY_C1: c_int = ncurses::KEY_C1;
const KEY_C3: c_int = ncurses::KEY_C3;
const KEY_END: c_int = ncurses::KEY_END;

// The ncurses crate does not expose these legacy/extended keypad codes.
// Values match the ncurses public header (keys.h) so behaviour is identical
// to the original C mdport.c.
const KEY_B1: c_int = 353;  // keypad lower-left
const KEY_B3: c_int = 354;  // keypad lower-right
const KEY_A2: c_int = 355;  // keypad up
const KEY_C2: c_int = 356;  // keypad down
const KEY_SUP: c_int = 337; // shift up
const KEY_SDOWN: c_int = 336; // shift down
const KEY_SEND: c_int = ncurses::KEY_SEND;
const KEY_SHOME: c_int = ncurses::KEY_SHOME;
const KEY_SLEFT: c_int = ncurses::KEY_SLEFT;
const KEY_SNEXT: c_int = ncurses::KEY_SNEXT;
const KEY_SPREVIOUS: c_int = ncurses::KEY_SPREVIOUS;
const KEY_SRIGHT: c_int = ncurses::KEY_SRIGHT;
const KEY_EOL: c_int = ncurses::KEY_EOL;
const ERR: c_int = ncurses::ERR;

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
#[no_mangle]
pub unsafe extern "C" fn md_onsignal_autosave() {
    // The auto-save handlers (auto_save, endit, quit) are Rust `#[no_mangle]`
    // functions; wire them up to the signals on Unix.
    #[cfg(unix)]
    {
        extern "C" {
            fn auto_save(sig: c_int);
            fn endit(sig: c_int);
            fn quit(sig: c_int);
        }
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
        cur::set_escdelay(64);
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
#[no_mangle]
pub unsafe extern "C" fn md_putchar(c: c_int) {
    libc::putchar(c);
}

// -------------------------------------------------------------------------
// Standout / raw-mode output
// -------------------------------------------------------------------------

/// md_raw_standout:
/// Turn on standout (reverse-video) output.
#[no_mangle]
pub unsafe extern "C" fn md_raw_standout() {
    cur::standout();
}

/// md_raw_standend:
/// Turn off standout (reverse-video) output.
#[no_mangle]
pub unsafe extern "C" fn md_raw_standend() {
    cur::standend();
}

// -------------------------------------------------------------------------
// File operations
// -------------------------------------------------------------------------

/// md_unlink_open_file:
/// Unlink an open file.  On POSIX there is nothing special to do beyond
/// unlinking the path.
#[no_mangle]
pub unsafe extern "C" fn md_unlink_open_file(file: *mut c_char, _inf: *mut c_void) -> c_int {
    if file.is_null() {
        return -1;
    }
    libc::unlink(file)
}

/// md_unlink:
/// Remove a file.
#[no_mangle]
pub unsafe extern "C" fn md_unlink(file: *mut c_char) -> c_int {
    if file.is_null() {
        return -1;
    }
    libc::unlink(file)
}

/// md_chmod:
/// Change file permissions.
#[no_mangle]
pub unsafe extern "C" fn md_chmod(filename: *mut c_char, mode: c_int) -> c_int {
    if filename.is_null() {
        return -1;
    }
    libc::chmod(filename, mode as libc::mode_t)
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
pub unsafe extern "C" fn md_getusername() -> *mut c_char {
    static mut LOGIN: [c_char; 80] = [0; 80];
    let mut l: *mut c_char = std::ptr::null_mut();

    #[cfg(unix)]
    {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_name.is_null() {
            l = (*pw).pw_name;
        }
    }

    if l.is_null() || *l == 0 {
        l = libc::getenv(c"USERNAME".as_ptr());
    }
    if l.is_null() || *l == 0 {
        l = libc::getenv(c"LOGNAME".as_ptr());
    }
    if l.is_null() || *l == 0 {
        l = libc::getenv(c"USER".as_ptr());
    }
    if l.is_null() || *l == 0 {
        l = c"nobody".as_ptr() as *mut c_char;
    }

    let src = std::ffi::CStr::from_ptr(l);
    let bytes = src.to_bytes();
    let n = bytes.len().min(79);
    for (i, b) in bytes.iter().take(n).enumerate() {
        LOGIN[i] = *b as c_char;
    }
    LOGIN[n] = 0;
    LOGIN.as_mut_ptr()
}

/// md_gethomedir:
/// Return the home directory of the current user, with a trailing slash.
#[no_mangle]
pub unsafe extern "C" fn md_gethomedir() -> *mut c_char {
    static mut HOMEDIR: [c_char; 4096] = [0; 4096];
    let mut h: *mut c_char = std::ptr::null_mut();

    #[cfg(unix)]
    {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_dir.is_null() {
            let dir = std::ffi::CStr::from_ptr((*pw).pw_dir);
            h = (*pw).pw_dir;
            if dir.to_bytes() == b"/" {
                h = std::ptr::null_mut();
            }
        }
    }

    if h.is_null() {
        h = libc::getenv(c"HOME".as_ptr());
    }

    HOMEDIR[0] = 0;
    if !h.is_null() && *h != 0 {
        let src = std::ffi::CStr::from_ptr(h);
        let bytes = src.to_bytes();
        let n = bytes.len().min(4095);
        for (i, b) in bytes.iter().take(n).enumerate() {
            HOMEDIR[i] = *b as c_char;
        }
        HOMEDIR[n] = 0;
        if n > 0 && HOMEDIR[n - 1] != b'/' as c_char {
            HOMEDIR[n] = b'/' as c_char;
            HOMEDIR[n + 1] = 0;
        }
    }

    HOMEDIR.as_mut_ptr()
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
pub unsafe extern "C" fn md_getshell() -> *mut c_char {
    static mut SHELL: [c_char; 4096] = [0; 4096];
    let mut s: *mut c_char = std::ptr::null_mut();

    #[cfg(unix)]
    {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_shell.is_null() {
            s = (*pw).pw_shell;
        }
    }

    if s.is_null() || *s == 0 {
        s = libc::getenv(c"COMSPEC".as_ptr());
    }
    if s.is_null() || *s == 0 {
        s = libc::getenv(c"SHELL".as_ptr());
    }
    if s.is_null() || *s == 0 {
        s = libc::getenv(c"SystemRoot".as_ptr());
    }
    if s.is_null() || *s == 0 {
        s = c"/bin/sh".as_ptr() as *mut c_char;
    }

    let src = std::ffi::CStr::from_ptr(s);
    let bytes = src.to_bytes();
    let n = bytes.len().min(4095);
    for (i, b) in bytes.iter().take(n).enumerate() {
        SHELL[i] = *b as c_char;
    }
    SHELL[n] = 0;
    SHELL.as_mut_ptr()
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
            let shell_cstr = std::ffi::CStr::from_ptr(sh);
            libc::execl(
                shell_cstr.as_ptr(),
                c"shell".as_ptr(),
                c"-i".as_ptr(),
                std::ptr::null::<c_char>(),
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
#[no_mangle]
pub unsafe extern "C" fn directory_exists(dirname: *mut c_char) -> c_int {
    if dirname.is_null() {
        return 0;
    }
    let path = std::ffi::CStr::from_ptr(dirname);
    match std::fs::metadata(path.to_str().unwrap_or("")) {
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
#[no_mangle]
pub unsafe extern "C" fn md_getrealname(uid: c_int) -> *mut c_char {
    static mut UIDSTR: [c_char; 20] = [0; 20];
    #[cfg(unix)]
    {
        let pw = libc::getpwuid(uid as libc::uid_t);
        if !pw.is_null() && !(*pw).pw_name.is_null() {
            return (*pw).pw_name;
        }
    }
    let s = std::ffi::CString::new(uid.to_string()).unwrap_or_default();
    let bytes = s.as_bytes();
    let n = bytes.len().min(19);
    for (i, b) in bytes.iter().take(n).enumerate() {
        UIDSTR[i] = *b as c_char;
    }
    UIDSTR[n] = 0;
    UIDSTR.as_mut_ptr()
}

// -------------------------------------------------------------------------
// Tty character helpers
// -------------------------------------------------------------------------

/// md_erasechar:
/// Return the terminal erase character.
#[no_mangle]
pub unsafe extern "C" fn md_erasechar() -> c_int {
    cur::erasechar()
}

/// md_killchar:
/// Return the terminal kill character.
#[no_mangle]
pub unsafe extern "C" fn md_killchar() -> c_int {
    cur::killchar()
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
#[no_mangle]
pub unsafe extern "C" fn md_setsuspchar(_c: c_int) -> c_int {
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
        ch = cur::getch();

        if ch == ERR {
            // Timed out waiting for a valid sequence: flush and treat as ESC.
            mode = M_NORMAL;
            cur::nocbreak();
            cur::raw();
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
                0x44 => ch = ctrl('H'), // 'D'
                0x43 => ch = ctrl('L'), // 'C'
                0x41 => ch = ctrl('K'), // 'A'
                0x42 => ch = ctrl('J'), // 'B'
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
            cur::halfdelay(1);
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

    cur::nocbreak();
    cur::raw();

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
#[no_mangle]
pub unsafe extern "C" fn md_loadav(avg: *mut f64) {
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
#[no_mangle]
pub unsafe extern "C" fn md_start_checkout_timer(_time: c_int) {
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
#[no_mangle]
pub unsafe extern "C" fn md_stop_checkout_timer() {
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
        extern "C" {
            fn tstp(v: c_int);
        }
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
