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
#[cfg(not(test))]
use std::sync::atomic::AtomicU32;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;

use glam::IVec2;

use ratatui::backend::CrosstermBackend;
use ratatui::style::{Modifier, Style};
use ratatui::Terminal;

/// Terminal size (fixed by [`crate::config::GameConfig`]).
const NROWS: usize = crate::config::GameConfig::SCREEN_LINES as usize; // 24
const NCOLS: usize = crate::config::GameConfig::SCREEN_COLS as usize; // 80

#[derive(Clone, Copy)]
struct ScreenCell {
    ch: u8,
    reverse_video: bool,
}

impl ScreenCell {
    const BLANK: Self = Self {
        ch: b' ',
        reverse_video: false,
    };
}

#[cfg(not(test))]
#[derive(Default)]
pub(super) struct MessageState {
    pub(super) pending: String,
    pub(super) next_position: i32,
}

type Backend = CrosstermBackend<Box<dyn Write + Send>>;

pub(super) struct UiState {
    grid: Mutex<[[ScreenCell; NCOLS]; NROWS]>,
    cursor: Mutex<IVec2>,
    reverse_video: AtomicBool,
    input_timeout: AtomicI32,
    shutdown: AtomicBool,
    terminal: Mutex<Option<Terminal<Backend>>>,
    #[cfg(not(test))]
    pub(super) message: Mutex<MessageState>,
    pub(super) render_pending: AtomicBool,
    #[cfg(not(test))]
    pub(super) hp_width: AtomicI32,
    #[cfg(not(test))]
    pub(super) status_hungry: AtomicI32,
    #[cfg(not(test))]
    pub(super) status_level: AtomicI32,
    #[cfg(not(test))]
    pub(super) status_purse: AtomicI32,
    #[cfg(not(test))]
    pub(super) status_hp: AtomicI32,
    #[cfg(not(test))]
    pub(super) status_armor: AtomicI32,
    #[cfg(not(test))]
    pub(super) status_strength: AtomicU32,
    #[cfg(not(test))]
    pub(super) status_experience: AtomicI32,
}

impl UiState {
    const fn new() -> Self {
        Self {
            grid: Mutex::new([[ScreenCell::BLANK; NCOLS]; NROWS]),
            cursor: Mutex::new(IVec2::ZERO),
            reverse_video: AtomicBool::new(false),
            input_timeout: AtomicI32::new(-1),
            shutdown: AtomicBool::new(true),
            terminal: Mutex::new(None),
            #[cfg(not(test))]
            message: Mutex::new(MessageState {
                pending: String::new(),
                next_position: 0,
            }),
            render_pending: AtomicBool::new(false),
            #[cfg(not(test))]
            hp_width: AtomicI32::new(0),
            #[cfg(not(test))]
            status_hungry: AtomicI32::new(0),
            #[cfg(not(test))]
            status_level: AtomicI32::new(0),
            #[cfg(not(test))]
            status_purse: AtomicI32::new(-1),
            #[cfg(not(test))]
            status_hp: AtomicI32::new(0),
            #[cfg(not(test))]
            status_armor: AtomicI32::new(0),
            #[cfg(not(test))]
            status_strength: AtomicU32::new(0),
            #[cfg(not(test))]
            status_experience: AtomicI32::new(0),
        }
    }
}

pub(super) static UI: UiState = UiState::new();

/// Lock a `Mutex`, recovering from poisoning instead of panicking.
#[inline]
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

// ─── Backend internals ───────────────────────────────────────────────────────

/// The physical terminal size in (columns, rows), if it can be queried.
///
/// Used by startup to reject terminals smaller than the fixed game grid.
pub(crate) fn physical_size() -> Option<IVec2> {
    crossterm::terminal::size()
        .ok()
        .map(|(cols, rows)| IVec2::new(cols as i32, rows as i32))
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
    let mut guard = lock(&UI.terminal);
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
    UI.shutdown.store(false, Ordering::Relaxed);
}

/// Drop back to the host terminal.
fn deinit_terminal() {
    if let Some(mut terminal) = lock(&UI.terminal).take() {
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
    UI.shutdown.store(true, Ordering::Relaxed);
}

/// Render the retained grid to the real terminal as one frame.
pub(super) fn render() {
    if UI.shutdown.load(Ordering::Relaxed) {
        return;
    }
    let grid = lock(&UI.grid);
    let mut guard = lock(&UI.terminal);
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
                        if cell.reverse_video {
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
fn set_cell(y: i32, x: i32, ch: u8, reverse_video: bool) {
    if in_bounds(y, x) {
        lock(&UI.grid)[y as usize][x as usize] = ScreenCell { ch, reverse_video };
    }
}

#[inline]
fn cell_at(y: i32, x: i32) -> u8 {
    if in_bounds(y, x) {
        lock(&UI.grid)[y as usize][x as usize].ch
    } else {
        b' '
    }
}

#[inline]
fn advance_cursor() {
    let mut cursor = lock(&UI.cursor);
    cursor.x += 1;
    if cursor.x >= NCOLS as i32 {
        cursor.x = 0;
        cursor.y += 1;
        if cursor.y >= NROWS as i32 {
            cursor.y = 0;
        }
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
    UI.shutdown.load(Ordering::Relaxed)
}

pub(crate) fn clear() {
    for row in lock(&UI.grid).iter_mut() {
        for cell in row.iter_mut() {
            *cell = ScreenCell::BLANK;
        }
    }
    *lock(&UI.cursor) = IVec2::ZERO;
}

pub(crate) fn clear_to_end_of_line() {
    let cursor = *lock(&UI.cursor);
    if in_bounds(cursor.y, 0) {
        let mut grid = lock(&UI.grid);
        for x in cursor.x.max(0) as usize..NCOLS {
            grid[cursor.y as usize][x] = ScreenCell::BLANK;
        }
    }
}

/// Set the reverse-video mode captured by subsequent cell writes.
pub(crate) fn set_reverse_video(enabled: bool) {
    UI.reverse_video.store(enabled, Ordering::Relaxed);
}

pub(crate) fn move_cursor(pos: IVec2) {
    *lock(&UI.cursor) = pos;
}

pub(crate) fn cursor_pos() -> IVec2 {
    *lock(&UI.cursor)
}

pub(crate) fn write_glyph(ch: char) {
    let cursor = *lock(&UI.cursor);
    set_cell(
        cursor.y,
        cursor.x,
        ch as u8,
        UI.reverse_video.load(Ordering::Relaxed),
    );
    advance_cursor();
}

pub(crate) fn write_glyph_at(pos: IVec2, ch: char) {
    move_cursor(pos);
    write_glyph(ch);
}

pub(crate) fn glyph_at_cursor() -> char {
    let cursor = *lock(&UI.cursor);
    cell_at(cursor.y, cursor.x) as char
}

pub(crate) fn glyph_at(pos: IVec2) -> char {
    cell_at(pos.y, pos.x) as char
}

pub(crate) fn write_text(text: &str) {
    let reverse_video = UI.reverse_video.load(Ordering::Relaxed);
    let mut cursor = lock(&UI.cursor);
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
                    lock(&UI.grid)[cursor.y as usize][cursor.x as usize] =
                        ScreenCell { ch, reverse_video };
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

// ─── Input / terminal-mode controls ─────────────────────────────────────────

pub(crate) fn get_key_event() -> Option<crossterm::event::KeyEvent> {
    use crossterm::event::{self, Event};

    if UI.shutdown.load(Ordering::Relaxed) {
        ensure_terminal();
    }

    let timeout = UI.input_timeout.load(Ordering::Relaxed);
    let wait = if timeout <= 0 {
        None
    } else {
        Some(std::time::Duration::from_millis((timeout as u64) * 100))
    };

    loop {
        if let Some(duration) = wait {
            if !event::poll(duration).unwrap_or(false) {
                return None;
            }
        }

        match event::read() {
            Ok(Event::Key(key)) => return Some(key),
            Ok(Event::Resize(_, _)) | Ok(_) => continue,
            Err(_) => continue,
        }
    }
}

pub(crate) fn raw() {
    UI.input_timeout.store(-1, Ordering::Relaxed);
    ensure_terminal();
}

pub(crate) fn flushinp() {
    use crossterm::event;
    while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
        let _ = event::read();
    }
}
