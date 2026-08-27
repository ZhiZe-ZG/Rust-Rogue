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
/// `up`/`down` are the tiles directly above and below the rendered cell and
/// are only consulted for orientation-sensitive boundary tiles
/// ([`Tile::Wall`] and [`Tile::HiddenDoor`]): a boundary cell renders as a
/// vertical bar (`|`) when the cells above and below it are also boundary
/// cells, and as a horizontal bar (`-`) otherwise. [`Tile::Empty`] renders as
/// `None`; [`Tile::Trap`] is not drawn (it renders like floor, matching the
/// legacy behavior of hidden traps).
pub fn tile_to_ascii(tile: Tile, up: Option<Tile>, down: Option<Tile>) -> Option<c_char> {
	match tile {
		Tile::Empty => None,
		Tile::Floor => Some(FLOOR),
		Tile::Wall | Tile::HiddenDoor => Some(wall_glyph(up, down)),
		Tile::Passage => Some(PASSAGE),
		Tile::Door => Some(DOOR),
		Tile::Stairs => Some(STAIRS),
		Tile::Trap => Some(FLOOR),
	}
}

/// Whether `tile` is a solid boundary cell (wall, hidden door, or open door).
#[inline]
fn is_wall(tile: Option<Tile>) -> bool {
	matches!(tile, Some(Tile::Wall) | Some(Tile::HiddenDoor) | Some(Tile::Door))
}

/// Pick the ASCII glyph for a boundary cell.
///
/// A cell flanked by boundary cells above and below is a vertical wall (`|`);
/// every other boundary cell (horizontal wall, corner, wall segment beside a
/// doorway or passage) renders as a horizontal bar (`-`).
#[inline]
fn wall_glyph(up: Option<Tile>, down: Option<Tile>) -> c_char {
	if is_wall(up) && is_wall(down) {
		V_WALL
	} else {
		H_WALL
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// An interior horizontal wall has non-wall cells above/below it and
	/// renders as `-`.
	#[test]
	fn wall_renders_horizontal_without_vertical_neighbours() {
		assert_eq!(
			tile_to_ascii(Tile::Wall, Some(Tile::Empty), Some(Tile::Floor)),
			Some(H_WALL)
		);
		assert_eq!(
			tile_to_ascii(Tile::Wall, Some(Tile::Floor), Some(Tile::Empty)),
			Some(H_WALL)
		);
	}

	/// A vertical wall is flanked by boundary cells above and below and
	/// renders as `|`.
	#[test]
	fn wall_renders_vertical_between_boundary_neighbours() {
		assert_eq!(
			tile_to_ascii(Tile::Wall, Some(Tile::Wall), Some(Tile::Wall)),
			Some(V_WALL)
		);
		assert_eq!(
			tile_to_ascii(Tile::Wall, Some(Tile::Door), Some(Tile::HiddenDoor)),
			Some(V_WALL)
		);
	}

	/// Corners (a wall with only one boundary neighbour per axis) render as
	/// `-`, matching the legacy room layout.
	#[test]
	fn wall_corner_renders_horizontal() {
		assert_eq!(
			tile_to_ascii(Tile::Wall, Some(Tile::Empty), Some(Tile::Wall)),
			Some(H_WALL)
		);
	}

	/// Hidden doors are drawn like the wall segment they replace until they
	/// are revealed.
	#[test]
	fn hidden_door_renders_like_wall() {
		assert_eq!(
			tile_to_ascii(Tile::HiddenDoor, Some(Tile::Wall), Some(Tile::Wall)),
			Some(V_WALL)
		);
		assert_eq!(
			tile_to_ascii(Tile::HiddenDoor, Some(Tile::Empty), Some(Tile::Floor)),
			Some(H_WALL)
		);
	}

	/// Traps are not drawn: they render like floor until revealed.
	#[test]
	fn trap_renders_as_floor() {
		assert_eq!(tile_to_ascii(Tile::Trap, None, None), Some(FLOOR));
	}
}
