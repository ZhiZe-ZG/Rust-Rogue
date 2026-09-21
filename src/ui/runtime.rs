//! Terminal initialization, suspension, and shutdown.

use crate::ui::terminal;
use crate::ui::{Position, Window};

pub fn initialize() -> Window {
    unsafe { Window::from_raw(terminal::initscr()) }
}

pub fn create_window(size: Position, origin: Position) -> Window {
    unsafe { Window::from_raw(terminal::newwin(size.row, size.col, origin.row, origin.col)) }
}

/// Restore the host terminal after leaving the curses interface.
pub fn shutdown() {
    unsafe {
        terminal::endwin();
    }
}

pub fn is_shutdown() -> bool {
    unsafe { terminal::isendwin() != 0 }
}

pub fn baud_rate() -> i32 {
    unsafe { terminal::baudrate() }
}

pub fn move_physical_cursor(from: Position, to: Position) {
    unsafe {
        terminal::mvcur(from.row, from.col, to.row, to.col);
    }
}
