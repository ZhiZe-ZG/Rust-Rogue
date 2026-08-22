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

/// Convert a [`Tile`] into its on-screen ASCII character.
///
/// Walls on the top/bottom edge of the room render as horizontal bars (`-`);
/// walls in the interior render as vertical bars (`|`). [`Tile::Empty`]
/// renders as `None`.
pub fn tile_to_ascii(tile: Tile, local_y: usize, _local_x: usize, height: usize) -> Option<c_char> {
	match tile {
		Tile::Empty => None,
		Tile::Floor => Some(FLOOR),
		Tile::Wall => {
			if local_y == 0 || local_y + 1 == height {
				Some(H_WALL)
			} else {
				Some(V_WALL)
			}
		}
		Tile::Passage => Some(PASSAGE),
		Tile::Door => Some(DOOR),
		Tile::Stairs => Some(STAIRS),
		Tile::Trap => Some(TRAP),
	}
}
