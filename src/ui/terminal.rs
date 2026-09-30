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
use std::sync::{LazyLock, Mutex};

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

pub(crate) struct UiState {
    grid: Mutex<[[ScreenCell; NCOLS]; NROWS]>,
    cursor: Mutex<IVec2>,
    reverse_video: AtomicBool,
    input_timeout: AtomicI32,
    shutdown: AtomicBool,
    terminal: LazyLock<Mutex<Terminal<Backend>>>,
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
            terminal: LazyLock::new(|| {
                let stdout: Box<dyn Write + Send> = Box::new(std::io::stdout());
                let backend = CrosstermBackend::new(stdout);
                Mutex::new(Terminal::new(backend).expect("failed to initialize ratatui terminal"))
            }),
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

    pub(crate) fn init_terminal(&self) {
        let _terminal = lock(&self.terminal);
        if self.shutdown.load(Ordering::Relaxed) {
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::cursor::Hide,
                crossterm::terminal::EnterAlternateScreen
            );
        }
        let _ = crossterm::terminal::enable_raw_mode();
        self.shutdown.store(false, Ordering::Relaxed);
    }

    pub(crate) fn deinit_terminal(&self) {
        if self.shutdown.swap(true, Ordering::Relaxed) {
            return;
        }
        let mut terminal = lock(&self.terminal);
        let _ = terminal.show_cursor();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::cursor::Show,
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::style::ResetColor,
            crossterm::style::SetAttribute(crossterm::style::Attribute::Reset),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        );
        let _ = crossterm::terminal::disable_raw_mode();
    }

    pub(crate) fn render(&self) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        let grid = lock(&self.grid);
        let mut terminal = lock(&self.terminal);
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

    #[inline]
    fn set_cell(&self, y: i32, x: i32, ch: u8, reverse_video: bool) {
        if in_bounds(y, x) {
            lock(&self.grid)[y as usize][x as usize] = ScreenCell { ch, reverse_video };
        }
    }

    #[inline]
    fn cell_at(&self, y: i32, x: i32) -> u8 {
        if in_bounds(y, x) {
            lock(&self.grid)[y as usize][x as usize].ch
        } else {
            b' '
        }
    }

    #[inline]
    fn advance_cursor(&self) {
        let mut cursor = lock(&self.cursor);
        cursor.x += 1;
        if cursor.x >= NCOLS as i32 {
            cursor.x = 0;
            cursor.y += 1;
            if cursor.y >= NROWS as i32 {
                cursor.y = 0;
            }
        }
    }

    pub(crate) fn clear(&self) {
        for row in lock(&self.grid).iter_mut() {
            for cell in row.iter_mut() {
                *cell = ScreenCell::BLANK;
            }
        }
        *lock(&self.cursor) = IVec2::ZERO;
    }

    pub(crate) fn clear_to_end_of_line(&self) {
        let cursor = *lock(&self.cursor);
        if in_bounds(cursor.y, 0) {
            let mut grid = lock(&self.grid);
            for x in cursor.x.max(0) as usize..NCOLS {
                grid[cursor.y as usize][x] = ScreenCell::BLANK;
            }
        }
    }

    pub(crate) fn set_reverse_video(&self, enabled: bool) {
        self.reverse_video.store(enabled, Ordering::Relaxed);
    }

    pub(crate) fn move_cursor(&self, pos: IVec2) {
        *lock(&self.cursor) = pos;
    }

    pub(crate) fn cursor_pos(&self) -> IVec2 {
        *lock(&self.cursor)
    }

    pub(crate) fn write_glyph(&self, ch: char) {
        let cursor = *lock(&self.cursor);
        self.set_cell(
            cursor.y,
            cursor.x,
            ch as u8,
            self.reverse_video.load(Ordering::Relaxed),
        );
        self.advance_cursor();
    }

    pub(crate) fn write_glyph_at(&self, pos: IVec2, ch: char) {
        self.move_cursor(pos);
        self.write_glyph(ch);
    }

    pub(crate) fn glyph_at_cursor(&self) -> char {
        let cursor = *lock(&self.cursor);
        self.cell_at(cursor.y, cursor.x) as char
    }

    pub(crate) fn glyph_at(&self, pos: IVec2) -> char {
        self.cell_at(pos.y, pos.x) as char
    }

    pub(crate) fn write_text(&self, text: &str) {
        let reverse_video = self.reverse_video.load(Ordering::Relaxed);
        let mut cursor = lock(&self.cursor);
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
                        lock(&self.grid)[cursor.y as usize][cursor.x as usize] =
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

    pub(crate) fn write_text_at(&self, pos: IVec2, text: &str) {
        self.move_cursor(pos);
        self.write_text(text);
    }

    pub(crate) fn get_key_event(&self) -> Option<crossterm::event::KeyEvent> {
        use crossterm::event::{self, Event};

        if self.shutdown.load(Ordering::Relaxed) {
        }

        let timeout = self.input_timeout.load(Ordering::Relaxed);
        let wait = std::time::Duration::from_millis(if timeout <= 0 {
            100
        } else {
            (timeout as u64) * 100
        });

        loop {
            if !event::poll(wait).unwrap_or(false) {
                if crate::startup::exit_requested() {
                    return None;
                }
                if timeout > 0 {
                    return None;
                }
                continue;
            }

            match event::read() {
                Ok(Event::Key(key)) => return Some(key),
                Ok(Event::Resize(_, _)) | Ok(_) => continue,
                Err(_) => continue,
            }
        }
    }

    /// The physical terminal size in (columns, rows), if it can be queried.
    ///
    /// Used by startup to reject terminals smaller than the fixed game grid.
    pub(crate) fn physical_size(&self) -> Option<IVec2> {
        crossterm::terminal::size()
            .ok()
            .map(|(cols, rows)| IVec2::new(cols as i32, rows as i32))
    }

    pub(crate) fn raw(&self) {
        self.input_timeout.store(-1, Ordering::Relaxed);
    }
}

impl Drop for UiState {
    fn drop(&mut self) {
        self.deinit_terminal();
    }
}

pub(crate) static UI: UiState = UiState::new();

/// Lock a `Mutex`, recovering from poisoning instead of panicking.
#[inline]
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

// ─── Backend internals ───────────────────────────────────────────────────────

#[inline]
fn in_bounds(y: i32, x: i32) -> bool {
    y >= 0 && x >= 0 && (y as usize) < NROWS && (x as usize) < NCOLS
}

pub(crate) fn flushinp() {
    use crossterm::event;
    while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
        let _ = event::read();
    }
}
