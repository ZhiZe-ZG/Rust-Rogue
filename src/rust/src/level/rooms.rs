use crate::rnd::rnd;
use glam::IVec2;

use super::roomgraph::MAX_ROOMS;
use super::structure::Structure;
use super::tile::Tile;

/// How a door placed on a room boundary is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorKind {
	/// An open door, rendered as `+`.
	Open,
	/// A wall segment on a horizontal boundary, rendered as `-`.
	WallH,
	/// A wall segment on a vertical boundary, rendered as `|`.
	WallV,
}

/// A door placed on a room boundary while digging corridors.
///
/// `position` is relative to the room's top-left corner, matching the
/// room-local coordinates of [`Room::entry_points`] and `structure`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Door {
	pub position: IVec2,
	pub kind: DoorKind,
}

/// Logical room model used by Rust-side level generation.
///
/// Coordinates are absolute map positions; `structure`, `entry_points`, and
/// `doors` store room-local tiles/coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
	/// Absolute map position of the room's top-left corner.
	pub position: IVec2,
	/// Room size in map cells.
	pub size: IVec2,
	/// Room-local tile model (walled room or maze passages).
	pub structure: Structure,
	/// Doors placed on this room's boundary, relative to `position`.
	pub doors: Vec<Door>,
	/// Doorway positions relative to `position`.
	pub entry_points: Vec<IVec2>,
	/// Absolute map position of the room's gold stash.
	pub gold: IVec2,
	/// Value of the room's gold stash; `0` when there is none.
	pub goldval: i32,
	/// Whether this room slot is removed for the level.
	pub gone: bool,
	/// Whether the room is generated dark.
	pub dark: bool,
	/// Whether the room is generated as a maze.
	pub maze: bool,
	/// Number of registered entry points, mirrored to C's `r_nexits`.
	pub entry_point_count: i32,
}

impl Room {
	/// Create a room with tiles derived from its geometry.
	///
	/// A positive `size` yields a plain walled room; a zero or negative size
	/// yields an empty structure. Pre-built structures (e.g. mazes) should be
	/// supplied via [`Room::with_structure`].
	pub fn new(position: IVec2, size: IVec2) -> Self {
		let structure = if size.x > 0 && size.y > 0 {
			build_room_structure(size.y as usize, size.x as usize)
		} else {
			Structure::new(0, 0, Tile::Empty)
		};
		Self::with_structure(position, size, structure)
	}

	/// Create a room from a pre-built tile structure.
	pub fn with_structure(position: IVec2, size: IVec2, structure: Structure) -> Self {
		Self {
			position,
			size,
			structure,
			doors: Vec::new(),
			entry_points: Vec::new(),
			gold: IVec2::ZERO,
			goldval: 0,
			gone: false,
			dark: false,
			maze: false,
			entry_point_count: 0,
		}
	}

	/// Whether this room slot is removed for the level.
	pub fn is_gone(&self) -> bool {
		self.gone
	}

	/// Whether this room should be generated as a maze room.
	pub fn is_maze(&self) -> bool {
		self.maze
	}

	/// Mark this room slot as gone.
	pub fn mark_gone(&mut self) {
		self.gone = true;
	}

	/// Mark this room as dark.
	pub fn mark_dark(&mut self) {
		self.dark = true;
	}

	/// Convert this room into a maze room (overrides other room flags).
	pub fn set_maze(&mut self) {
		self.maze = true;
		self.dark = false;
		self.gone = false;
	}

	/// Clear all generation flags before layout generation starts.
	pub fn clear_flags(&mut self) {
		self.gone = false;
		self.dark = false;
		self.maze = false;
	}

	/// Register `relative_pos` as an entry point where a corridor joins the room.
	pub fn add_entry_point(&mut self, relative_pos: IVec2) {
		self.entry_points.push(relative_pos);
		self.entry_point_count += 1;
	}

	/// Place a door on one of this room's walls.
	///
	/// `local` is a map coordinate relative to this room's top-left corner
	/// (i.e. `pos - position` of an absolute position); `kind` is chosen by
	/// the caller and decides whether the door renders as an open `+` or as a
	/// wall segment cleared of `F_REAL` (a secret door). Returns `true` when
	/// `local` lands exactly on this room's outer wall row/column: the door is
	/// recorded, `Tile::Door` replaces the wall cell for open doors, and the
	/// coordinate is registered as an entry point. Returns `false` when
	/// `local` lies outside the room or in its interior, leaving the room
	/// unchanged.
	pub fn place_door(&mut self, local: IVec2, kind: DoorKind) -> bool {
		let (local_y, local_x) = (local.y, local.x);
		if local_y < 0 || local_x < 0 || local_y >= self.size.y || local_x >= self.size.x {
			return false;
		}

		let on_boundary = local_y == 0
			|| local_y + 1 == self.size.y
			|| local_x == 0
			|| local_x + 1 == self.size.x;
		if !on_boundary {
			return false;
		}

		if kind == DoorKind::Open {
			let _ = self.structure.set(local_y as usize, local_x as usize, Tile::Door);
		}
		self.doors.push(Door { position: local, kind });
		self.add_entry_point(local);
		true
	}
}

/// Fill each active room's tile structure from its geometry/flags.
pub fn build_generated_rooms(mut rooms: [Room; MAX_ROOMS]) -> [Room; MAX_ROOMS] {
	for room in &mut rooms {
		if room.is_gone() {
			continue;
		}

		if let Some(model) = build_room_model(room.position, room.size, room.is_maze()) {
			room.structure = model.structure;
		}
	}

	rooms
}

#[cfg(test)]
mod tests {
	use super::*;

	fn test_room() -> Room {
		Room::new(IVec2::new(10, 20), IVec2::new(6, 4))
	}

	#[test]
	fn place_door_records_door_and_registers_entry_point() {
		let mut room = test_room();

		// Top wall (y = 0).
		assert!(room.place_door(IVec2::new(1, 0), DoorKind::Open));
		// Right wall (x = size.x - 1).
		assert!(room.place_door(IVec2::new(5, 1), DoorKind::WallV));
		// Bottom wall (y = size.y - 1).
		assert!(room.place_door(IVec2::new(2, 3), DoorKind::Open));
		// Left wall (x = 0).
		assert!(room.place_door(IVec2::new(0, 2), DoorKind::Open));

		assert_eq!(room.entry_points.len(), 4);
		assert_eq!(room.entry_point_count, 4);
		assert_eq!(room.doors.len(), 4);

		// Door positions are stored relative to the room's top-left corner.
		let expected: [IVec2; 4] = [
			IVec2::new(1, 0),
			IVec2::new(5, 1),
			IVec2::new(2, 3),
			IVec2::new(0, 2),
		];
		for (door, exp) in room.doors.iter().zip(expected) {
			assert_eq!(door.position, exp);
		}
		assert_eq!(room.doors[1].kind, DoorKind::WallV, "right wall door keeps its kind");

		// Open doors replace the wall cell; wall-segment doors do not.
		assert_eq!(room.structure.get(0, 1), Some(Tile::Door));
		assert_eq!(room.structure.get(1, 5), Some(Tile::HWall));
		assert_eq!(room.structure.get(3, 2), Some(Tile::Door));
		assert_eq!(room.structure.get(2, 0), Some(Tile::Door));
	}

	#[test]
	fn place_door_rejects_interior_and_out_of_bounds() {
		let mut room = test_room();

		// Interior floor cell (2, 2).
		assert!(!room.place_door(IVec2::new(2, 2), DoorKind::Open));
		// Out of bounds: left of the room.
		assert!(!room.place_door(IVec2::new(-1, 1), DoorKind::Open));
		// Out of bounds: right of the room.
		assert!(!room.place_door(IVec2::new(6, 1), DoorKind::Open));
		// Out of bounds: above the room.
		assert!(!room.place_door(IVec2::new(1, -1), DoorKind::Open));
		// Out of bounds: below the room.
		assert!(!room.place_door(IVec2::new(1, 4), DoorKind::Open));

		assert!(room.entry_points.is_empty());
		assert!(room.entry_point_count == 0);
		assert!(room.doors.is_empty());
		assert_eq!(room.structure.get(2, 2), Some(Tile::Floor));
	}
}

/// Build a [`Room`] model from Rust-native geometry.
///
/// Pure-Rust half of the FFI `build_room_model` split: given a room's
/// position, size, and whether it is a maze, construct the tile structure
/// and wrap everything in a [`Room`]. No C types are involved here.
pub fn build_room_model(position: IVec2, size: IVec2, is_maze: bool) -> Option<Room> {
	if size.x <= 0 || size.y <= 0 {
		return None;
	}

	let structure = if is_maze {
		build_maze_structure(size.y as usize, size.x as usize)
	} else {
		build_room_structure(size.y as usize, size.x as usize)
	};

	Some(Room::with_structure(position, size, structure))
}

pub fn build_room_structure(height: usize, width: usize) -> Structure {
	let mut structure = Structure::new(height, width, Tile::Empty);

	for y in 0..height {
		for x in 0..width {
			let tile = if y == 0 || y + 1 == height {
				Tile::HWall
			} else if x == 0 || x + 1 == width {
				Tile::VWall
			} else {
				Tile::Floor
			};
			let _ = structure.set(y, x, tile);
		}
	}

	structure
}

pub fn build_maze_structure(height: usize, width: usize) -> Structure {
	let mut structure = Structure::new(height, width, Tile::Empty);

	if height == 0 || width == 0 {
		return structure;
	}

	fn is_passage(structure: &Structure, y: i32, x: i32) -> bool {
		match structure.get(y as usize, x as usize) {
			Some(Tile::Passage) => true,
			_ => false,
		}
	}

	fn dig_local(structure: &mut Structure, y: i32, x: i32, max_y: i32, max_x: i32) {
		let deltas: [(i32, i32); 4] = [(2, 0), (-2, 0), (0, 2), (0, -2)];

		loop {
			let mut cnt: i32 = 0;
			let mut next_y: i32 = 0;
			let mut next_x: i32 = 0;

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
				if rnd(cnt) == 0 {
					next_y = new_y;
					next_x = new_x;
				}
			}

			if cnt == 0 {
				return;
			}

			if next_y == y {
				let mid_x = (x + next_x) / 2;
				let _ = structure.set(y as usize, mid_x as usize, Tile::Passage);
			} else {
				let mid_y = (y + next_y) / 2;
				let _ = structure.set(mid_y as usize, x as usize, Tile::Passage);
			}

			let _ = structure.set(next_y as usize, next_x as usize, Tile::Passage);
			dig_local(structure, next_y, next_x, max_y, max_x);
		}
	}

	let max_y = height as i32 - 1;
	let max_x = width as i32 - 1;
	let start_y = if height > 1 {
		(rnd(height as i32) / 2) * 2
	} else {
		0
	};
	let start_x = if width > 1 {
		(rnd(width as i32) / 2) * 2
	} else {
		0
	};

	let _ = structure.set(start_y as usize, start_x as usize, Tile::Passage);
	dig_local(&mut structure, start_y, start_x, max_y, max_x);
	structure
}
