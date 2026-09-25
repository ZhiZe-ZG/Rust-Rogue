//! Terminal user interface.
//!
//! Input and output policy are separated from the low-level terminal backend.

pub mod input;
pub mod output;
pub mod runtime;
mod terminal;

/// A terminal cell position, stored as an integer 2D vector where `x` is the
/// column and `y` is the row.
pub use glam::IVec2;

/// Which logical screen a UI operation targets.
///
/// All windows in this game alias a single full-screen grid, so this is purely
/// descriptive — the variants name the two screens the rest of the code used
/// to pass around as raw window pointers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    /// The standard game screen.
    Stdscr,
    /// The "current screen" alias, used after refreshes.
    Curscr,
}

/// Fixed terminal grid size in (columns, rows).
pub fn screen_size() -> IVec2 {
    terminal::screen_size()
}

/// The physical terminal size in (columns, rows), if it can be queried.
pub fn physical_size() -> Option<IVec2> {
    terminal::physical_size()
}

/// Read one cell from the retained screen grid (row, column).
///
/// Used by save/restore to dump and reload the visible screen.
pub fn screen_cell(y: i32, x: i32) -> u8 {
    terminal::read_cell(y, x)
}

/// Write one cell into the retained screen grid (row, column).
pub fn set_screen_cell(y: i32, x: i32, ch: u8) {
    terminal::write_cell(y, x, ch);
}
