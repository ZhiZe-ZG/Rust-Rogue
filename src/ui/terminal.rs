//! Terminal backend: a retained grid rendered via ratatui, with crossterm
//! handling raw input and terminal lifecycle.
//!
//! The game's rendering model is immediate-mode curses: every write targets a
//! cell, reads back a cell, and flushes with `refresh`. ratatui is
//! retained-mode, so this module keeps a private grid of cells (the "screen")
//! plus a single shared cursor.
//!
//! All windows in this game are full-screen aliases of the same grid, so there
//! is no per-window state and no curses C ABI to preserve. The backend state
//! lives in ordinary process-wide statics (locks and atomics) rather than
//! `static mut`, so the whole module is safe.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;

use glam::IVec2;

use ratatui::backend::CrosstermBackend;
use ratatui::style::{Modifier, Style};
use ratatui::Terminal;

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
static GRID: Mutex<[[ScreenCell; NCOLS]; NROWS]> = Mutex::new([[BLANK_CELL; NCOLS]; NROWS]);

/// The single shared cursor position (row, column).
static CURSOR: Mutex<IVec2> = Mutex::new(IVec2::ZERO);

/// Current standout mode for subsequent writes (curses `standout`/`standend`).
static STANDOUT: AtomicBool = AtomicBool::new(false);

/// Input timeout in tenths of a second (`0`/negative means block indefinitely).
static INPUT_TIMEOUT: AtomicI32 = AtomicI32::new(-1);

/// Whether the terminal has been shut down (`endwin` called).
static SHUTDOWN: AtomicBool = AtomicBool::new(true);

/// The ratatui terminal handle. Kept alive for the process lifetime; raw mode
/// is toggled independently so shell escapes/suspension can restore it.
type Backend = CrosstermBackend<Box<dyn Write + Send>>;
static TERMINAL: Mutex<Option<Terminal<Backend>>> = Mutex::new(None);

/// Lock a `Mutex`, recovering from poisoning instead of panicking.
#[inline]
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

// ─── Curses key codes (mirrors ncurses `keys.h`) ──────────────────────────────

const ERR: i32 = -1;
const KEY_DOWN: i32 = 0o402; // 258
const KEY_UP: i32 = 0o403; // 259
const KEY_LEFT: i32 = 0o404; // 260
const KEY_RIGHT: i32 = 0o405; // 261
const KEY_HOME: i32 = 0o406; // 262
const KEY_BACKSPACE: i32 = 0o407; // 263
const KEY_NPAGE: i32 = 0o522; // 338
const KEY_PPAGE: i32 = 0o523; // 339
const KEY_END: i32 = 0o550; // 360

// ─── Backend internals ───────────────────────────────────────────────────────

/// Fixed screen size in (columns, rows).
pub(crate) const fn screen_size() -> IVec2 {
    IVec2::new(NCOLS as i32, NROWS as i32)
}

#[inline]
fn in_bounds(y: i32, x: i32) -> bool {
    y >= 0 && x >= 0 && (y as usize) < NROWS && (x as usize) < NCOLS
}

/// Create the crossterm terminal if it does not exist yet, and enable raw mode.
///
/// The alternate screen is entered exactly once (when the terminal is first
/// created / recreated after a suspension). Raw mode toggling is idempotent.
fn ensure_terminal() {
    let mut guard = lock(&TERMINAL);
    if guard.is_none() {
        let stdout: Box<dyn Write + Send> = Box::new(std::io::stdout());
        let backend = CrosstermBackend::new(stdout);
        *guard = Some(Terminal::new(backend).expect("failed to initialize ratatui terminal"));
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::cursor::Hide,
            crossterm::terminal::EnterAlternateScreen
        );
    }
    let _ = crossterm::terminal::enable_raw_mode();
    SHUTDOWN.store(false, Ordering::Relaxed);
}

/// Drop back to the host terminal.
fn deinit_terminal() {
    if let Some(mut terminal) = lock(&TERMINAL).take() {
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
    SHUTDOWN.store(true, Ordering::Relaxed);
}

/// Render the retained grid to the real terminal as one frame.
fn render() {
    if SHUTDOWN.load(Ordering::Relaxed) {
        return;
    }
    let grid = lock(&GRID);
    let mut guard = lock(&TERMINAL);
    if let Some(terminal) = guard.as_mut() {
        let _ = terminal.draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();
            for y in 0..NROWS {
                for x in 0..NCOLS {
                    if (x as u16) >= area.width || (y as u16) >= area.height {
                        continue;
                    }
                    let cell = grid[y][x];
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
fn set_cell(y: i32, x: i32, ch: u8, standout: bool) {
    if in_bounds(y, x) {
        lock(&GRID)[y as usize][x as usize] = ScreenCell { ch, standout };
    }
}

#[inline]
fn cell_at(y: i32, x: i32) -> u8 {
    if in_bounds(y, x) {
        lock(&GRID)[y as usize][x as usize].ch
    } else {
        b' '
    }
}

#[inline]
fn advance_cursor() {
    let mut cursor = lock(&CURSOR);
    cursor.x += 1;
    if cursor.x >= NCOLS as i32 {
        cursor.x = 0;
        cursor.y += 1;
        if cursor.y >= NROWS as i32 {
            cursor.y = 0;
        }
    }
}

/// Map a crossterm key event to a curses-compatible code.
fn map_key(event: &crossterm::event::KeyEvent) -> i32 {
    use crossterm::event::{KeyCode, KeyModifiers};

    match event.code {
        KeyCode::Char(c) => {
            if event.modifiers.contains(KeyModifiers::CONTROL) {
                (c.to_ascii_lowercase() as u8 & 0x1f) as i32
            } else {
                (c as u8) as i32
            }
        }
        KeyCode::Enter => b'\n' as i32,
        KeyCode::Esc => 27,
        KeyCode::Tab => b'\t' as i32,
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

// ─── Screen functions ────────────────────────────────────────────────────────

pub(crate) fn init() {
    ensure_terminal();
}

pub(crate) fn shutdown() {
    deinit_terminal();
}

pub(crate) fn is_shutdown() -> bool {
    SHUTDOWN.load(Ordering::Relaxed)
}

pub(crate) fn clear() {
    for row in lock(&GRID).iter_mut() {
        for cell in row.iter_mut() {
            *cell = BLANK_CELL;
        }
    }
    *lock(&CURSOR) = IVec2::ZERO;
}

pub(crate) fn clear_to_end_of_line() {
    let cursor = *lock(&CURSOR);
    if in_bounds(cursor.y, 0) {
        let mut grid = lock(&GRID);
        for x in cursor.x.max(0) as usize..NCOLS {
            grid[cursor.y as usize][x] = BLANK_CELL;
        }
    }
}

pub(crate) fn refresh() {
    render();
}

pub(crate) fn set_standout(enabled: bool) {
    STANDOUT.store(enabled, Ordering::Relaxed);
}

pub(crate) fn move_cursor(pos: IVec2) {
    *lock(&CURSOR) = pos;
}

pub(crate) fn cursor_pos() -> IVec2 {
    *lock(&CURSOR)
}

pub(crate) fn write_glyph(ch: char) {
    let cursor = *lock(&CURSOR);
    set_cell(
        cursor.y,
        cursor.x,
        ch as u8,
        STANDOUT.load(Ordering::Relaxed),
    );
    advance_cursor();
}

pub(crate) fn write_glyph_at(pos: IVec2, ch: char) {
    move_cursor(pos);
    write_glyph(ch);
}

pub(crate) fn glyph_at_cursor() -> char {
    let cursor = *lock(&CURSOR);
    cell_at(cursor.y, cursor.x) as char
}

pub(crate) fn glyph_at(pos: IVec2) -> char {
    cell_at(pos.y, pos.x) as char
}

pub(crate) fn write_text(text: &str) {
    let standout = STANDOUT.load(Ordering::Relaxed);
    let mut cursor = lock(&CURSOR);
    for byte in text.bytes() {
        match byte {
            b'\n' => {
                cursor.x = 0;
                cursor.y += 1;
                if cursor.y >= NROWS as i32 {
                    cursor.y = 0;
                }
            }
            ch => {
                if in_bounds(cursor.y, cursor.x) {
                    lock(&GRID)[cursor.y as usize][cursor.x as usize] = ScreenCell { ch, standout };
                }
                cursor.x += 1;
                if cursor.x >= NCOLS as i32 {
                    cursor.x = 0;
                    cursor.y += 1;
                    if cursor.y >= NROWS as i32 {
                        cursor.y = 0;
                    }
                }
            }
        }
    }
}

pub(crate) fn write_text_at(pos: IVec2, text: &str) {
    move_cursor(pos);
    write_text(text);
}

// ─── Direct grid access for save/restore (state.rs) ─────────────────────────

pub(crate) fn read_cell(y: i32, x: i32) -> u8 {
    cell_at(y, x)
}

pub(crate) fn write_cell(y: i32, x: i32, ch: u8) {
    set_cell(y, x, ch, false);
}

// ─── Input / terminal-mode controls ─────────────────────────────────────────

pub(crate) fn getch() -> i32 {
    use crossterm::event::{self, Event};

    if SHUTDOWN.load(Ordering::Relaxed) {
        ensure_terminal();
    }

    let timeout = INPUT_TIMEOUT.load(Ordering::Relaxed);
    let wait = if timeout <= 0 {
        None
    } else {
        Some(std::time::Duration::from_millis((timeout as u64) * 100))
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

pub(crate) fn set_escape_delay(_milliseconds: i32) {}

pub(crate) fn raw() {
    INPUT_TIMEOUT.store(-1, Ordering::Relaxed);
    ensure_terminal();
}

pub(crate) fn nocbreak() {}

pub(crate) fn echo() {}

pub(crate) fn noecho() {}

pub(crate) fn halfdelay(tenths: i32) {
    INPUT_TIMEOUT.store(tenths, Ordering::Relaxed);
}

pub(crate) fn erasechar() -> u8 {
    0x7f
}

pub(crate) fn killchar() -> u8 {
    0x15
}

pub(crate) fn flushinp() {
    use crossterm::event;
    while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
        let _ = event::read();
    }
}

pub(crate) fn baudrate() -> i32 {
    9600
}

pub(crate) fn move_physical_cursor(_from: IVec2, _to: IVec2) {}