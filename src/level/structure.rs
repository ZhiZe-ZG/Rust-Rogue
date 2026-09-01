use glam::IVec2;

use super::tile::Tile;

/// 2D tile container used by room/passage/level generation.
///
/// `Structure` owns a rectangular grid of logical [`Tile`] values and offers
/// bounds-checked read/write helpers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Structure {
	height: usize,
	width: usize,
	tiles: Vec<Vec<Tile>>,
}

impl Structure {
	/// Create a `height` by `width` grid filled with `fill`.
	pub fn new(height: usize, width: usize, fill: Tile) -> Self {
		let tiles = vec![vec![fill; width]; height];
		Self {
			height,
			width,
			tiles,
		}
	}

	/// Grid height in rows.
	pub fn height(&self) -> usize {
		self.height
	}

	/// Grid width in columns.
	pub fn width(&self) -> usize {
		self.width
	}

	/// Read a tile at `(y, x)`.
	///
	/// Returns `None` when the coordinate is out of bounds.
	pub fn get(&self, y: usize, x: usize) -> Option<Tile> {
		self.tiles.get(y).and_then(|row| row.get(x)).copied()
	}

	/// Write `tile` at `(y, x)`.
	///
	/// Returns `true` on success, or `false` when out of bounds.
	pub fn set(&mut self, y: usize, x: usize, tile: Tile) -> bool {
		if let Some(slot) = self.tiles.get_mut(y).and_then(|row| row.get_mut(x)) {
			*slot = tile;
			true
		} else {
			false
		}
	}

	/// Borrow the full 2D tile matrix.
	pub fn tiles(&self) -> &[Vec<Tile>] {
		&self.tiles
	}

	/// Place a copy of `sub` into this structure at `position`.
	///
	/// The sub-structure's `(y, x)` cell is written to this structure's
	/// `(position.y + y, position.x + x)`. Returns `true` if the entire
	/// sub-structure fits within this structure's bounds and was fully
	/// placed. If any part would extend past the edge, nothing is placed and
	/// `false` is returned.
	///
	/// This is useful for stamping a room's tile model or a passage onto a
	/// larger map structure. For example:
	/// `map.put_sub_structure(room.position, &room.structure)`.
	pub fn put_sub_structure(&mut self, position: IVec2, sub: &Structure) -> bool {
		let end_y = position.y as i64 + sub.height as i64;
		let end_x = position.x as i64 + sub.width as i64;
		if position.y < 0 || position.x < 0 || end_y > self.height as i64 || end_x > self.width as i64 {
			return false;
		}

		for (sy, row) in sub.tiles.iter().enumerate() {
			for (sx, tile) in row.iter().enumerate() {
				let ty = position.y as usize + sy;
				let tx = position.x as usize + sx;
				self.tiles[ty][tx] = *tile;
			}
		}
		true
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn put_sub_structure_copies_tiles_at_position() {
		let mut map = Structure::new(6, 6, Tile::Empty);
		let mut room = Structure::new(2, 2, Tile::Floor);
		room.set(0, 0, Tile::Wall);

		let ok = map.put_sub_structure(IVec2::new(2, 3), &room);

		assert!(ok);
		assert_eq!(map.get(3, 2), Some(Tile::Wall));
		assert_eq!(map.get(3, 3), Some(Tile::Floor));
		assert_eq!(map.get(4, 2), Some(Tile::Floor));
		assert_eq!(map.get(4, 3), Some(Tile::Floor));
		// Uncovered cells stay untouched.
		assert_eq!(map.get(0, 0), Some(Tile::Empty));
	}

	#[test]
	fn put_sub_structure_rejects_out_of_bounds_placement() {
		let mut map = Structure::new(3, 3, Tile::Empty);
		let room = Structure::new(2, 2, Tile::Floor);

		// Extends past the right/bottom edge.
		assert!(!map.put_sub_structure(IVec2::new(2, 2), &room));
		// Extends past the top/left edge.
		assert!(!map.put_sub_structure(IVec2::new(-1, 0), &room));
		// Nothing was modified by a rejected placement.
		assert!(map.tiles().iter().all(|row| row.iter().all(|&t| t == Tile::Empty)));
	}

	#[test]
	fn put_sub_structure_accepts_exactly_fitting_placement() {
		let mut map = Structure::new(4, 4, Tile::Empty);
		let room = Structure::new(4, 4, Tile::Floor);

		assert!(map.put_sub_structure(IVec2::new(0, 0), &room));
		assert_eq!(map.get(3, 3), Some(Tile::Floor));
	}
}


