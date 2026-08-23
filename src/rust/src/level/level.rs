//! In-memory dungeon level state.
//!
//! The `Level` type tracks the depth, room layouts, passages, and tile map
//! for the current dungeon level, plus the process-wide singleton holding
//! the live level.

use std::os::raw::{c_char, c_int};

use glam::IVec2;

use crate::draw::{clear_tile_flag, set_tile_char};
use crate::player::CCoord;
use crate::rnd::rnd;

use super::ffitools::{DOOR, H_WALL, V_WALL};
use super::passages::{CorridorPlan, Passage};
use super::roomgraph::{RoomGraph, MAX_ROOMS};
use super::rooms::{build_generated_rooms, Room};
use super::structure::Structure;
use super::tile::Tile;

/// Flag bit marking a non-real wall segment.
const F_REAL: c_int = 0x10;

unsafe extern "C" {
    fn msg(fmt: *const c_char, ...);
}

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

    /// Stamp a passage tile at absolute map position `pos`.
    ///
    /// Marks the cell as [`Tile::Passage`] in the level map so it becomes
    /// part of the canonical grid (mirrored to the C `places` grid by
    /// `sync_passages_to_c`). Returns `pos` so callers can record it both as
    /// a tile of the current corridor and, when applicable, an entry point.
    pub(crate) fn putpass(&mut self, pos: IVec2) -> IVec2 {
        let (y, x) = (pos.y, pos.x);
        if y >= 0 && x >= 0 {
            let _ = self.map.set(y as usize, x as usize, Tile::Passage);
        }
        pos
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

    /// Place one end of a corridor on the boundary of `self.rooms[room_index]`.
    ///
    /// If the room is still present, a door is registered on its boundary via
    /// [`Level::door`]; if it was removed (`ISGONE`), a plain passage tile is
    /// laid instead (see [`Level::putpass`]). The placed coordinate is
    /// recorded into `tiles` (and into `entry_points` for doors) so the
    /// caller can reconstruct the corridor as a [`Passage`].
    pub(crate) fn place_corridor_end(
        &mut self,
        room_index: usize,
        pos: IVec2,
        tiles: &mut Vec<IVec2>,
        entry_points: &mut Vec<IVec2>,
    ) {
        if self.rooms[room_index].is_gone() {
            tiles.push(self.putpass(pos));
        } else {
            let door_pos = self.door(room_index, pos);
            tiles.push(door_pos);
            entry_points.push(door_pos);
        }
    }

    /// Lay the passage tiles of the corridor described by `plan`.
    ///
    /// Walks an L-shaped path: from `plan.start` it steps along `plan.step`
    /// for `plan.distance` cells, making a perpendicular run of
    /// `plan.turn_distance` cells starting at `plan.turn_spot`, so the
    /// corridor ends up aligned with `plan.end`. A final check warns if the
    /// path did not reach the expected end point. Every laid tile is
    /// recorded into `tiles`.
    pub(crate) fn dig_corridor(&mut self, plan: &CorridorPlan, tiles: &mut Vec<IVec2>) {
        let mut curr = plan.start;
        let mut distance = plan.distance;

        while distance > 0 {
            curr.x += plan.step.x;
            curr.y += plan.step.y;

            if distance == plan.turn_spot {
                let mut remaining = plan.turn_distance;
                while remaining > 0 {
                    tiles.push(self.putpass(IVec2::new(curr.x, curr.y)));
                    curr.x += plan.turn_step.x;
                    curr.y += plan.turn_step.y;
                    remaining -= 1;
                }
            }

            tiles.push(self.putpass(IVec2::new(curr.x, curr.y)));
            distance -= 1;
        }

        curr.x += plan.step.x;
        curr.y += plan.step.y;
        if curr.x != plan.end.x || curr.y != plan.end.y {
            unsafe {
                msg(b"warning, connectivity problem on this level\0".as_ptr() as *const c_char);
            }
        }
    }

    /// Pick the point where the corridor meets `self.rooms[room_index]`'s boundary.
    ///
    /// For a vertical corridor (`direc == 'd'`) the point sits on the room's
    /// bottom wall when `start` is set (the room the corridor leaves) or on its
    /// top wall otherwise, randomizing the x coordinate. For a horizontal
    /// corridor it sits on the right (`start`) or left wall, randomizing the y
    /// coordinate. In maze rooms the point is redrawn until it lands on an
    /// existing passage so the corridor always joins the maze. If the room was
    /// removed ([`Room::is_gone`]), its top-left corner is returned unchanged.
    pub(crate) fn entry_point(&self, room_index: usize, direc: char, start: bool) -> CCoord {
        let room = &self.rooms[room_index];
        let mut p = CCoord {
            x: room.position.x,
            y: room.position.y,
        };
        if !room.is_gone() {
            loop {
                if direc == 'd' {
                    p.x = room.position.x + rnd(room.size.x - 2) + 1;
                    p.y = if start { room.position.y + room.size.y - 1 } else { room.position.y };
                } else {
                    p.y = room.position.y + rnd(room.size.y - 2) + 1;
                    p.x = if start { room.position.x + room.size.x - 1 } else { room.position.x };
                }
                if !room.is_maze()
                    || matches!(self.map.get(p.y as usize, p.x as usize), Some(Tile::Passage))
                {
                    break;
                }
            }
        }
        p
    }

    /// Determine the direction of the corridor between rooms `r1` and `r2`.
    ///
    /// Rooms side by side (indices differing by one) are connected by a
    /// horizontal corridor (`'r'`); rooms stacked (any other pair) by a
    /// vertical corridor (`'d'`). Also returns the smaller index, which anchors
    /// the corridor's start.
    pub(crate) fn corridor_direction(r1: c_int, r2: c_int) -> (char, usize) {
        if r1 < r2 {
            let direc = if r1 + 1 == r2 { 'r' } else { 'd' };
            (direc, r1 as usize)
        } else {
            let direc = if r2 + 1 == r1 { 'r' } else { 'd' };
            (direc, r2 as usize)
        }
    }

    /// Compute the full geometric plan for a corridor between rooms `r1`/`r2`.
    ///
    /// Determines the corridor direction from the room indices, picks random
    /// entry points on both room boundaries, and derives the straight run, the
    /// perpendicular turn, and the random position of the turn.
    pub(crate) fn plan_corridor(&self, r1: c_int, r2: c_int) -> CorridorPlan {
        let (direc, base_room) = Self::corridor_direction(r1, r2);
        let partner_room = if direc == 'd' { base_room + 3 } else { base_room + 1 };

        let step = if direc == 'd' {
            CCoord { x: 0, y: 1 }
        } else {
            CCoord { x: 1, y: 0 }
        };

        let start = self.entry_point(base_room, direc, true);
        let end = self.entry_point(partner_room, direc, false);

        let (distance, turn_step, turn_distance) = if direc == 'd' {
            (
                (start.y - end.y).abs() - 1,
                CCoord {
                    x: if start.x < end.x { 1 } else { -1 },
                    y: 0,
                },
                (start.x - end.x).abs(),
            )
        } else {
            (
                (start.x - end.x).abs() - 1,
                CCoord {
                    x: 0,
                    y: if start.y < end.y { 1 } else { -1 },
                },
                (start.y - end.y).abs(),
            )
        };

        let turn_spot = if distance > 1 { rnd(distance - 1) + 1 } else { 1 };

        CorridorPlan {
            base_room,
            partner_room,
            step,
            start,
            end,
            distance,
            turn_step,
            turn_distance,
            turn_spot,
        }
    }

    /// Wrap the tiles laid while digging a corridor into a [`Passage`].
    ///
    /// Takes the tile and entry-point coordinates collected while digging the
    /// corridor, computes their bounding box, and stores the resulting
    /// [`Passage`] with its coordinates made relative to the bounding box
    /// origin.
    pub(crate) fn finish_passage(&mut self, tiles: Vec<IVec2>, entry_points: Vec<IVec2>) {
        if tiles.is_empty() {
            return;
        }

        let min_x = tiles.iter().map(|p| p.x).min().unwrap_or(0);
        let max_x = tiles.iter().map(|p| p.x).max().unwrap_or(0);
        let min_y = tiles.iter().map(|p| p.y).min().unwrap_or(0);
        let max_y = tiles.iter().map(|p| p.y).max().unwrap_or(0);

        let position = IVec2::new(min_x, min_y);
        let size = IVec2::new(max_x - min_x + 1, max_y - min_y + 1);

        let relative_tiles = tiles
            .into_iter()
            .map(|p| IVec2::new(p.x - min_x, p.y - min_y))
            .collect();
        let relative_entry_points = entry_points
            .into_iter()
            .map(|p| IVec2::new(p.x - min_x, p.y - min_y))
            .collect();

        let passage = Passage {
            position,
            size,
            tiles: relative_tiles,
            entry_points: relative_entry_points,
        };
        self.add_passage(passage);
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

    /// Stamping a passage tile records it in the level map and returns it.
    #[test]
    fn putpass_stamps_passage_into_map() {
        let mut level = Level::new();
        let pos = level.putpass(IVec2::new(5, 7));

        assert_eq!(pos, IVec2::new(5, 7));
        assert_eq!(level.map.get(7, 5), Some(Tile::Passage));
    }

    /// Out-of-bounds passage placement is ignored without panicking.
    #[test]
    fn putpass_ignores_out_of_bounds_positions() {
        let mut level = Level::new();

        let pos = level.putpass(IVec2::new(-1, 7));
        assert_eq!(pos, IVec2::new(-1, 7));
        assert_eq!(level.map.get(7, 0), Some(Tile::Empty));

        let pos = level.putpass(IVec2::new(5, -3));
        assert_eq!(pos, IVec2::new(5, -3));
        assert_eq!(level.map.get(0, 5), Some(Tile::Empty));
    }

    /// `finish_passage` wraps laid tiles into a [`Passage`] with coordinates
    /// made relative to the bounding-box origin.
    #[test]
    fn finish_passage_builds_relative_passage() {
        let mut level = Level::new();
        level.finish_passage(
            vec![IVec2::new(2, 3), IVec2::new(3, 3), IVec2::new(4, 3), IVec2::new(4, 4)],
            vec![IVec2::new(2, 3)],
        );

        assert_eq!(level.passages.len(), 1);
        let passage = &level.passages[0];
        assert_eq!(passage.position, IVec2::new(2, 3));
        assert_eq!(passage.size, IVec2::new(3, 2));
        assert_eq!(
            passage.tiles,
            vec![IVec2::new(0, 0), IVec2::new(1, 0), IVec2::new(2, 0), IVec2::new(2, 1)]
        );
        assert_eq!(passage.entry_points, vec![IVec2::new(0, 0)]);
    }

    /// `finish_passage` ignores empty tile lists without storing a passage.
    #[test]
    fn finish_passage_ignores_empty_tiles() {
        let mut level = Level::new();
        level.finish_passage(Vec::new(), Vec::new());
        assert!(level.passages.is_empty());
    }

    /// `plan_corridor` anchors the plan at the two rooms' boundaries.
    #[test]
    fn plan_corridor_anchors_at_room_boundaries() {
        let mut level = Level::new();
        level.rooms[0] = Room::new(IVec2::new(10, 10), IVec2::new(6, 4), None, None);
        level.rooms[1] = Room::new(IVec2::new(18, 10), IVec2::new(6, 4), None, None);

        let plan = level.plan_corridor(0, 1);

        assert_eq!(plan.base_room, 0);
        assert_eq!(plan.partner_room, 1);
        assert_eq!((plan.step.x, plan.step.y), (1, 0));
        // Start on the base room's right wall; end on the partner's left wall.
        assert_eq!(plan.start.x, 15);
        assert_eq!(plan.end.x, 18);
        assert!((11..=12).contains(&plan.start.y));
        assert!((11..=12).contains(&plan.end.y));
    }
}
