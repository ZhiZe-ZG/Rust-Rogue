//! Terminal initialization, suspension, and shutdown.

use crate::ui::terminal;
use crate::ui::Window;
use glam::IVec2;

pub fn initialize() -> Window {
    unsafe { Window::from_raw(terminal::initscr()) }
}

pub fn create_window(size: IVec2, origin: IVec2) -> Window {
    unsafe { Window::from_raw(terminal::newwin(size.y, size.x, origin.y, origin.x)) }
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

pub fn move_physical_cursor(from: IVec2, to: IVec2) {
    unsafe {
        terminal::mvcur(from.y, from.x, to.y, to.x);
    }
}
