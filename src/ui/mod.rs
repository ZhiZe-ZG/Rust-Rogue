//! Terminal user interface.
//!
//! Input and output policy are separated from the low-level terminal backend.

pub(crate) mod input;
pub(crate) mod output;
pub(crate) mod terminal;

/// A copy of the most recently remembered message.
pub fn huh_string() -> String {
	terminal::UI.huh_string()
}

/// Replace the remembered message without retaining a UI lock.
pub fn set_huh_string(text: &str) {
	terminal::UI.set_huh_string(text);
}

/// Set the message cursor used for pagination.
pub fn set_mpos(value: i32) {
	terminal::UI.set_mpos(value);
}
