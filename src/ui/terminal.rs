//! Terminal backend: a retained grid rendered via ratatui, with crossterm
//! handling raw input and terminal lifecycle.
//!
//! The game's rendering model is immediate-mode curses: every write targets a
//! cell, reads back a cell, and flushes with `refresh`. ratatui is
//! retained-mode, so this module keeps a private grid of cells (the "screen")
//! plus a single shared cursor.
//!
//! All windows in this game are full-screen aliases of the same grid, so there
//! is no per-window state and no curses C ABI to preserve.

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
static mut GRID: [[ScreenCell; NCOLS]; NROWS] = [[BLANK_CELL; NCOLS]; NROWS];

/// The single shared cursor position (row, column).
static mut CURSOR: IVec2 = IVec2::ZERO;

/// Current standout mode for subsequent writes (curses `standout`/`standend`).
static mut STANDOUT: bool = false;

/// Input timeout in tenths of a second (`0`/negative means block indefinitely).
static mut INPUT_TIMEOUT: i32 = -1;

/// Whether the terminal has been shut down (`endwin` called).
static mut SHUTDOWN: bool = true;

/// The ratatui terminal handle. Kept alive for the process lifetime; raw mode
/// is toggled independently so shell escapes/suspension can restore it.
static mut TERMINAL: Option<Terminal<CrosstermBackend<Box<dyn std::io::Write>>>> = None;

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
unsafe fn grid_mut() -> &'static mut [[ScreenCell; NCOLS]; NROWS] {
    &mut *std::ptr::addr_of_mut!(GRID)
}

#[inline]
fn in_bounds(y: i32, x: i32) -> bool {
    y >= 0 && x >= 0 && (y as usize) < NROWS && (x as usize) < NCOLS
}

/// Create the crossterm terminal if it does not exist yet, and enable raw mode.
///
/// The alternate screen is entered exactly once (when the terminal is first
/// created / recreated after a suspension). Raw mode toggling is idempotent.
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
unsafe fn set_cell(y: i32, x: i32, ch: u8, standout: bool) {
    if in_bounds(y, x) {
        grid_mut()[y as usize][x as usize] = ScreenCell { ch, standout };
    }
}

#[inline]
unsafe fn cell_at(y: i32, x: i32) -> u8 {
    if in_bounds(y, x) {
        grid_mut()[y as usize][x as usize].ch
    } else {
        b' '
    }
}

#[inline]
unsafe fn advance_cursor() {
    CURSOR.x += 1;
    if CURSOR.x >= NCOLS as i32 {
        CURSOR.x = 0;
        CURSOR.y += 1;
        if CURSOR.y >= NROWS as i32 {
            CURSOR.y = 0;
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

pub(crate) unsafe fn init() {
    ensure_terminal();
}

pub(crate) unsafe fn shutdown() {
    deinit_terminal();
}

pub(crate) unsafe fn is_shutdown() -> bool {
    SHUTDOWN
}

pub(crate) unsafe fn clear() {
    for row in grid_mut().iter_mut() {
        for cell in row.iter_mut() {
            *cell = BLANK_CELL;
        }
    }
    CURSOR = IVec2::ZERO;
}

pub(crate) unsafe fn clear_to_end_of_line() {
    if in_bounds(CURSOR.y, 0) {
        for x in CURSOR.x.max(0) as usize..NCOLS {
            grid_mut()[CURSOR.y as usize][x] = BLANK_CELL;
        }
    }
}

pub(crate) unsafe fn refresh() {
    render();
}

pub(crate) unsafe fn set_standout(enabled: bool) {
    STANDOUT = enabled;
}

pub(crate) unsafe fn move_cursor(pos: IVec2) {
    CURSOR = pos;
}

pub(crate) unsafe fn cursor_pos() -> IVec2 {
    CURSOR
}

pub(crate) unsafe fn write_glyph(ch: char) {
    set_cell(CURSOR.y, CURSOR.x, ch as u8, STANDOUT);
    advance_cursor();
}

pub(crate) unsafe fn write_glyph_at(pos: IVec2, ch: char) {
    CURSOR = pos;
    write_glyph(ch);
}

pub(crate) unsafe fn glyph_at_cursor() -> char {
    cell_at(CURSOR.y, CURSOR.x) as char
}

pub(crate) unsafe fn glyph_at(pos: IVec2) -> char {
    cell_at(pos.y, pos.x) as char
}

pub(crate) unsafe fn write_text(text: &str) {
    for byte in text.bytes() {
        match byte {
            b'\n' => {
                CURSOR.x = 0;
                CURSOR.y += 1;
                if CURSOR.y >= NROWS as i32 {
                    CURSOR.y = 0;
                }
            }
            ch => {
                set_cell(CURSOR.y, CURSOR.x, ch, STANDOUT);
                advance_cursor();
            }
        }
    }
}

pub(crate) unsafe fn write_text_at(pos: IVec2, text: &str) {
    CURSOR = pos;
    write_text(text);
}

// ─── Direct grid access for save/restore (state.rs) ─────────────────────────

pub(crate) unsafe fn read_cell(y: i32, x: i32) -> u8 {
    cell_at(y, x)
}

pub(crate) unsafe fn write_cell(y: i32, x: i32, ch: u8) {
    set_cell(y, x, ch, false);
}

// ─── Input / terminal-mode controls ─────────────────────────────────────────

pub(crate) unsafe fn getch() -> i32 {
    use crossterm::event::{self, Event};

    if SHUTDOWN {
        ensure_terminal();
    }

    let wait = if INPUT_TIMEOUT <= 0 {
        None
    } else {
        Some(std::time::Duration::from_millis(
            (INPUT_TIMEOUT as u64) * 100,
        ))
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

pub(crate) unsafe fn set_escape_delay(_milliseconds: i32) {}

pub(crate) unsafe fn raw() {
    INPUT_TIMEOUT = -1;
    ensure_terminal();
}

pub(crate) unsafe fn nocbreak() {}

pub(crate) unsafe fn echo() {}

pub(crate) unsafe fn noecho() {}

pub(crate) unsafe fn halfdelay(tenths: i32) {
    INPUT_TIMEOUT = tenths;
}

pub(crate) unsafe fn erasechar() -> u8 {
    0x7f
}

pub(crate) unsafe fn killchar() -> u8 {
    0x15
}

pub(crate) unsafe fn flushinp() {
    use crossterm::event::{self, Event};
    while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
        let _ = event::read();
    }
}

pub(crate) unsafe fn baudrate() -> i32 {
    9600
}

pub(crate) unsafe fn move_physical_cursor(_from: IVec2, _to: IVec2) {}
