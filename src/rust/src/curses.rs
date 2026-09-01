//! C-shape ncurses wrappers.
//!
//! The rest of the game speaks to ncurses with legacy C signatures
//! (`c_int`/`c_uint`/`*mut c_void` windows, raw string pointers).  This
//! module is the single place that translates those calls to the safe
//! `ncurses` crate API, so no Rust file besides this one reaches for the
//! raw C ABI.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uint, c_uchar, c_void};

/// Opaque window handle used by the legacy code: the ncurses crate uses
/// `WINDOW = *mut i8`, the game uses `*mut c_void`. They are the same bytes.
type Win = *mut c_void;

#[inline]
fn to_win(w: Win) -> ncurses::WINDOW {
    w as ncurses::WINDOW
}

// ─── stdscr (no-window) functions ────────────────────────────────────────────

pub unsafe fn clear() -> c_int {
    ncurses::clear()
}

pub unsafe fn clrtoeol() -> c_int {
    ncurses::clrtoeol()
}

pub unsafe fn refresh() -> c_int {
    ncurses::refresh()
}

pub unsafe fn endwin() -> c_int {
    ncurses::endwin()
}

pub unsafe fn standout() -> c_int {
    ncurses::standout()
}

pub unsafe fn standend() -> c_int {
    ncurses::standend()
}

pub unsafe fn noecho() -> c_int {
    ncurses::noecho()
}

pub unsafe fn echo() -> c_int {
    ncurses::echo()
}

pub unsafe fn raw() -> c_int {
    ncurses::raw()
}

pub unsafe fn getch() -> c_int {
    ncurses::getch()
}

pub unsafe fn baudrate() -> c_int {
    ncurses::baudrate()
}

pub unsafe fn isendwin() -> c_int {
    ncurses::isendwin() as c_int
}

pub unsafe fn erasechar() -> c_int {
    ncurses::erasechar().map(|c| c as c_int).unwrap_or(0)
}

pub unsafe fn killchar() -> c_int {
    ncurses::killchar().map(|c| c as c_int).unwrap_or(0)
}

pub unsafe fn flushinp() -> c_int {
    ncurses::flushinp()
}

// ─── Cursor movement ─────────────────────────────────────────────────────────

/// The curses `move(y, x)` — exposed under both spellings the game uses.
pub unsafe fn r#move(y: c_int, x: c_int) -> c_int {
    ncurses::mv(y, x)
}

/// Alias of [`r#move`] for modules that declared `#[link_name = "move"]`.
pub unsafe fn move_(y: c_int, x: c_int) -> c_int {
    ncurses::mv(y, x)
}

// ─── Character output / input ────────────────────────────────────────────────

pub unsafe fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int {
    ncurses::mvaddch(y, x, ch)
}

pub unsafe fn addch(ch: c_uint) -> c_int {
    ncurses::addch(ch)
}

pub unsafe fn inch() -> c_int {
    ncurses::inch() as c_int
}

pub unsafe fn mvinch(y: c_int, x: c_int) -> c_int {
    ncurses::mvinch(y, x) as c_int
}

pub unsafe fn addstr(s: *const c_char) -> c_int {
    if s.is_null() {
        return ncurses::ERR;
    }
    let text = CStr::from_ptr(s).to_string_lossy();
    ncurses::addstr(&text).unwrap_or(ncurses::ERR)
}

pub unsafe fn mvaddstr(y: c_int, x: c_int, s: *const c_char) -> c_int {
    if s.is_null() {
        return ncurses::ERR;
    }
    let text = CStr::from_ptr(s).to_string_lossy();
    ncurses::mvaddstr(y, x, &text).unwrap_or(ncurses::ERR)
}

// ─── Window functions ────────────────────────────────────────────────────────
//
// The legacy code carries window handles as different opaque pointer types
// (`*mut c_void`, `*mut CWindow`, ...), so these wrap any raw pointer.

pub unsafe fn wclear<T>(w: *mut T) -> c_int {
    ncurses::wclear(w as ncurses::WINDOW)
}

pub unsafe fn wmove<T>(w: *mut T, y: c_int, x: c_int) -> c_int {
    ncurses::wmove(w as ncurses::WINDOW, y, x)
}

pub unsafe fn waddch<T>(w: *mut T, ch: c_uint) -> c_int {
    ncurses::waddch(w as ncurses::WINDOW, ch)
}

pub unsafe fn waddstr<T>(w: *mut T, s: *const c_char) -> c_int {
    if s.is_null() {
        return ncurses::ERR;
    }
    let text = CStr::from_ptr(s).to_string_lossy();
    ncurses::waddstr(w as ncurses::WINDOW, &text).unwrap_or(ncurses::ERR)
}

pub unsafe fn wrefresh<T>(w: *mut T) -> c_int {
    ncurses::wrefresh(w as ncurses::WINDOW)
}

pub unsafe fn wstandout<T>(w: *mut T) -> c_int {
    ncurses::wstandout(w as ncurses::WINDOW)
}

pub unsafe fn wstandend<T>(w: *mut T) -> c_int {
    ncurses::wstandend(w as ncurses::WINDOW)
}

pub unsafe fn touchwin<T>(w: *mut T) -> c_int {
    ncurses::touchwin(w as ncurses::WINDOW)
}

pub unsafe fn clearok<T>(w: *mut T, bf: c_uchar) -> c_int {
    ncurses::clearok(w as ncurses::WINDOW, bf != 0)
}

pub unsafe fn keypad<T>(w: *mut T, bf: c_uchar) -> c_int {
    ncurses::keypad(w as ncurses::WINDOW, bf != 0)
}

pub unsafe fn idlok<T>(w: *mut T, bf: c_int) -> c_int {
    ncurses::idlok(w as ncurses::WINDOW, bf != 0)
}

pub unsafe fn leaveok<T>(w: *mut T, bf: c_int) -> c_int {
    ncurses::leaveok(w as ncurses::WINDOW, bf != 0)
}

pub unsafe fn getcurx<T>(w: *mut T) -> c_int {
    ncurses::getcurx(w as ncurses::WINDOW)
}

pub unsafe fn getcury<T>(w: *mut T) -> c_int {
    ncurses::getcury(w as ncurses::WINDOW)
}

pub unsafe fn mvcur(ly: c_int, lx: c_int, y: c_int, x: c_int) -> c_int {
    ncurses::mvcur(ly, lx, y, x)
}

pub unsafe fn initscr() -> Win {
    ncurses::initscr() as Win
}

pub unsafe fn newwin(nlines: c_int, ncols: c_int, y: c_int, x: c_int) -> Win {
    ncurses::newwin(nlines, ncols, y, x) as Win
}

// ─── unctrl ──────────────────────────────────────────────────────────────────
//
// The `ncurses` crate does not wrap `unctrl(3)`, so implement the standard
// behaviour locally: printable chars stay as-is, control chars render as
// `^X`, and bytes >= 0x80 render as `M-x` (with `M-^X` for control).

pub unsafe fn unctrl(ch: c_int) -> *mut c_char {
    static mut BUF: [c_char; 8] = [0; 8];
    let chb = (ch & 0xff) as u8;
    let mut out = [0u8; 8];
    let len;
    if chb < 0x20 {
        out[0] = b'^';
        out[1] = chb + b'@';
        len = 2;
    } else if chb == 0x7f {
        out[0] = b'^';
        out[1] = b'?';
        len = 2;
    } else if chb >= 0x80 {
        out[0] = b'M';
        out[1] = b'-';
        let low = chb & 0x7f;
        if low < 0x20 {
            out[2] = b'^';
            out[3] = low + b'@';
            len = 4;
        } else {
            out[2] = low;
            len = 3;
        }
    } else {
        out[0] = chb;
        len = 1;
    }
    for (i, b) in out.iter().take(len).enumerate() {
        BUF[i] = *b as c_char;
    }
    BUF.as_mut_ptr()
}