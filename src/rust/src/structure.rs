use crate::tile::Tile;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Structure {
	height: usize,
	width: usize,
	tiles: Vec<Vec<Tile>>,
}

impl Structure {
	pub fn new(height: usize, width: usize, fill: Tile) -> Self {
		let tiles = vec![vec![fill; width]; height];
		Self {
			height,
			width,
			tiles,
		}
	}

	pub fn height(&self) -> usize {
		self.height
	}

	pub fn width(&self) -> usize {
		self.width
	}

	pub fn get(&self, y: usize, x: usize) -> Option<Tile> {
		self.tiles.get(y).and_then(|row| row.get(x)).copied()
	}

	pub fn set(&mut self, y: usize, x: usize, tile: Tile) -> bool {
		if let Some(slot) = self.tiles.get_mut(y).and_then(|row| row.get_mut(x)) {
			*slot = tile;
			true
		} else {
			false
		}
	}

	pub fn tiles(&self) -> &[Vec<Tile>] {
		&self.tiles
	}
}
