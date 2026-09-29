//! Terminal user interface.
//!
//! Input and output policy are separated from the low-level terminal backend.

pub(crate) mod input;
pub(crate) mod output;
pub(crate) mod runtime;
mod state;
mod terminal;

/// Fixed terminal grid size in (columns, rows).
pub(crate) fn screen_size() -> glam::IVec2 {
    terminal::screen_size()
}

/// The physical terminal size in (columns, rows), if it can be queried.
pub(crate) fn physical_size() -> Option<glam::IVec2> {
    terminal::physical_size()
}
