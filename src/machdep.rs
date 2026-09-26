//! Machine-dependent routines, ported from `src/c/mach_dep.c`.
//!
//! Various installation dependent routines.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use std::os::raw::{c_int, c_uchar};
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ffi::{fclose, fopen, rewind, CFile};
use crate::globals::{got_ltc, orig_dsusp, scoreboard};
use crate::mdport::{
    md_chmod, md_dsuspchar, md_onsignal_default, md_setdsuspchar, md_sleep, md_suspchar, md_unlink,
};
use crate::ui::input;
use crate::ui::Window;

// Build-time feature flags mirroring config.h for the standard build.
const SCOREFILE_ENABLED: bool = true; // config.h: #define SCOREFILE "rogue.scr"
const LOCKFILE_ENABLED: bool = true; // config.h: #define LOCKFILE "rogue.lck"
const CHECKTIME: bool = false; // config.h: /* #undef CHECKTIME */
const DUMP: bool = false; // not set in the standard build

const SCOREFILE: &str = "rogue.scr";
const LOCKFILE: &str = "rogue.lck";
const ENOENT: c_int = 2;

/// `FILE *lfd` from mach_dep.c -- handle of the scoreboard lock file.
static mut LFD: *mut CFile = ptr::null_mut();

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn __error() -> *mut c_int;
}

#[cfg(not(target_os = "macos"))]
unsafe extern "C" {
    fn __errno_location() -> *mut c_int;
}

#[inline]
unsafe fn errno_location() -> *mut c_int {
    #[cfg(target_os = "macos")]
    {
        __error()
    }
    #[cfg(not(target_os = "macos"))]
    {
        __errno_location()
    }
}

/// Current time in seconds since the Unix epoch.
fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Returns the modification time (in seconds since the epoch) of the file at
/// `path`, or `None` if the file does not exist / cannot be statted.
unsafe fn lockfile_mtime(path: &str) -> Option<i64> {
    let md = std::fs::metadata(path).ok()?;
    let modified = md.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

/// init_check:
/// Check to see if it is proper to play the game now.
///
/// Uses globals: whoami, fruit.
///
/// The MAXLOAD / MAXUSERS features are not enabled in the standard
/// build (config.h leaves both undefined), so this is a no-op.
#[no_mangle]
pub unsafe extern "C" fn init_check() {
    let _ = (crate::globals::whoami(), crate::globals::fruit());
}

/// open_score:
/// Open up the score file for future use.
///
/// Uses globals: scoreboard.
#[no_mangle]
pub unsafe extern "C" fn open_score() {
    if !SCOREFILE_ENABLED {
        scoreboard = ptr::null_mut();
        return;
    }

    if !scoreboard.is_null() {
        rewind(scoreboard);
        return;
    }

    let scorefile_bytes = crate::ffi::to_c_bytes(SCOREFILE);

    scoreboard = fopen(scorefile_bytes.as_ptr(), b"r+\0".as_ptr());

    if scoreboard.is_null() && *errno_location() == ENOENT {
        scoreboard = fopen(scorefile_bytes.as_ptr(), b"w+\0".as_ptr());
        md_chmod(SCOREFILE, 0o664);
    }

    if scoreboard.is_null() {
        eprintln!(
            "Could not open {} for writing: error {}",
            SCOREFILE,
            *errno_location()
        );
    }
}

/// setup:
/// Get starting setup for all games.
#[no_mangle]
pub unsafe extern "C" fn setup() {
    if DUMP {
        // md_onsignal_autosave();
    } else {
        md_onsignal_default();
    }

    if CHECKTIME {
        // md_start_checkout_timer(CHECKTIME * 60);
    }

    input::set_raw_mode(true);
    input::set_echo(false);
    input::set_keypad(Window::Stdscr, true);
    getltchars(); /* get the local tty chars */
}

/// getltchars:
/// Get the local tty chars for later use.
///
/// Uses globals: got_ltc, orig_dsusp.
#[no_mangle]
pub unsafe extern "C" fn getltchars() {
    got_ltc = true;
    orig_dsusp = md_dsuspchar();
    md_setdsuspchar(md_suspchar());
}

/// resetltchars:
/// Reset the local tty chars to original values.
///
/// Uses globals: got_ltc, orig_dsusp.
#[no_mangle]
pub unsafe extern "C" fn resetltchars() {
    if got_ltc {
        md_setdsuspchar(orig_dsusp);
    }
}

/// playltchars:
/// Set local tty chars to the values we use when playing.
///
/// Uses globals: got_ltc.
#[no_mangle]
pub unsafe extern "C" fn playltchars() {
    if got_ltc {
        md_setdsuspchar(md_suspchar());
    }
}

/// start_score:
/// Start the scoring sequence.
///
/// The CHECKTIME feature is not enabled in the standard build, so
/// md_stop_checkout_timer() is never needed.
#[no_mangle]
pub unsafe extern "C" fn start_score() {
    // CHECKTIME is not defined in the standard build.
}

/// is_symlink:
/// See if the file is not a regular file (i.e. a symbolic link or
/// special file).
#[allow(dead_code)]
unsafe fn is_symlink(path: &str) -> c_uchar {
    match std::fs::symlink_metadata(path) {
        Ok(md) => {
            // Original C: ((sbuf2.st_mode & S_IFMT) != S_IFREG)
            if md.file_type().is_file() {
                false as c_uchar
            } else {
                true as c_uchar
            }
        }
        Err(_) => false as c_uchar,
    }
}

/// lock_sc:
/// Lock the score file.  If it takes too long, ask the user if they
/// care to wait.  Return true as c_uchar if the lock is successful.
///
/// Uses globals: lfd (static), prbuf.
#[no_mangle]
pub unsafe extern "C" fn lock_sc() -> c_int {
    if !SCOREFILE_ENABLED || !LOCKFILE_ENABLED {
        return true as c_uchar as c_int;
    }

    let lockfile_bytes = crate::ffi::to_c_bytes(LOCKFILE);

    'over: loop {
        LFD = fopen(lockfile_bytes.as_ptr(), b"w+\0".as_ptr());
        if !LFD.is_null() {
            return true as c_uchar as c_int;
        }

        for _ in 0..5 {
            md_sleep(1);
            LFD = fopen(lockfile_bytes.as_ptr(), b"w+\0".as_ptr());
            if !LFD.is_null() {
                return true as c_uchar as c_int;
            }
        }

        match lockfile_mtime(LOCKFILE) {
            None => {
                // stat() failed -- the lock file is gone; try again.
                LFD = fopen(lockfile_bytes.as_ptr(), b"w+\0".as_ptr());
                return true as c_uchar as c_int;
            }
            Some(mtime) => {
                if now_secs() - mtime > 10 {
                    if md_unlink(LOCKFILE) < 0 {
                        return false as c_uchar as c_int;
                    }
                    continue 'over;
                }

                println!("The score file is very busy.  Do you want to wait longer");
                println!("for it to become free so your score can get posted?");
                println!("If so, type \"y\"");
                let mut answer = String::new();
                let _ = std::io::stdin().read_line(&mut answer);
                if answer.trim_start().starts_with('y') {
                    loop {
                        LFD = fopen(lockfile_bytes.as_ptr(), b"w+\0".as_ptr());
                        if !LFD.is_null() {
                            return true as c_uchar as c_int;
                        }
                        if let Some(mtime2) = lockfile_mtime(LOCKFILE) {
                            if now_secs() - mtime2 > 10 {
                                if md_unlink(LOCKFILE) < 0 {
                                    return false as c_uchar as c_int;
                                }
                            }
                        } else {
                            LFD = fopen(lockfile_bytes.as_ptr(), b"w+\0".as_ptr());
                            return true as c_uchar as c_int;
                        }
                        md_sleep(1);
                    }
                }
                return false as c_uchar as c_int;
            }
        }
    }
}

/// unlock_sc:
/// Unlock the score file.
///
/// Uses globals: lfd (static).
#[no_mangle]
pub unsafe extern "C" fn unlock_sc() {
    if !SCOREFILE_ENABLED || !LOCKFILE_ENABLED {
        return;
    }
    if !LFD.is_null() {
        fclose(LFD);
    }
    LFD = ptr::null_mut();
    md_unlink(LOCKFILE);
}

/// flush_type:
/// Flush typeahead for traps, etc.
#[no_mangle]
pub unsafe extern "C" fn flush_type() {
    input::flush_pending();
}
