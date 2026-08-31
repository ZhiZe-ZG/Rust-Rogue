use crate::rnd::set_seed;
use crate::curses as cur;
use crate::io::msg_str;
use crate::player::{CThing, CThingMonster};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};
use std::ptr;

const MAXSTR: usize = 1024;
const ESCAPE: c_int = 27;
const QUIT: c_int = 1;

#[repr(C)]
pub struct CFile {
    _private: [u8; 0],
}

extern "C" {
    static mut LINES: c_int;
    static mut COLS: c_int;
    static mut mpos: c_int;
    static mut stdscr: *mut CWindow;
    static mut curscr: *mut CWindow;
    static mut hw: *mut CWindow;
    static mut file_name: c_char;
    static mut version: c_char;
    static mut wizard: c_int;
    static mut environ: *mut *mut c_char;
    static mut master_mode_enabled: c_uchar;
    static mut player: CThing;

    fn readchar() -> c_int;
    fn get_str(s: *mut c_char, win: *mut CWindow) -> c_int;
    fn md_unlink(file: *mut c_char) -> c_int;

    fn putchar(c: c_int) -> c_int;
    fn setup();
    fn md_tstphold();
    fn md_tstpresume();
    fn perror(s: *const c_char);
    fn strlen(s: *const c_char) -> usize;
    fn strcmp(a: *const c_char, b: *const c_char) -> c_int;
    fn fread(ptr: *mut u8, size: usize, n: usize, stream: *mut CFile) -> usize;
    fn sscanf(buf: *const c_char, fmt: *const c_char, ...) -> c_int;
    fn rs_restore_file(inf: *mut CFile) -> c_int;
    fn md_getpid() -> c_int;
    fn playit();
    fn resetltchars();
    fn md_chmod(filename: *mut c_char, mode: c_int) -> c_int;
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut CFile;
    fn access(path: *const c_char, mode: c_int) -> c_int;
    fn md_ignoreallsignals();
    fn md_unlink_open_file(file: *mut c_char, inf: *mut CFile) -> c_int;
    fn fwrite(ptr: *const u8, size: usize, nmemb: usize, stream: *mut CFile) -> usize;
    fn strerror(errnum: c_int) -> *const c_char;
    fn rs_save_file(savef: *mut CFile);
    fn fflush(stream: *mut CFile) -> c_int;
    fn fclose(stream: *mut CFile) -> c_int;
    fn exit(status: c_int) -> !;
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

#[repr(C)]
pub struct CWindow {
    _private: [u8; 0],
}

/// Copies a C string into a fixed-size destination buffer and always preserves a trailing NUL.
unsafe fn copy_cstr(dst: *mut c_char, src: *const c_char, max: usize) {
    let mut i = 0usize;
    while i + 1 < max {
        let ch = ptr::read(src.add(i));
        ptr::write(dst.add(i), ch);
        if ch == 0 {
            return;
        }
        i += 1;
    }
    ptr::write(dst.add(max - 1), 0);
}

/// Checks the restored player state and reports whether the saved game is already dead.
unsafe fn restore_player_dead() -> bool {
    (*(std::ptr::addr_of_mut!(player) as *mut CThingMonster)).t_stats.s_hpt <= 0
}

/// Implements the interactive save command flow and then delegates the actual write to save_file.
#[no_mangle]
pub unsafe extern "C" fn save_game() {
    let mut savef: *mut CFile;
    let mut c: c_int;
    let mut buf = [0 as c_char; MAXSTR];
    let file_name_ptr = &raw mut file_name as *mut c_char;

    mpos = 0;

    'over: loop {
        if ptr::read(file_name_ptr) != 0 {
            loop {
                msg_str(&format!(
                    "save file ({})? ",
                    CStr::from_ptr(file_name_ptr).to_string_lossy()
                ));
                c = readchar();
                mpos = 0;
                if c == ESCAPE {
                    msg_str("");
                    return;
                }
                if c == 'n' as c_int || c == 'N' as c_int || c == 'y' as c_int || c == 'Y' as c_int {
                    break;
                }
                msg_str("please answer Y or N");
            }

            if c == 'y' as c_int || c == 'Y' as c_int {
                cur::addstr(c"Yes\n".as_ptr());
                cur::refresh();
                copy_cstr(buf.as_mut_ptr(), file_name_ptr, MAXSTR);
                // Continue to file-open logic using current buffer value.
            } else {
                // Fall through to prompt for a new name.
                buf[0] = 0;
            }
        } else {
            buf[0] = 0;
        }

        loop {
            if buf[0] == 0 {
                mpos = 0;
                msg_str("file name: ");
                if get_str(buf.as_mut_ptr(), stdscr) == QUIT {
                    msg_str("");
                    return;
                }
                mpos = 0;
            }

            if access(buf.as_ptr(), 0) == 0 {
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
                msg_str(&format!(
                    "file name: {}",
                    CStr::from_ptr(buf.as_ptr()).to_string_lossy()
                ));
                md_unlink(file_name_ptr);
            }

            copy_cstr(file_name_ptr, buf.as_ptr(), MAXSTR);
            savef = fopen(file_name_ptr, c"w".as_ptr());
            if !savef.is_null() {
                save_file(savef);
            }

            msg_str(&format!(
                "{}",
                CStr::from_ptr(strerror(*errno_location())).to_string_lossy()
            ));
            buf[0] = 0;
        }
    }
}

/// Writes the save-file header and hands off the actual save payload to the existing C helpers.
#[no_mangle]
pub unsafe extern "C" fn save_file(savef: *mut CFile) {
    let mut buf = [0u8; 80];
    let lines = LINES;
    let cols = COLS;
    let header = format!("{} x {}\n", lines, cols);
    let version_ptr = &raw const version;

    cur::mvcur(0, cols - 1, lines - 1, 0);
    putchar('\n' as c_int);
    cur::endwin();
    resetltchars();
    md_chmod(&raw mut file_name as *mut c_char, 0o400);

    fwrite(
        version_ptr as *const u8,
        1,
        CStr::from_ptr(version_ptr).to_bytes_with_nul().len(),
        savef,
    );

    buf[..header.len()].copy_from_slice(header.as_bytes());
    fwrite(buf.as_ptr(), 1, buf.len(), savef);

    rs_save_file(savef);
    fflush(savef);
    fclose(savef);
    exit(0)
}

/// Restores a saved game from disk, rebuilds runtime state, and resumes the main game loop.
#[no_mangle]
pub unsafe extern "C" fn restore(file: *mut c_char, envp: *mut *mut c_char) -> c_uchar {
    let mut buf = [0 as c_char; MAXSTR];
    let mut lines: c_int = 0;
    let mut cols: c_int = 0;
    let file_name_ptr = &raw mut file_name as *mut c_char;
    let version_ptr = &raw const version;

    let mut file_ptr = file;
    if strcmp(file_ptr, c"-r".as_ptr()) == 0 {
        file_ptr = file_name_ptr;
    }

    md_tstphold();

    let inf = fopen(file_ptr, c"r".as_ptr());
    if inf.is_null() {
        perror(file_ptr);
        return 0;
    }

    let _ = fflush(ptr::null_mut());
    let _ = fread(buf.as_mut_ptr() as *mut u8, 1, strlen(version_ptr) + 1, inf);
    if strcmp(buf.as_ptr(), version_ptr) != 0 {
        msg_str("Sorry, saved game is out of date.\n");
        return 0;
    }

    let _ = fread(buf.as_mut_ptr() as *mut u8, 1, 80, inf);
    let _ = sscanf(buf.as_ptr(), c"%d x %d\n".as_ptr(), &mut lines, &mut cols);

    if stdscr.is_null() {
        cur::initscr();
    }
    cur::keypad(stdscr as *mut CWindow, 1);

    if lines > LINES {
        cur::endwin();
        msg_str(&format!(
            "Sorry, original game was played on a screen with {} lines.\n",
            lines
        ));
        msg_str(&format!(
            "Current screen only has {} lines. Unable to restore game\n",
            LINES
        ));
        return 0;
    }
    if cols > COLS {
        cur::endwin();
        msg_str(&format!(
            "Sorry, original game was played on a screen with {} columns.\n",
            cols
        ));
        msg_str(&format!(
            "Current screen only has {} columns. Unable to restore game\n",
            COLS
        ));
        return 0;
    }

    hw = cur::newwin(LINES, COLS, 0, 0) as *mut CWindow;
    setup();
    let _ = rs_restore_file(inf);

    if (master_mode_enabled == 0 || wizard == 0) && md_unlink_open_file(file_ptr, inf) < 0 {
        msg_str("Cannot unlink file\n");
        return 0;
    }

    mpos = 0;
    cur::clearok(stdscr as *mut CWindow, 1);

    if restore_player_dead() {
        cur::endwin();
        msg_str("\n\"He's dead, Jim\"\n");
        return 0;
    }

    md_tstpresume();
    environ = envp;
    copy_cstr(file_name_ptr, file_ptr, MAXSTR);
    cur::clearok(curscr as *mut CWindow, 1);
    set_seed(md_getpid());
    msg_str(&format!(
        "file name: {}",
        CStr::from_ptr(file_ptr).to_string_lossy()
    ));
    playit();
    0
}

/// Handles signal-triggered autosave by reopening the current save file and delegating to save_file.
#[no_mangle]
pub unsafe extern "C" fn auto_save(sig: c_int) {
    let _ = sig;
    let file_name_ptr = &raw mut file_name as *mut c_char;
    let mode = c"w";
    let mut savef: *mut CFile;

    md_ignoreallsignals();
    if ptr::read(file_name_ptr) != 0
        && ((!{
            savef = fopen(file_name_ptr, mode.as_ptr());
            savef.is_null()
        }) || (md_unlink_open_file(file_name_ptr, savef) >= 0 && !{
            savef = fopen(file_name_ptr, mode.as_ptr());
            savef.is_null()
        }))
    {
        save_file(savef);
    }
    exit(0)
}
