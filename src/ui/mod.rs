//! Terminal user interface.
//!
//! Input and output policy are separated from the low-level terminal backend.

pub(crate) mod input;
pub(crate) mod output;
pub(crate) mod runtime;
mod state;
mod terminal;

/// Which logical screen a UI operation targets.
///
/// All windows in this game alias a single full-screen grid, so this is purely
/// descriptive — the variants name the two screens the rest of the code used
/// to pass around as raw window pointers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Window {
    /// The standard game screen.
    Stdscr,
    /// The "current screen" alias, used after refreshes.
    Curscr,
}

/// Fixed terminal grid size in (columns, rows).
pub(crate) fn screen_size() -> glam::IVec2 {
    terminal::screen_size()
}

/// The physical terminal size in (columns, rows), if it can be queried.
pub(crate) fn physical_size() -> Option<glam::IVec2> {
    terminal::physical_size()
}
