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
//! implemented in `src/c/mdport.c`. The port targets POSIX (Linux/macOS).
//! The only remaining FFI is genuine OS interop (`libc` signal/termios/passwd
//! calls); all hands own their data with Rust types.

use crate::startup::tstp;
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

// -------------------------------------------------------------------------
// Signal handling
// -------------------------------------------------------------------------

/// md_onsignal_default:
/// Restore default signal disposition for common termination signals.
pub unsafe fn md_onsignal_default() {
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
pub unsafe fn md_onsignal_exit() {
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

/// md_ignoreallsignals:
/// Ignore all signals.
pub unsafe fn md_ignoreallsignals() {
    // libc::NSIG is not exposed by the Rust libc crate; 32 matches the
    // `#ifndef NSIG  #define NSIG 32` fallback in the original mdport.c.
    for sig in 0..32 {
        libc::signal(sig, libc::SIG_IGN);
    }
}

/// md_init:
/// Perform machine-dependent startup initialization.
pub unsafe fn md_init() {
    #[cfg(unix)]
    {
        // ESCDELAY is a curses global; the ncurses crate exposes set_escdelay().
        input::set_escape_delay(64);
    }
    md_onsignal_exit();
}

/// md_hasclreol:
/// Return true if the terminal supports clear-to-end-of-line.
pub unsafe fn md_hasclreol() -> i32 {
    // The ncurses crate doesn't expose clr_eol/CE directly.  Assume the
    // terminal supports it (all common terminals do).
    1
}

// -------------------------------------------------------------------------
// Standout / raw-mode output
// -------------------------------------------------------------------------

/// md_raw_standout:
/// Turn on standout (reverse-video) output.
pub unsafe fn md_raw_standout() {
    output::set_standout(true);
}

/// md_raw_standend:
/// Turn off standout (reverse-video) output.
pub unsafe fn md_raw_standend() {
    output::set_standout(false);
}

// -------------------------------------------------------------------------
// File operations
// -------------------------------------------------------------------------

/// md_unlink_open_file:
/// Unlink an open file.  On POSIX there is nothing special to do beyond
/// unlinking the path, so the legacy `FILE*` argument is gone entirely.
pub unsafe fn md_unlink_open_file(file: &str) -> i32 {
    match std::fs::remove_file(file) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// md_unlink:
/// Remove a file.
pub unsafe fn md_unlink(file: &str) -> i32 {
    match std::fs::remove_file(file) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// md_chmod:
/// Change file permissions.
pub unsafe fn md_chmod(filename: &str, mode: i32) -> i32 {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::set_permissions(filename, std::fs::Permissions::from_mode(mode as u32)) {
        Ok(()) => 0,
        Err(_) => -1,
    }
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
pub unsafe fn md_normaluser() {
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
            eprintln!("Could not drop setgid privileges.  Aborting.");
            std::process::exit(1);
        }

        // Drop user privileges (one call, R/E/S all set to real uid).
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let uerr = libc::setresuid((-1i32) as libc::uid_t, realuid, realuid) != 0;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let uerr = libc::setreuid(realuid, realuid) != 0;
        if uerr {
            eprintln!("Could not drop setuid privileges.  Aborting.");
            std::process::exit(1);
        }
    }
}

/// md_getuid:
/// Return the real user id.
pub unsafe fn md_getuid() -> u32 {
    #[cfg(unix)]
    {
        libc::getuid() as u32
    }
    #[cfg(not(unix))]
    {
        42
    }
}

/// md_getpid:
/// Return the process id.
pub unsafe fn md_getpid() -> i32 {
    #[cfg(unix)]
    {
        std::process::id() as i32
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
pub unsafe fn md_sleep(s: i32) {
    #[cfg(unix)]
    {
        std::thread::sleep(std::time::Duration::from_secs(s as u64));
    }
}

/// md_getshell:
/// Return the user's login shell.
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
pub unsafe fn md_shellescape() -> i32 {
    #[cfg(unix)]
    {
        let sh = md_getshell();
        let mut pid = libc::fork();
        while pid < 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            pid = libc::fork();
        }

        let mut ret_status: i32 = 0;

        if pid == 0 {
            // Shell process: drop privileges then exec the shell.
            md_normaluser();
            let shell_c = std::ffi::CString::new(sh.as_str())
                .unwrap_or_else(|_| std::ffi::CString::new("/bin/sh").unwrap());
            libc::execl(
                shell_c.as_ptr(),
                c"shell".as_ptr(),
                c"-i".as_ptr(),
                std::ptr::null::<libc::c_char>(),
            );
            eprintln!("No shelly");
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

// -------------------------------------------------------------------------
// Tty character helpers
// -------------------------------------------------------------------------

/// md_dsuspchar:
/// Return the terminal delete-suspend character.
pub unsafe fn md_dsuspchar() -> i32 {
    // No portable POSIX VDSUSP; use 0 (which the caller treats as "disabled").
    0
}

/// md_setdsuspchar:
/// Set the terminal delete-suspend character.
pub unsafe fn md_setdsuspchar(_c: i32) -> i32 {
    0
}

/// md_suspchar:
/// Return the terminal suspend character.
pub unsafe fn md_suspchar() -> i32 {
    #[cfg(unix)]
    {
        let mut attr = std::mem::zeroed::<libc::termios>();
        // STDIN_FILENO === 0 on POSIX.
        if libc::tcgetattr(0, &mut attr) == 0 {
            return attr.c_cc[libc::VSUSP] as i32;
        }
        0
    }
    #[cfg(not(unix))]
    {
        0
    }
}

// -------------------------------------------------------------------------
// Job-control signal helpers
// -------------------------------------------------------------------------

/// md_tstphold:
/// Hold (ignore) SIGTSTP so the process can't be suspended.
pub unsafe fn md_tstphold() {
    #[cfg(unix)]
    {
        libc::signal(libc::SIGTSTP, libc::SIG_IGN);
    }
}

/// md_tstpresume:
/// Restore the SIGTSTP handler to the game's tstp() function.
pub unsafe fn md_tstpresume() {
    #[cfg(unix)]
    {
        libc::signal(libc::SIGTSTP, tstp as libc::sighandler_t);
    }
}

/// md_tstpsignal:
/// Send SIGTSTP to the process group to actually suspend.
pub unsafe fn md_tstpsignal() {
    #[cfg(unix)]
    {
        libc::kill(0, libc::SIGTSTP);
    }
}
