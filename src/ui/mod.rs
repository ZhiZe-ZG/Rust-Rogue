//! Terminal user interface.
//!
//! Input and output policy are separated from the low-level ncurses adapter.

use std::ffi::c_void;

pub mod input;
pub mod output;
pub mod runtime;
mod terminal;

/// A terminal cell position, stored as an integer 2D vector where `x` is the
/// column and `y` is the row.
pub use glam::IVec2;

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
