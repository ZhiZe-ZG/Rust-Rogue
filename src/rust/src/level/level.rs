//! In-memory dungeon level state.
//!
//! The `Level` type tracks the depth, room layouts, passages, and tile map
//! for the current dungeon level, plus the process-wide singleton holding
//! the live level.

use std::os::raw::{c_char, c_int};

use glam::IVec2;

use crate::draw::{clear_tile_flag, set_tile_char};
use crate::rnd::rnd;

use super::ffitools::{DOOR, H_WALL, V_WALL};
use super::passages::Passage;
use super::roomgraph::{RoomGraph, MAX_ROOMS};
use super::rooms::{build_generated_rooms, Room};
use super::structure::Structure;
use super::tile::Tile;

/// Flag bit marking a non-real wall segment.
const F_REAL: c_int = 0x10;

/// Map height in cells. Matches the C `places` grid (32 rows), the largest
/// on-screen area a dungeon level can occupy.
pub const LEVEL_HEIGHT: usize = 32;
/// Map width in cells. Matches the C `places` grid (80 columns).
pub const LEVEL_WIDTH: usize = 80;

pub const MAX_LEVEL_ROOMS: usize = MAX_ROOMS;
pub const MAX_LEVEL_PASSAGES: usize = 13;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Door {
    pub position: IVec2,
    pub kind: DoorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub depth: i32,
    pub rooms: Vec<Room>,
    pub room_graph: RoomGraph,
    pub passages: Vec<Passage>,
    pub doors: Vec<Door>,
    pub map: Structure,
}

impl Default for Level {
    fn default() -> Self {
        Self {
            depth: 0,
            rooms: Vec::new(),
            room_graph: RoomGraph::new(),
            passages: Vec::new(),
            doors: Vec::new(),
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
        self.doors.clear();
        self.map = Structure::new(LEVEL_HEIGHT, LEVEL_WIDTH, Tile::Empty);
    }

    pub fn add_room(&mut self, room: Room) {
        self.rooms.push(room);
    }

    pub fn add_passage(&mut self, passage: Passage) {
        self.passages.push(passage);
    }

    /// Register a door at an absolute map position.
    ///
    /// Records the door for later drawing and, for an open door, stamps the
    /// tile map so the canonical grid reflects the doorway. Wall-segment
    /// doors keep the surrounding wall tile.
    pub fn place_door(&mut self, door: Door) {
        if door.kind == DoorKind::Open {
            let (y, x) = (door.position.y, door.position.x);
            if y >= 0 && x >= 0 {
                let _ = self.map.set(y as usize, x as usize, Tile::Door);
            }
        }
        self.doors.push(door);
    }

    /// Place a door at `pos` on the boundary of `self.rooms[room_index]`.
    ///
    /// Registers `pos` as an exit of the room and, unless the room is a maze,
    /// records a door on this level for later drawing. The door's kind (open
    /// `+` or a wall segment depending on depth and randomness) is decided
    /// here. Returns `pos` so the caller can record it both as a passage tile
    /// and as an entry point of the current corridor.
    pub(crate) fn door(&mut self, room_index: usize, pos: IVec2) -> IVec2 {
        let depth = self.depth;
        let room = &mut self.rooms[room_index];
        room.add_entry_point(pos - room.position);
        room.entry_point_count += 1;

        if room.is_maze() {
            return pos;
        }

        let kind = if rnd(10) + 1 < depth && rnd(5) == 0 {
            if pos.y == room.position.y || pos.y == room.position.y + room.size.y - 1 {
                DoorKind::WallH
            } else {
                DoorKind::WallV
            }
        } else {
            DoorKind::Open
        };

        drop(room);

        self.place_door(Door { position: pos, kind });

        pos
    }

    /// Draw this level's registered doors onto the C `places` grid.
    ///
    /// Each door is written at its absolute map position: an open door as
    /// `+`, and a wall-segment door as `-`/`|` with the `F_REAL` flag
    /// cleared (so it renders as a secret door).
    /// Uses globals: `places` (via `set_tile_char`/`clear_tile_flag`).
    pub unsafe fn draw_doors(&self) {
        for door in &self.doors {
            let (y, x) = (door.position.y, door.position.x);
            match door.kind {
                DoorKind::Open => {
                    set_tile_char(y, x, DOOR);
                }
                DoorKind::WallH => {
                    set_tile_char(y, x, H_WALL);
                    clear_tile_flag(y, x, F_REAL as c_char);
                }
                DoorKind::WallV => {
                    set_tile_char(y, x, V_WALL);
                    clear_tile_flag(y, x, F_REAL as c_char);
                }
            }
        }
    }

    pub(crate) fn generate_rooms_and_connections(
        &mut self,
        rooms: [Room; MAX_ROOMS],
        bsze: IVec2,
    ) -> [Room; MAX_ROOMS] {
        self.room_graph = RoomGraph::for_level(rooms, bsze, self.depth);
        self.room_graph.generate_connections_for_rooms();

        let generated_rooms = build_generated_rooms(self.room_graph.clone().into_rooms());
        self.rooms = generated_rooms.to_vec();

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

    /// Registering a door stores it and leaves it drawable.
    #[test]
    fn place_door_stores_door_for_drawing() {
        let mut level = Level::new();
        level.place_door(Door {
            position: IVec2::new(3, 4),
            kind: DoorKind::Open,
        });
        level.place_door(Door {
            position: IVec2::new(5, 6),
            kind: DoorKind::WallH,
        });

        assert_eq!(level.doors.len(), 2);
        assert_eq!(level.doors[0].position, IVec2::new(3, 4));
        assert_eq!(level.doors[0].kind, DoorKind::Open);
        assert_eq!(level.doors[1].kind, DoorKind::WallH);

        // Open doors are stamped into the tile map; wall segments are not.
        assert_eq!(level.map.get(4, 3), Some(Tile::Door));
        assert_eq!(level.map.get(6, 5), Some(Tile::Empty));
    }

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
