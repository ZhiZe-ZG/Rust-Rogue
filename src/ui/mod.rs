//! Terminal user interface.
//!
//! Input and output policy are separated from the low-level ncurses adapter.

use std::ffi::c_void;

pub mod input;
pub mod output;
pub mod runtime;
mod terminal;

/// A terminal cell position in row/column order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub row: i32,
    pub col: i32,
}

impl Position {
    pub const fn new(row: i32, col: i32) -> Self {
        Self { row, col }
    }
}

/// Opaque handle to a terminal window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window(*mut c_void);

impl Window {
    /// Wrap a legacy window pointer at the UI boundary.
    pub(crate) const unsafe fn from_raw<T>(window: *mut T) -> Self {
        Self(window.cast())
    }

    pub(super) const fn as_raw(self) -> *mut c_void {
        self.0
    }

    pub(crate) const fn into_raw<T>(self) -> *mut T {
        self.0.cast()
    }
}
