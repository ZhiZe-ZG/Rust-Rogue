//! Terminal initialization, suspension, and shutdown.

use crate::ui::{terminal, Window};
use glam::IVec2;

/// Initialize the terminal backend and return the standard screen handle.
pub fn initialize() -> Window {
    terminal::init();
    Window::Stdscr
}

/// Restore the host terminal after leaving the game interface.
pub fn shutdown() {
    terminal::shutdown();
}

pub fn is_shutdown() -> bool {
    terminal::is_shutdown()
}

pub fn baud_rate() -> i32 {
    terminal::baudrate()
}

pub fn move_physical_cursor(from: IVec2, to: IVec2) {
    terminal::move_physical_cursor(from, to);
}