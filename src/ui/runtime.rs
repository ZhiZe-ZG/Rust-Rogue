//! Terminal initialization, suspension, and shutdown.

use crate::ui::{terminal, Window};
use glam::IVec2;

pub(crate) fn initialize() -> Window {
    terminal::init();
    Window::Stdscr
}

pub(crate) fn shutdown() {
    crate::ui::output::flush_now();
    terminal::shutdown();
}

pub(crate) fn is_shutdown() -> bool {
    terminal::is_shutdown()
}

pub(crate) fn baud_rate() -> i32 {
    terminal::baudrate()
}

pub(crate) fn move_physical_cursor(from: IVec2, to: IVec2) {
    terminal::move_physical_cursor(from, to);
}
