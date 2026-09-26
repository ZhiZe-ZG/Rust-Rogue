//! Game save, restore, and shell-escape handling.
//!
//! Ported from `src/c/save.c` to Rust.
use crate::machdep::{resetltchars, setup};
use crate::mdport::{
    md_chmod, md_getpid, md_ignoreallsignals, md_tstphold, md_tstpresume, md_unlink,
    md_unlink_open_file,
};
use crate::options::read_line;
use crate::rnd::set_seed;
use crate::startup::playit;
use crate::state::{rs_restore_file, rs_save_file};
use crate::ui::input::{self, readchar};
use crate::ui::output::{self, msg_str};
use crate::ui::runtime;
use crate::ui::Window;
use glam::IVec2;
use std::os::raw::{c_int, c_uchar};

use crate::ffi::{access, exit, fclose, fflush, fopen, fread, fwrite, putchar, CFile};

const ESCAPE: c_int = 27;

// The version banner written as the header of saved games (was `char version[]`).
const VERSION: &[u8] = b"rogue (rogueforge) 09/05/07\0";

unsafe extern "C" {
    static mut mpos: c_int;
    static mut wizard: c_int;
    static mut master_mode_enabled: c_uchar;
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

/// Checks the restored player state and reports whether the saved game is already dead.
unsafe fn restore_player_dead() -> bool {
    crate::game::PLAYER.stats().hit_points <= 0
}

/// Implements the interactive save command flow and then delegates the actual write to save_file.
#[no_mangle]
pub unsafe extern "C" fn save_game() {
    let mut savef: *mut CFile;
    let mut c: c_int;
    let mut buf = crate::globals::file_name();

    mpos = 0;

    'over: loop {
        if !crate::globals::file_name().is_empty() {
            loop {
                msg_str(&format!("save file ({})? ", crate::globals::file_name()));
                c = readchar();
                mpos = 0;
                if c == ESCAPE {
                    msg_str("");
                    return;
                }
                if c == 'n' as c_int || c == 'N' as c_int || c == 'y' as c_int || c == 'Y' as c_int
                {
                    break;
                }
                msg_str("please answer Y or N");
            }

            if c == 'y' as c_int || c == 'Y' as c_int {
                output::write_text("Yes\n");
                output::refresh();
                buf = crate::globals::file_name();
            } else {
                buf = String::new();
            }
        } else {
            buf = String::new();
        }

        loop {
            if buf.is_empty() {
                mpos = 0;
                msg_str("file name: ");
                match read_line("", Window::Stdscr) {
                    None => {
                        msg_str("");
                        return;
                    }
                    Some(text) => buf = text,
                }
                mpos = 0;
            }

            let buf_bytes = crate::ffi::to_c_bytes(&buf);
            if access(buf_bytes.as_ptr(), 0) == 0 {
                loop {
                    msg_str("File exists.  Do you wish to overwrite it?");
                    mpos = 0;
                    c = readchar();
                    if c == ESCAPE {
                        msg_str("");
                        return;
                    }
                    if c == 'y' as c_int || c == 'Y' as c_int {
                        break;
                    }
                    if c == 'n' as c_int || c == 'N' as c_int {
                        continue 'over;
                    }
                    msg_str("Please answer Y or N");
                }
                msg_str(&format!("file name: {}", buf));
                md_unlink(&buf);
            }

            crate::globals::set_file_name(buf.clone());
            let name_bytes = crate::ffi::to_c_bytes(&buf);
            savef = fopen(name_bytes.as_ptr(), b"w\0".as_ptr());
            if !savef.is_null() {
                save_file(savef);
            }

            msg_str(&format!("error {}", *errno_location()));
            buf = String::new();
        }
    }
}

/// Writes the save-file header and hands off the actual save payload to the existing C helpers.
#[no_mangle]
pub unsafe extern "C" fn save_file(savef: *mut CFile) {
    let mut buf = [0u8; 80];
    let size = crate::ui::screen_size();
    let lines = size.y;
    let cols = size.x;
    let header = format!("{} x {}\n", lines, cols);

    runtime::move_physical_cursor(IVec2::new(cols - 1, 0), IVec2::new(0, lines - 1));
    putchar('\n' as c_int);
    runtime::shutdown();
    resetltchars();
    md_chmod(&crate::globals::file_name(), 0o400);

    fwrite(VERSION.as_ptr(), 1, VERSION.len(), savef);

    buf[..header.len()].copy_from_slice(header.as_bytes());
    fwrite(buf.as_ptr(), 1, buf.len(), savef);

    rs_save_file(savef.cast());
    fflush(savef);
    fclose(savef);
    exit(0)
}

/// Restores a saved game from disk, rebuilds runtime state, and resumes the main game loop.
#[no_mangle]
pub unsafe extern "C" fn restore(file: *mut std::os::raw::c_char) -> c_uchar {
    let mut in_buf = [0u8; 1024];
    let mut lines: c_int = 0;
    let mut cols: c_int = 0;

    // The caller passes a file argument (typically "-r"); interpret it as Rust text.
    let mut file_name = if file.is_null() {
        String::new()
    } else {
        let mut len = 0usize;
        let p = file as *const u8;
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf8_lossy(std::slice::from_raw_parts(p, len)).into_owned()
    };

    if file_name == "-r" {
        file_name = crate::globals::file_name();
    }

    md_tstphold();

    let file_bytes = crate::ffi::to_c_bytes(&file_name);
    let inf = fopen(file_bytes.as_ptr(), b"r\0".as_ptr());
    if inf.is_null() {
        msg_str(&format!("{}: cannot open", file_name));
        return 0;
    }

    let _ = fflush(std::ptr::null_mut());
    let _ = fread(in_buf.as_mut_ptr(), 1, VERSION.len(), inf);
    if in_buf[..VERSION.len()] != *VERSION {
        msg_str("Sorry, saved game is out of date.\n");
        return 0;
    }

    let _ = fread(in_buf.as_mut_ptr(), 1, 80, inf);
    let header = String::from_utf8_lossy(&in_buf[..80]);
    let mut parts = header.split('x');
    if let (Some(a), Some(b)) = (parts.next(), parts.next()) {
        lines = a.trim().parse().unwrap_or(0);
        cols = b.trim().parse().unwrap_or(0);
    }

    if runtime::is_shutdown() {
        runtime::initialize();
    }
    input::set_keypad(Window::Stdscr, true);

    let screen = crate::ui::screen_size();
    if lines > screen.y {
        runtime::shutdown();
        msg_str(&format!(
            "Sorry, original game was played on a screen with {} lines.\n",
            lines
        ));
        msg_str(&format!(
            "Current screen only has {} lines. Unable to restore game\n",
            screen.y
        ));
        return 0;
    }
    if cols > screen.x {
        runtime::shutdown();
        msg_str(&format!(
            "Sorry, original game was played on a screen with {} columns.\n",
            cols
        ));
        msg_str(&format!(
            "Current screen only has {} columns. Unable to restore game\n",
            screen.x
        ));
        return 0;
    }

    setup();
    let _ = rs_restore_file(inf.cast());

    if (master_mode_enabled == 0 || wizard == 0)
        && md_unlink_open_file(&file_name, inf.cast()) < 0
    {
        msg_str("Cannot unlink file\n");
        return 0;
    }

    mpos = 0;
    output::set_clear_on_refresh(Window::Stdscr, true);

    if restore_player_dead() {
        runtime::shutdown();
        msg_str("\n\"He's dead, Jim\"\n");
        return 0;
    }

    md_tstpresume();
    crate::globals::set_file_name(file_name.clone());
    output::set_clear_on_refresh(Window::Curscr, true);
    set_seed(md_getpid());
    msg_str(&format!("file name: {}", file_name));
    playit();
    0
}

/// Handles signal-triggered autosave by reopening the current save file and delegating to save_file.
#[no_mangle]
pub unsafe extern "C" fn auto_save(sig: c_int) {
    let _ = sig;
    let mut savef: *mut CFile;

    md_ignoreallsignals();
    let file_name = crate::globals::file_name();
    if !file_name.is_empty() {
        let name_bytes = crate::ffi::to_c_bytes(&file_name);
        savef = fopen(name_bytes.as_ptr(), b"w\0".as_ptr());
        if !savef.is_null() {
            save_file(savef);
        } else if md_unlink_open_file(&file_name, savef.cast()) >= 0 {
            savef = fopen(name_bytes.as_ptr(), b"w\0".as_ptr());
            if !savef.is_null() {
                save_file(savef);
            }
        }
    }
    exit(0)
}
