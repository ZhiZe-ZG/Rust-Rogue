//! In-memory dungeon level state.
//!
//! The `Level` type tracks the depth, room layouts, passages, and tile map
//! for the current dungeon level, plus the process-wide singleton holding
//! the live level.

use glam::IVec2;

use super::passages::Passage;
use super::roomgraph::{RoomGraph, MAX_ROOMS};
use super::rooms::{build_generated_rooms, Room};
use super::structure::Structure;
use super::tile::Tile;

/// Map height in cells. Matches the C `places` grid (32 rows), the largest
/// on-screen area a dungeon level can occupy.
pub const LEVEL_HEIGHT: usize = 32;
/// Map width in cells. Matches the C `places` grid (80 columns).
pub const LEVEL_WIDTH: usize = 80;

pub const MAX_LEVEL_ROOMS: usize = MAX_ROOMS;
pub const MAX_LEVEL_PASSAGES: usize = 13;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub depth: i32,
    pub rooms: Vec<Room>,
    pub room_graph: RoomGraph,
    pub passages: Vec<Passage>,
    pub map: Structure,
}

impl Default for Level {
    fn default() -> Self {
        Self {
            depth: 0,
            rooms: Vec::new(),
            room_graph: RoomGraph::new(),
            passages: Vec::new(),
            map: Structure::new(LEVEL_HEIGHT, LEVEL_WIDTH, Tile::Empty),
        }
    }
}

impl Level {
    pub fn new() -> Self {
        let mut level = Self::default();
        level.rooms = (0..MAX_LEVEL_ROOMS)
            .map(|_| Room::new(IVec2::ZERO, IVec2::ZERO, None, None))
            .collect();
        level.map = Structure::new(LEVEL_HEIGHT, LEVEL_WIDTH, Tile::Empty);
        level
    }

    pub fn create() -> Self {
        Self::new()
    }

    pub fn reset(&mut self) {
        self.depth = 0;
        self.rooms.clear();
        self.room_graph.reset();
        self.passages.clear();
        self.map = Structure::new(LEVEL_HEIGHT, LEVEL_WIDTH, Tile::Empty);
    }

    pub fn add_room(&mut self, room: Room) {
        self.rooms.push(room);
    }

    pub fn add_passage(&mut self, passage: Passage) {
        self.passages.push(passage);
    }

    pub(crate) fn generate_rooms_and_connections(
        &mut self,
        rooms: [Room; MAX_ROOMS],
        bsze: IVec2,
    ) -> [Room; MAX_ROOMS] {
        self.room_graph = RoomGraph::for_level(rooms, bsze, self.depth);
        self.room_graph.generate_connections_for_rooms();

        let generated_rooms = build_generated_rooms(self.room_graph.clone().into_rooms());
        self.rooms = generated_rooms
            .iter()
            .filter(|room| !room.is_gone())
            .cloned()
            .collect();

        // Stamp every active room's tile model onto the level map.
        for room in &generated_rooms {
            if room.is_gone() {
                continue;
            }
            let _ = self.map.put_sub_structure(room.position, &room.structure);
        }

        generated_rooms
    }
}

pub fn create_level() -> Level {
    Level::new()
}

static mut CURRENT_LEVEL: Option<Level> = None;

pub unsafe fn current_level_mut() -> &'static mut Level {
    if CURRENT_LEVEL.is_none() {
        CURRENT_LEVEL = Some(Level::new());
    }
    CURRENT_LEVEL.as_mut().unwrap()
}

pub unsafe fn set_current_level(level: Level) -> &'static mut Level {
    CURRENT_LEVEL = Some(level);
    CURRENT_LEVEL.as_mut().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a level with a fixed depth and verify that every active
    /// room's tile model was stamped into the level map.
    #[test]
    fn generation_stamps_rooms_into_map() {
        let mut level = Level::new();
        level.depth = 1;

        // 9 room slots with default geometry (0 size → skipped as gone).
        let rooms = std::array::from_fn(|_| Room::new(IVec2::ZERO, IVec2::ZERO, None, None));
        let bsze = IVec2::new(26, 8);

        let generated = level.generate_rooms_and_connections(rooms, bsze);

        // Every non-gone room must appear in the map at its position.
        for room in generated.iter().filter(|r| !r.is_gone()) {
            for local_y in 0..room.size.y as usize {
                for local_x in 0..room.size.x as usize {
                    let expected = room.structure.get(local_y, local_x).unwrap();
                    let actual = level
                        .map
                        .get(room.position.y as usize + local_y, room.position.x as usize + local_x)
                        .unwrap();
                    assert_eq!(
                        actual, expected,
                        "room at {:?} cell ({local_y},{local_x}) not stamped",
                        room.position
                    );
                }
            }
        }
    }
}
