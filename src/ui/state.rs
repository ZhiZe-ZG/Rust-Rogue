use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicI32};
use std::sync::Mutex;

#[cfg(not(test))]
use std::sync::atomic::AtomicU32;

use glam::IVec2;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::config::GameConfig;

const NROWS: usize = GameConfig::SCREEN_LINES as usize;
const NCOLS: usize = GameConfig::SCREEN_COLS as usize;

#[derive(Clone, Copy)]
pub(super) struct ScreenCell {
    pub(super) ch: u8,
    pub(super) standout: bool,
}

impl ScreenCell {
    pub(super) const BLANK: Self = Self {
        ch: b' ',
        standout: false,
    };
}

#[cfg(not(test))]
#[derive(Default)]
pub(super) struct MessageState {
    pub(super) pending: String,
    pub(super) next_position: i32,
}

pub(super) type Backend = CrosstermBackend<Box<dyn Write + Send>>;

/// All process-wide mutable state owned by the terminal UI.
///
/// Locks remain per domain: in particular, input can block without holding a
/// lock needed by screen writes or terminal rendering.
pub(super) struct UiState {
    pub(super) grid: Mutex<[[ScreenCell; NCOLS]; NROWS]>,
    pub(super) cursor: Mutex<IVec2>,
    pub(super) standout: AtomicBool,
    pub(super) input_timeout: AtomicI32,
    pub(super) shutdown: AtomicBool,
    pub(super) terminal: Mutex<Option<Terminal<Backend>>>,
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
            standout: AtomicBool::new(false),
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
