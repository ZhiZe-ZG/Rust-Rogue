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
}
