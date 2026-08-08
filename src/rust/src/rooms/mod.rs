use crate::structure::Structure;
use crate::tile::Tile;
use glam::IVec2;
use std::os::raw::c_char;

pub const FLOOR: c_char = b'.' as c_char;
pub const H_WALL: c_char = b'-' as c_char;
pub const V_WALL: c_char = b'|' as c_char;
pub const PASSAGE: c_char = b'#' as c_char;
pub const DOOR: c_char = b'+' as c_char;
pub const STAIRS: c_char = b'%' as c_char;
pub const TRAP: c_char = b'^' as c_char;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
	pub position: IVec2,
	pub size: IVec2,
	pub structure: Structure,
}

impl Room {
	pub fn new(position: IVec2, size: IVec2, structure: Structure) -> Self {
		Self {
			position,
			size,
			structure,
		}
	}
}

pub(crate) fn build_room_structure(height: usize, width: usize) -> Structure {
	let mut structure = Structure::new(height, width, Tile::Empty);

	for y in 0..height {
		for x in 0..width {
			let is_border = y == 0 || x == 0 || y + 1 == height || x + 1 == width;
			let tile = if is_border { Tile::Wall } else { Tile::Floor };
			let _ = structure.set(y, x, tile);
		}
	}

	structure
}

pub(crate) fn room_tile_to_ascii(
	tile: Tile,
	local_y: usize,
	local_x: usize,
	height: usize,
) -> Option<c_char> {
	match tile {
		Tile::Empty => None,
		Tile::Floor => Some(FLOOR),
		Tile::Wall => {
			if local_y == 0 || local_y + 1 == height {
				Some(H_WALL)
			} else {
				let _ = local_x;
				Some(V_WALL)
			}
		}
		Tile::Passage => Some(PASSAGE),
		Tile::Door => Some(DOOR),
		Tile::Stairs => Some(STAIRS),
		Tile::Trap => Some(TRAP),
	}
}

mod ffi;

pub use ffi::{door_open, draw_room, rogue_do_maze};