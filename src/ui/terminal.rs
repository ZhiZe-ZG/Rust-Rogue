//! Terminal backend: a retained grid rendered via ratatui, with crossterm
//! handling raw input and terminal lifecycle.
//!
//! The game's rendering model is immediate-mode curses: every write targets a
//! cell, reads back a cell with `inch()`/`mvinch()`, and flushes with
//! `refresh()`. ratatui is retained-mode, so this module keeps a private grid
//! of cells (the "screen") plus a single shared cursor. All of the legacy
//! curses entry points are re-implemented against that grid, preserving their
//! exact signatures so the rest of the codebase is unchanged.
//!
//! All windows in this game are full-screen aliases of the same grid (the only
//! window ever created is `24x80`), so window handles collapse onto one shared
//! screen and cursor.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

use ratatui::backend::CrosstermBackend;
use ratatui::style::{Modifier, Style};
use ratatui::Terminal;

type Win = *mut c_void;

/// Terminal size (fixed by [`crate::config::GameConfig`]).
const NROWS: usize = crate::config::GameConfig::SCREEN_LINES as usize; // 24
const NCOLS: usize = crate::config::GameConfig::SCREEN_COLS as usize; // 80

/// A single screen cell: one ASCII glyph plus standout (reverse-video).
#[derive(Clone, Copy)]
struct ScreenCell {
    ch: u8,
    standout: bool,
}

const BLANK_CELL: ScreenCell = ScreenCell {
    ch: b' ',
    standout: false,
};

/// The retained screen grid, indexed `grid[y][x]`.
static mut GRID: [[ScreenCell; NCOLS]; NROWS] = [[BLANK_CELL; NCOLS]; NROWS];

/// The single shared cursor (row, column).
static mut CUR_Y: c_int = 0;
static mut CUR_X: c_int = 0;

/// Current standout mode for subsequent writes (curses `standout`/`standend`).
static mut STANDOUT: bool = false;

/// Input timeout in tenths of a second (`-1` means block indefinitely).
static mut INPUT_TIMEOUT: c_int = -1;

/// Whether the terminal has been shut down (`endwin` called).
static mut SHUTDOWN: bool = true;

/// The ratatui terminal handle. Kept alive for the process lifetime; raw mode
/// is toggled independently so shell escapes/suspension can restore it.
static mut TERMINAL: Option<Terminal<CrosstermBackend<Box<dyn std::io::Write>>>> = None;

/// Stable sentinel addresses used as the legacy window handles. The game
/// distinguishes `stdscr`/`curscr`/`hw` but our backend aliases them all.
static mut STDSCR_SLOT: u8 = 0;
static mut CURSCR_SLOT: u8 = 0;
static mut HW_SLOT: u8 = 0;

/// The legacy `stdscr`/`curscr` screen handles and dimensions, previously
/// provided by the ncurses C library. Now owned by Rust so every existing
/// `extern "C" { static mut stdscr/curscr/LINES/COLS ... }` declaration links
/// against these symbols unchanged.
#[no_mangle]
pub static mut stdscr: *mut c_void = std::ptr::null_mut();
#[no_mangle]
pub static mut curscr: *mut c_void = std::ptr::null_mut();
#[no_mangle]
pub static mut LINES: c_int = crate::config::GameConfig::SCREEN_LINES;
#[no_mangle]
pub static mut COLS: c_int = crate::config::GameConfig::SCREEN_COLS;

// ─── Curses key codes (mirrors ncurses `keys.h`) ──────────────────────────────

const ERR: c_int = -1;
const KEY_DOWN: c_int = 0o402; // 258
const KEY_UP: c_int = 0o403; // 259
const KEY_LEFT: c_int = 0o404; // 260
const KEY_RIGHT: c_int = 0o405; // 261
const KEY_HOME: c_int = 0o406; // 262
const KEY_BACKSPACE: c_int = 0o407; // 263
const KEY_NPAGE: c_int = 0o522; // 338
const KEY_PPAGE: c_int = 0o523; // 339
const KEY_END: c_int = 0o550; // 360

// ─── Backend internals ───────────────────────────────────────────────────────

#[inline]
unsafe fn grid_mut() -> &'static mut [[ScreenCell; NCOLS]; NROWS] {
    &mut *std::ptr::addr_of_mut!(GRID)
}

/// Create the crossterm terminal if it does not exist yet, and enable raw mode.
///
/// The alternate screen is entered exactly once (when the terminal is first
/// created / recreated after a suspension). Entering it on every frame would
/// clear the real terminal out from under ratatui's incremental diff, leaving
/// only the cells that changed (the moving `@`) visible. Raw mode toggling is
/// idempotent and safe to call repeatedly.
unsafe fn ensure_terminal() {
    if TERMINAL.is_none() {
        let stdout: Box<dyn std::io::Write> = Box::new(std::io::stdout());
        let backend = CrosstermBackend::new(stdout);
        TERMINAL = Some(Terminal::new(backend).expect("failed to initialize ratatui terminal"));
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::cursor::Hide,
            crossterm::terminal::EnterAlternateScreen
        );
    }
    let _ = crossterm::terminal::enable_raw_mode();
    SHUTDOWN = false;
}

/// Drop back to the host terminal.
unsafe fn deinit_terminal() {
    if let Some(mut terminal) = TERMINAL.take() {
        let _ = terminal.show_cursor();
    }
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::cursor::Show,
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::style::ResetColor,
        crossterm::style::SetAttribute(crossterm::style::Attribute::Reset),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
    );
    let _ = crossterm::terminal::disable_raw_mode();
    SHUTDOWN = true;
}

/// Render the retained grid to the real terminal as one frame.
unsafe fn render() {
    if SHUTDOWN {
        return;
    }
    if let Some(terminal) = TERMINAL.as_mut() {
        let _ = terminal.draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();
            for y in 0..NROWS {
                for x in 0..NCOLS {
                    if (x as u16) >= area.width || (y as u16) >= area.height {
                        continue;
                    }
                    let cell = grid_mut()[y][x];
                    let symbol = (cell.ch as char).to_string();
                    if let Some(target) = buf.cell_mut((x as u16, y as u16)) {
                        target.set_symbol(&symbol);
                        if cell.standout {
                            target.set_style(Style::default().add_modifier(Modifier::REVERSED));
                        } else {
                            target.set_style(Style::default());
                        }
                    }
                }
            }
        });
    }
}

#[inline]
unsafe fn set_cell(y: c_int, x: c_int, ch: u8, standout: bool) {
    if y < 0 || x < 0 || y >= NROWS as c_int || x >= NCOLS as c_int {
        return;
    }
    grid_mut()[y as usize][x as usize] = ScreenCell { ch, standout };
}

#[inline]
unsafe fn cell_at(y: c_int, x: c_int) -> u8 {
    if y < 0 || x < 0 || y >= NROWS as c_int || x >= NCOLS as c_int {
        return b' ';
    }
    grid_mut()[y as usize][x as usize].ch
}

#[inline]
unsafe fn advance_cursor() {
    CUR_X += 1;
    if CUR_X >= NCOLS as c_int {
        CUR_X = 0;
        CUR_Y += 1;
        if CUR_Y >= NROWS as c_int {
            CUR_Y = 0;
        }
    }
}

/// Map a crossterm key event to a curses-compatible code.
fn map_key(event: &crossterm::event::KeyEvent) -> c_int {
    use crossterm::event::{KeyCode, KeyModifiers};

    match event.code {
        KeyCode::Char(c) => {
            if event.modifiers.contains(KeyModifiers::CONTROL) {
                (c.to_ascii_lowercase() as u8 & 0x1f) as c_int
            } else {
                (c as u8) as c_int
            }
        }
        KeyCode::Enter => b'\n' as c_int,
        KeyCode::Esc => 27,
        KeyCode::Tab => b'\t' as c_int,
        KeyCode::Backspace => KEY_BACKSPACE,
        KeyCode::Left => KEY_LEFT,
        KeyCode::Right => KEY_RIGHT,
        KeyCode::Up => KEY_UP,
        KeyCode::Down => KEY_DOWN,
        KeyCode::Home => KEY_HOME,
        KeyCode::End => KEY_END,
        KeyCode::PageUp => KEY_PPAGE,
        KeyCode::PageDown => KEY_NPAGE,
        _ => ERR,
    }
}

// ─── stdscr (no-window) functions ────────────────────────────────────────────

pub unsafe fn clear() -> c_int {
    for row in grid_mut().iter_mut() {
        for cell in row.iter_mut() {
            *cell = BLANK_CELL;
        }
    }
    CUR_Y = 0;
    CUR_X = 0;
    0
}

pub unsafe fn clrtoeol() -> c_int {
    if (0..NROWS as c_int).contains(&CUR_Y) {
        for x in CUR_X.max(0) as usize..NCOLS {
            grid_mut()[CUR_Y as usize][x] = BLANK_CELL;
        }
    }
    0
}

pub unsafe fn refresh() -> c_int {
    render();
    0
}

pub unsafe fn endwin() -> c_int {
    deinit_terminal();
    0
}

pub unsafe fn standout() -> c_int {
    STANDOUT = true;
    0
}

pub unsafe fn standend() -> c_int {
    STANDOUT = false;
    0
}

pub unsafe fn noecho() -> c_int {
    0
}

pub unsafe fn echo() -> c_int {
    0
}

pub unsafe fn raw() -> c_int {
    INPUT_TIMEOUT = -1;
    ensure_terminal();
    0
}

pub unsafe fn nocbreak() -> c_int {
    0
}

pub unsafe fn halfdelay(tenths: c_int) -> c_int {
    INPUT_TIMEOUT = tenths;
    0
}

pub unsafe fn set_escdelay(_size: c_int) -> c_int {
    0
}

pub unsafe fn getch() -> c_int {
    use crossterm::event::{self, Event};

    if SHUTDOWN {
        ensure_terminal();
    }

    let wait = if INPUT_TIMEOUT <= 0 {
        None
    } else {
        Some(std::time::Duration::from_millis((INPUT_TIMEOUT as u64) * 100))
    };

    if let Some(duration) = wait {
        if !event::poll(duration).unwrap_or(false) {
            return ERR;
        }
    }

    match event::read() {
        Ok(Event::Key(key)) => map_key(&key),
        Ok(Event::Resize(_, _)) => ERR,
        _ => ERR,
    }
}

pub unsafe fn baudrate() -> c_int {
    9600
}

pub unsafe fn isendwin() -> c_int {
    if SHUTDOWN {
        1
    } else {
        0
    }
}

pub unsafe fn erasechar() -> c_int {
    0x7f
}

pub unsafe fn killchar() -> c_int {
    0x15
}

pub unsafe fn flushinp() -> c_int {
    use crossterm::event::{self, Event};
    while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
        let _ = event::read();
    }
    0
}

// ─── Cursor movement ─────────────────────────────────────────────────────────

pub unsafe fn move_(y: c_int, x: c_int) -> c_int {
    CUR_Y = y;
    CUR_X = x;
    0
}

// ─── Character output / input ────────────────────────────────────────────────

pub unsafe fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int {
    CUR_Y = y;
    CUR_X = x;
    set_cell(y, x, (ch & 0xff) as u8, STANDOUT);
    advance_cursor();
    0
}

pub unsafe fn addch(ch: c_uint) -> c_int {
    set_cell(CUR_Y, CUR_X, (ch & 0xff) as u8, STANDOUT);
    advance_cursor();
    0
}

pub unsafe fn inch() -> c_int {
    cell_at(CUR_Y, CUR_X) as c_int
}

pub unsafe fn mvinch(y: c_int, x: c_int) -> c_int {
    cell_at(y, x) as c_int
}

pub unsafe fn addstr(s: *const c_char) -> c_int {
    if s.is_null() {
        return ERR;
    }
    let text = CStr::from_ptr(s).to_bytes();
    for byte in text {
        match *byte {
            b'\n' => {
                CUR_X = 0;
                CUR_Y += 1;
                if CUR_Y >= NROWS as c_int {
                    CUR_Y = 0;
                }
            }
            b'\0' => break,
            ch => {
                set_cell(CUR_Y, CUR_X, ch, STANDOUT);
                advance_cursor();
            }
        }
    }
    0
}

pub unsafe fn mvaddstr(y: c_int, x: c_int, s: *const c_char) -> c_int {
    CUR_Y = y;
    CUR_X = x;
    addstr(s)
}

// ─── Window functions ────────────────────────────────────────────────────────
//
// All windows alias the single full-screen grid; window handles are ignored.

pub unsafe fn wclear<T>(_w: *mut T) -> c_int {
    clear()
}

pub unsafe fn wmove<T>(_w: *mut T, y: c_int, x: c_int) -> c_int {
    move_(y, x)
}

pub unsafe fn waddch<T>(_w: *mut T, ch: c_uint) -> c_int {
    addch(ch)
}

pub unsafe fn waddstr<T>(_w: *mut T, s: *const c_char) -> c_int {
    addstr(s)
}

pub unsafe fn wrefresh<T>(_w: *mut T) -> c_int {
    refresh()
}

pub unsafe fn wstandout<T>(_w: *mut T) -> c_int {
    standout()
}

pub unsafe fn wstandend<T>(_w: *mut T) -> c_int {
    standend()
}

pub unsafe fn touchwin<T>(_w: *mut T) -> c_int {
    0
}

pub unsafe fn clearok<T>(_w: *mut T, _bf: c_uchar) -> c_int {
    0
}

pub unsafe fn keypad<T>(_w: *mut T, _bf: c_uchar) -> c_int {
    0
}

pub unsafe fn idlok<T>(_w: *mut T, _bf: c_int) -> c_int {
    0
}

pub unsafe fn leaveok<T>(_w: *mut T, _bf: c_int) -> c_int {
    0
}

pub unsafe fn getcurx<T>(_w: *mut T) -> c_int {
    CUR_X
}

pub unsafe fn getcury<T>(_w: *mut T) -> c_int {
    CUR_Y
}

pub unsafe fn mvcur(_ly: c_int, _lx: c_int, _y: c_int, _x: c_int) -> c_int {
    0
}

pub unsafe fn initscr() -> Win {
    ensure_terminal();

    stdscr = &raw mut STDSCR_SLOT as *mut c_void;
    curscr = &raw mut CURSCR_SLOT as *mut c_void;

    stdscr
}

pub unsafe fn newwin(_nlines: c_int, _ncols: c_int, _y: c_int, _x: c_int) -> Win {
    &raw mut HW_SLOT as *mut c_void
}

// ─── Window-content primitives used by save/restore (state.rs) ───────────────
//
// The C `save_state`/`restore_state` code called `getmaxx`/`getmaxy`/
// `mvwinch`/`mvwaddch` to dump and reload the visible screen. Those become
// direct reads/writes of the retained grid.

/// Window width in columns.
#[no_mangle]
pub unsafe extern "C" fn getmaxx(_win: *mut c_void) -> c_int {
    NCOLS as c_int
}

/// Window height in rows.
#[no_mangle]
pub unsafe extern "C" fn getmaxy(_win: *mut c_void) -> c_int {
    NROWS as c_int
}

/// Read one cell from the screen (col, row).
#[no_mangle]
pub unsafe extern "C" fn mvwinch(_win: *mut c_void, y: c_int, x: c_int) -> c_int {
    cell_at(y, x) as c_int
}

/// Write one cell into the screen (col, row).
#[no_mangle]
pub unsafe extern "C" fn mvwaddch(_win: *mut c_void, y: c_int, x: c_int, ch: c_int) -> c_int {
    set_cell(y, x, (ch & 0xff) as u8, false);
    0
}