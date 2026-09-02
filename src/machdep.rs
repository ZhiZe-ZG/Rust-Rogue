//! Machine-dependent routines, ported from `src/c/mach_dep.c`.
//!
//! Various installation dependent routines.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::curses as cur;
use crate::globals::{fruit, got_ltc, orig_dsusp, prbuf, scoreboard, whoami};

const MAXSTR: usize = 1024;
const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

// Build-time feature flags mirroring config.h for the standard build.
const SCOREFILE_ENABLED: bool = true; // config.h: #define SCOREFILE "rogue.scr"
const LOCKFILE_ENABLED: bool = true;  // config.h: #define LOCKFILE "rogue.lck"
const CHECKTIME: bool = false;        // config.h: /* #undef CHECKTIME */
const DUMP: bool = false;             // not set in the standard build

const SCOREFILE: &[u8] = b"rogue.scr";
const LOCKFILE: &[u8] = b"rogue.lck";
const ENOENT: c_int = 2;

/// `FILE *lfd` from mach_dep.c -- handle of the scoreboard lock file.
static mut LFD: *mut crate::score::CFile = ptr::null_mut();

#[cfg(target_os = "macos")]
unsafe extern "C" {
    static mut __stdinp: *mut c_void;
    static mut __stderrp: *mut c_void;
}

#[cfg(not(target_os = "macos"))]
unsafe extern "C" {
    static mut stdin: *mut c_void;
    static mut stderr: *mut c_void;
}

#[inline]
unsafe fn c_stdin() -> *mut c_void {
    #[cfg(target_os = "macos")]
    {
        __stdinp
    }
    #[cfg(not(target_os = "macos"))]
    {
        stdin
    }
}

#[inline]
unsafe fn c_stderr() -> *mut c_void {
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
    static mut stdscr: *mut c_void;

    fn fopen(path: *const c_char, mode: *const c_char) -> *mut crate::score::CFile;
    fn fclose(stream: *mut crate::score::CFile) -> c_int;
    fn fgets(buf: *mut c_char, n: c_int, stream: *mut c_void) -> *mut c_char;
    fn fflush(stream: *mut c_void) -> c_int;
    fn fprintf(stream: *mut c_void, fmt: *const c_char, ...) -> c_int;
    fn printf(fmt: *const c_char, ...) -> c_int;
    fn rewind(stream: *mut crate::score::CFile);
    fn strerror(errnum: c_int) -> *const c_char;

    // Machine-dependent helpers now implemented in src/rust/src/mdport.rs.
    fn md_chmod(filename: *mut c_char, mode: c_int) -> c_int;
    fn md_dsuspchar() -> c_int;
    fn md_getuid() -> c_uint;
    fn md_onsignal_default();
    fn md_setdsuspchar(c: c_int) -> c_int;
    fn md_sleep(s: c_int);
    fn md_suspchar() -> c_int;
    fn md_unlink(file: *mut c_char) -> c_int;
}

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
unsafe fn lockfile_mtime(path: *const c_char) -> Option<i64> {
    let path_str = CStr::from_ptr(path).to_string_lossy();
    let md = std::fs::metadata(path_str.as_ref()).ok()?;
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
    let _ = (whoami.as_ptr(), fruit.as_ptr());
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

    let scorefile = CString::new(SCOREFILE).unwrap();
    let scorefile_ptr = scorefile.as_ptr() as *mut c_char;

    scoreboard = fopen(scorefile_ptr, c"r+".as_ptr());

    if scoreboard.is_null() && *errno_location() == ENOENT {
        scoreboard = fopen(scorefile_ptr, c"w+".as_ptr());
        md_chmod(scorefile_ptr, 0o664);
    }

    if scoreboard.is_null() {
        fprintf(
            c_stderr(),
            c"Could not open %s for writing: %s\n".as_ptr(),
            scorefile_ptr,
            strerror(*errno_location()),
        );
        fflush(c_stderr());
    }
}

/// setup:
/// Get starting setup for all games.
///
/// Uses globals: stdscr (curses).
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

    cur::raw();                                /* Raw mode */
    cur::noecho();                             /* Echo off */
    cur::keypad(stdscr, TRUE);
    getltchars();                              /* get the local tty chars */
}

/// getltchars:
/// Get the local tty chars for later use.
///
/// Uses globals: got_ltc, orig_dsusp.
#[no_mangle]
pub unsafe extern "C" fn getltchars() {
    got_ltc = TRUE;
    orig_dsusp = md_dsuspchar();
    md_setdsuspchar(md_suspchar());
}

/// resetltchars:
/// Reset the local tty chars to original values.
///
/// Uses globals: got_ltc, orig_dsusp.
#[no_mangle]
pub unsafe extern "C" fn resetltchars() {
    if got_ltc != 0 {
        md_setdsuspchar(orig_dsusp);
    }
}

/// playltchars:
/// Set local tty chars to the values we use when playing.
///
/// Uses globals: got_ltc.
#[no_mangle]
pub unsafe extern "C" fn playltchars() {
    if got_ltc != 0 {
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
#[no_mangle]
pub unsafe extern "C" fn is_symlink(sp: *mut c_char) -> c_uchar {
    if sp.is_null() {
        return FALSE;
    }
    let path = CStr::from_ptr(sp).to_string_lossy();
    match std::fs::symlink_metadata(path.as_ref()) {
        Ok(md) => {
            // Original C: ((sbuf2.st_mode & S_IFMT) != S_IFREG)
            if md.file_type().is_file() { FALSE } else { TRUE }
        }
        Err(_) => FALSE,
    }
}

/// lock_sc:
/// Lock the score file.  If it takes too long, ask the user if they
/// care to wait.  Return TRUE if the lock is successful.
///
/// Uses globals: lfd (static), prbuf.
#[no_mangle]
pub unsafe extern "C" fn lock_sc() -> c_int {
    if !SCOREFILE_ENABLED || !LOCKFILE_ENABLED {
        return TRUE as c_int;
    }

    let lockfile = CString::new(LOCKFILE).unwrap();
    let lockfile_ptr = lockfile.as_ptr() as *mut c_char;

    'over: loop {
        LFD = fopen(lockfile_ptr, c"w+".as_ptr());
        if !LFD.is_null() {
            return TRUE as c_int;
        }

        for _ in 0..5 {
            md_sleep(1);
            LFD = fopen(lockfile_ptr, c"w+".as_ptr());
            if !LFD.is_null() {
                return TRUE as c_int;
            }
        }

        match lockfile_mtime(lockfile_ptr) {
            None => {
                // stat() failed -- the lock file is gone; try again.
                LFD = fopen(lockfile_ptr, c"w+".as_ptr());
                return TRUE as c_int;
            }
            Some(mtime) => {
                if now_secs() - mtime > 10 {
                    if md_unlink(lockfile_ptr) < 0 {
                        return FALSE as c_int;
                    }
                    continue 'over;
                }

                printf(c"The score file is very busy.  Do you want to wait longer\n".as_ptr());
                printf(c"for it to become free so your score can get posted?\n".as_ptr());
                printf(c"If so, type \"y\"\n".as_ptr());
                let _ = fgets(prbuf.as_mut_ptr(), MAXSTR as c_int, c_stdin());
                if prbuf[0] == 'y' as c_char {
                    loop {
                        LFD = fopen(lockfile_ptr, c"w+".as_ptr());
                        if !LFD.is_null() {
                            return TRUE as c_int;
                        }
                        if let Some(mtime2) = lockfile_mtime(lockfile_ptr) {
                            if now_secs() - mtime2 > 10 {
                                if md_unlink(lockfile_ptr) < 0 {
                                    return FALSE as c_int;
                                }
                            }
                        } else {
                            LFD = fopen(lockfile_ptr, c"w+".as_ptr());
                            return TRUE as c_int;
                        }
                        md_sleep(1);
                    }
                }
                return FALSE as c_int;
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
    let lockfile = CString::new(LOCKFILE).unwrap();
    md_unlink(lockfile.as_ptr() as *mut c_char);
}

/// flush_type:
/// Flush typeahead for traps, etc.
#[no_mangle]
pub unsafe extern "C" fn flush_type() {
    cur::flushinp();
}