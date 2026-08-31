use crate::rnd::set_seed;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};
use std::ptr;

const MAXSTR: usize = 1024;
const ESCAPE: c_int = 27;
const QUIT: c_int = 1;

#[repr(C)]
pub struct CFile {
    _private: [u8; 0],
}

#[repr(C)]
pub struct CCoord {
    pub x: c_int,
    pub y: c_int,
}

#[repr(C)]
pub struct CStats {
    pub s_str: c_uint,
    pub s_exp: c_int,
    pub s_lvl: c_int,
    pub s_arm: c_int,
    pub s_hpt: c_int,
    pub s_dmg: [c_char; 13],
    pub s_maxhp: c_int,
}

#[repr(C)]
pub struct CThingPlayer {
    pub l_next: *mut CThingPlayer,
    pub l_prev: *mut CThingPlayer,
    pub t_pos: CCoord,
    pub t_turn: c_uchar,
    pub t_type: c_char,
    pub t_disguise: c_char,
    pub t_oldch: c_char,
    pub t_dest: *mut CCoord,
    pub t_flags: c_short,
    pub t_stats: CStats,
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
    static mut player: CThingPlayer;

    fn msg(fmt: *const c_char, ...);
    fn readchar() -> c_int;
    fn addstr(s: *const c_char) -> c_int;
    fn refresh() -> c_int;
    fn get_str(s: *mut c_char, win: *mut CWindow) -> c_int;
    fn md_unlink(file: *mut c_char) -> c_int;

    fn mvcur(ly: c_int, lx: c_int, y: c_int, x: c_int) -> c_int;
    fn putchar(c: c_int) -> c_int;
    fn endwin() -> c_int;
    fn initscr() -> *mut CWindow;
    fn keypad(win: *mut CWindow, flag: c_int) -> c_int;
    fn newwin(nlines: c_int, ncols: c_int, y: c_int, x: c_int) -> *mut CWindow;
    fn clearok(win: *mut CWindow, bf: c_int);
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
    player.t_stats.s_hpt <= 0
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
                msg(c"save file (%s)? ".as_ptr(), file_name_ptr);
                c = readchar();
                mpos = 0;
                if c == ESCAPE {
                    msg(c"".as_ptr());
                    return;
                }
                if c == 'n' as c_int || c == 'N' as c_int || c == 'y' as c_int || c == 'Y' as c_int {
                    break;
                }
                msg(c"please answer Y or N".as_ptr());
            }

            if c == 'y' as c_int || c == 'Y' as c_int {
                addstr(c"Yes\n".as_ptr());
                refresh();
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
                msg(c"file name: ".as_ptr());
                if get_str(buf.as_mut_ptr(), stdscr) == QUIT {
                    msg(c"".as_ptr());
                    return;
                }
                mpos = 0;
            }

            if access(buf.as_ptr(), 0) == 0 {
                loop {
                    msg(c"File exists.  Do you wish to overwrite it?".as_ptr());
                    mpos = 0;
                    c = readchar();
                    if c == ESCAPE {
                        msg(c"".as_ptr());
                        return;
                    }
                    if c == 'y' as c_int || c == 'Y' as c_int {
                        break;
                    }
                    if c == 'n' as c_int || c == 'N' as c_int {
                        continue 'over;
                    }
                    msg(c"Please answer Y or N".as_ptr());
                }
                msg(c"file name: %s".as_ptr(), buf.as_ptr());
                md_unlink(file_name_ptr);
            }

            copy_cstr(file_name_ptr, buf.as_ptr(), MAXSTR);
            savef = fopen(file_name_ptr, c"w".as_ptr());
            if !savef.is_null() {
                save_file(savef);
            }

            msg(c"%s".as_ptr(), strerror(*errno_location()));
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

    mvcur(0, cols - 1, lines - 1, 0);
    putchar('\n' as c_int);
    endwin();
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
        msg(c"Sorry, saved game is out of date.\n".as_ptr());
        return 0;
    }

    let _ = fread(buf.as_mut_ptr() as *mut u8, 1, 80, inf);
    let _ = sscanf(buf.as_ptr(), c"%d x %d\n".as_ptr(), &mut lines, &mut cols);

    initscr();
    keypad(stdscr, 1);

    if lines > LINES {
        endwin();
        msg(c"Sorry, original game was played on a screen with %d lines.\n".as_ptr(), lines);
        msg(c"Current screen only has %d lines. Unable to restore game\n".as_ptr(), LINES);
        return 0;
    }
    if cols > COLS {
        endwin();
        msg(c"Sorry, original game was played on a screen with %d columns.\n".as_ptr(), cols);
        msg(c"Current screen only has %d columns. Unable to restore game\n".as_ptr(), COLS);
        return 0;
    }

    hw = newwin(LINES, COLS, 0, 0);
    setup();
    let _ = rs_restore_file(inf);

    if (master_mode_enabled == 0 || wizard == 0) && md_unlink_open_file(file_ptr, inf) < 0 {
        msg(c"Cannot unlink file\n".as_ptr());
        return 0;
    }

    mpos = 0;
    clearok(stdscr, 1);

    if restore_player_dead() {
        endwin();
        msg(c"\n\"He's dead, Jim\"\n".as_ptr());
        return 0;
    }

    md_tstpresume();
    environ = envp;
    copy_cstr(file_name_ptr, file_ptr, MAXSTR);
    clearok(curscr, 1);
    set_seed(md_getpid());
    msg(c"file name: %s".as_ptr(), file_ptr);
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
