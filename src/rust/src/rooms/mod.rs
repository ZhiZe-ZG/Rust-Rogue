use crate::structure::Structure;
use crate::tile::Tile;
use glam::IVec2;
use std::os::raw::c_int;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
	pub position: IVec2,
	pub size: IVec2,
	pub structure: Structure,
	pub entry_points: Vec<IVec2>,
}

impl Room {
	pub fn new(
		position: IVec2,
		size: IVec2,
		structure: Option<Structure>,
		entry_points: Option<Vec<IVec2>>,
	) -> Self {
		let default_structure = if size.x > 0 && size.y > 0 {
			build_room_structure(size.y as usize, size.x as usize)
		} else {
			Structure::new(0, 0, Tile::Empty)
		};

		Self {
			position,
			size,
			structure: structure.unwrap_or(default_structure),
			entry_points: entry_points.unwrap_or_default(),
		}
	}

	pub fn add_entry_point(&mut self, relative_pos: IVec2) {
		self.entry_points.push(relative_pos);
	}

	pub fn set_entry_points(&mut self, entry_points: Vec<IVec2>) {
		self.entry_points = entry_points;
	}

	pub fn delete_entry_point(&mut self, relative_pos: IVec2) -> bool {
		if let Some(idx) = self.entry_points.iter().position(|&p| p == relative_pos) {
			self.entry_points.remove(idx);
			true
		} else {
			false
		}
	}

	pub fn place_tile(&mut self, local_y: usize, local_x: usize, tile: Tile) -> bool {
		self.structure.set(local_y, local_x, tile)
	}
}

pub fn place_tile(room: &mut Room, local_y: usize, local_x: usize, tile: Tile) -> bool {
	room.place_tile(local_y, local_x, tile)
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

pub(crate) fn build_maze_structure(height: usize, width: usize) -> Structure {
	let mut structure = Structure::new(height, width, Tile::Empty);

	if height == 0 || width == 0 {
		return structure;
	}

	fn is_passage(structure: &Structure, y: c_int, x: c_int) -> bool {
		matches!(structure.get(y as usize, x as usize), Some(Tile::Passage))
	}

	fn dig_local(structure: &mut Structure, y: c_int, x: c_int, max_y: c_int, max_x: c_int) {
		let deltas: [(c_int, c_int); 4] = [(2, 0), (-2, 0), (0, 2), (0, -2)];

		loop {
			let mut cnt: c_int = 0;
			let mut next_y: c_int = 0;
			let mut next_x: c_int = 0;

			for (dy, dx) in deltas {
				let new_y = y + dy;
				let new_x = x + dx;
				if new_y < 0 || new_y > max_y || new_x < 0 || new_x > max_x {
					continue;
				}
				if is_passage(structure, new_y, new_x) {
					continue;
				}

				cnt += 1;
				if crate::rnd::rnd(cnt) == 0 {
					next_y = new_y;
					next_x = new_x;
				}
			}

			if cnt == 0 {
				return;
			}

			if next_y == y {
				let mid_x = if (next_x - x) < 0 { next_x + 1 } else { next_x - 1 };
				let _ = structure.set(y as usize, mid_x as usize, Tile::Passage);
			} else {
				let mid_y = if (next_y - y) < 0 { next_y + 1 } else { next_y - 1 };
				let _ = structure.set(mid_y as usize, x as usize, Tile::Passage);
			}

			let _ = structure.set(next_y as usize, next_x as usize, Tile::Passage);
			dig_local(structure, next_y, next_x, max_y, max_x);
		}
	}

	let max_y = height as c_int - 1;
	let max_x = width as c_int - 1;
	let start_y = if height > 1 {
		(crate::rnd::rnd(height as c_int) / 2) * 2
	} else {
		0
	};
	let start_x = if width > 1 {
		(crate::rnd::rnd(width as c_int) / 2) * 2
	} else {
		0
	};

	let _ = structure.set(start_y as usize, start_x as usize, Tile::Passage);
	dig_local(&mut structure, start_y, start_x, max_y, max_x);
	structure
}

mod ffi;

pub use ffi::{do_maze, door_open, draw_room};