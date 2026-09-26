//! Machine-dependent routines, ported from `src/c/mach_dep.c`.
//!
//! Various installation dependent routines.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use std::fs::{File, OpenOptions};

use std::time::{SystemTime, UNIX_EPOCH};

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

/// `FILE *lfd` from mach_dep.c -- handle of the scoreboard lock file.
static mut LFD: Option<File> = None;

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
pub unsafe fn init_check() {
    let _ = (crate::globals::whoami(), crate::globals::fruit());
}

/// open_score:
/// Open up the score file for future use.
///
/// Uses globals: scoreboard.
pub unsafe fn open_score() {
    if !SCOREFILE_ENABLED {
        scoreboard = None;
        return;
    }

    if let Some(file) = scoreboard.as_mut() {
        use std::io::Seek;
        let _ = file.seek(std::io::SeekFrom::Start(0));
        return;
    }

    // "r+" then fall back to "w+" when the file does not exist.
    scoreboard = match OpenOptions::new().read(true).write(true).open(SCOREFILE) {
        Ok(file) => Some(file),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                let created = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(SCOREFILE);
                match created {
                    Ok(file) => {
                        md_chmod(SCOREFILE, 0o664);
                        Some(file)
                    }
                    Err(err) => {
                        eprintln!("Could not open {} for writing: error {}", SCOREFILE, err);
                        None
                    }
                }
            } else {
                eprintln!("Could not open {} for writing: error {}", SCOREFILE, err);
                None
            }
        }
    };
}

/// setup:
/// Get starting setup for all games.
pub unsafe fn setup() {
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
pub unsafe fn getltchars() {
    got_ltc = 1;
    orig_dsusp = md_dsuspchar();
    md_setdsuspchar(md_suspchar());
}

/// resetltchars:
/// Reset the local tty chars to original values.
///
/// Uses globals: got_ltc, orig_dsusp.
pub unsafe fn resetltchars() {
    if got_ltc != 0 {
        md_setdsuspchar(orig_dsusp);
    }
}

/// playltchars:
/// Set local tty chars to the values we use when playing.
///
/// Uses globals: got_ltc.
pub unsafe fn playltchars() {
    if got_ltc != 0 {
        md_setdsuspchar(md_suspchar());
    }
}

/// start_score:
/// Start the scoring sequence.
///
/// The CHECKTIME feature is not enabled in the standard build, so
/// md_stop_checkout_timer() is never needed.
pub unsafe fn start_score() {
    // CHECKTIME is not defined in the standard build.
}

/// is_symlink:
/// See if the file is not a regular file (i.e. a symbolic link or
/// special file).
#[allow(dead_code)]
unsafe fn is_symlink(path: &str) -> u8 {
    match std::fs::symlink_metadata(path) {
        Ok(md) => {
            // Original C: ((sbuf2.st_mode & S_IFMT) != S_IFREG)
            if md.file_type().is_file() {
                false as u8
            } else {
                true as u8
            }
        }
        Err(_) => false as u8,
    }
}

/// lock_sc:
/// Lock the score file.  If it takes too long, ask the user if they
/// care to wait.  Return true as u8 if the lock is successful.
///
/// Uses globals: lfd (static), prbuf.
pub unsafe fn lock_sc() -> i32 {
    if !SCOREFILE_ENABLED || !LOCKFILE_ENABLED {
        return true as u8 as i32;
    }

    let try_open = || File::create(LOCKFILE).ok();

    'over: loop {
        LFD = try_open();
        if LFD.is_some() {
            return true as u8 as i32;
        }

        for _ in 0..5 {
            md_sleep(1);
            LFD = try_open();
            if LFD.is_some() {
                return true as u8 as i32;
            }
        }

        match lockfile_mtime(LOCKFILE) {
            None => {
                // stat() failed -- the lock file is gone; try again.
                LFD = try_open();
                return true as u8 as i32;
            }
            Some(mtime) => {
                if now_secs() - mtime > 10 {
                    if md_unlink(LOCKFILE) < 0 {
                        return false as u8 as i32;
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
                        LFD = try_open();
                        if LFD.is_some() {
                            return true as u8 as i32;
                        }
                        if let Some(mtime2) = lockfile_mtime(LOCKFILE) {
                            if now_secs() - mtime2 > 10 {
                                if md_unlink(LOCKFILE) < 0 {
                                    return false as u8 as i32;
                                }
                            }
                        } else {
                            LFD = try_open();
                            return true as u8 as i32;
                        }
                        md_sleep(1);
                    }
                }
                return false as u8 as i32;
            }
        }
    }
}

/// unlock_sc:
/// Unlock the score file.
///
/// Uses globals: lfd (static).
pub unsafe fn unlock_sc() {
    if !SCOREFILE_ENABLED || !LOCKFILE_ENABLED {
        return;
    }
    LFD = None;
    md_unlink(LOCKFILE);
}

/// flush_type:
/// Flush typeahead for traps, etc.
pub unsafe fn flush_type() {
    input::flush_pending();
}
