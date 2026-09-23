//! Terminal initialization, suspension, and shutdown.

use crate::ui::{terminal, Window};
use glam::IVec2;

/// Initialize the terminal backend and return the standard screen handle.
pub fn initialize() -> Window {
    unsafe { terminal::init() }
    Window::Stdscr
}

/// Restore the host terminal after leaving the game interface.
pub fn shutdown() {
    unsafe {
        terminal::shutdown();
    }
}

pub fn is_shutdown() -> bool {
    unsafe { terminal::is_shutdown() }
}

pub fn baud_rate() -> i32 {
    unsafe { terminal::baudrate() }
}

pub fn move_physical_cursor(_from: IVec2, _to: IVec2) {
    unsafe {
        terminal::move_physical_cursor(_from, _to);
    }
}