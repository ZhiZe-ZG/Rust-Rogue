//! Utility helpers for the level FFI boundary.
//!
//! Small, dependency-light helpers shared with the legacy FFI glue in
//! [`super::ffi`].

use std::os::raw::c_char;

use super::tile::Tile;

/// ASCII characters used to render tiles onto the screen.
pub const FLOOR: c_char = b'.' as c_char;
pub const PASSAGE: c_char = b'#' as c_char;
pub const H_WALL: c_char = b'-' as c_char;
pub const V_WALL: c_char = b'|' as c_char;
pub const DOOR: c_char = b'+' as c_char;
pub const STAIRS: c_char = b'%' as c_char;
pub const TRAP: c_char = b'^' as c_char;

/// Flag bit marking a cell as a passage (`#`).
pub const F_PASS: c_char = 0x80u8 as c_char;
/// Flag bit marking a cell as a real (opaque) wall.
pub const F_REAL: c_char = 0x10u8 as c_char;
/// Flag bit marking a cell as already drawn on screen refresh.
pub const F_SEEN: c_char = 0x40u8 as c_char;
/// Flat `p_flags` nibble holding a passage component number (0-15).
pub const F_PNUM: c_char = 0x0fu8 as c_char;

/// Convert a [`Tile`] into its on-screen ASCII character.
///
/// Horizontal walls render as horizontal bars (`-`); vertical walls render as
/// vertical bars (`|`). [`Tile::Empty`] renders as `None`.
pub fn tile_to_ascii(tile: Tile) -> Option<c_char> {
	match tile {
		Tile::Empty => None,
		Tile::Floor => Some(FLOOR),
		Tile::HWall => Some(H_WALL),
		Tile::VWall => Some(V_WALL),
		Tile::Passage => Some(PASSAGE),
		Tile::Door => Some(DOOR),
		Tile::Stairs => Some(STAIRS),
		Tile::Trap => Some(TRAP),
	}
}
