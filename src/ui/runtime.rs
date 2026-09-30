//! Terminal initialization, suspension, and shutdown.

use crate::ui::terminal;
pub(crate) fn initialize() {
    terminal::init();
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
