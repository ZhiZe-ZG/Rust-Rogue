//! In-memory dungeon level state.
//!
//! The `Level` type tracks the depth, room layouts, passages, and tile map
//! for the current dungeon level, plus the process-wide singleton holding
//! the live level.

use std::os::raw::c_char;

use glam::IVec2;

use crate::rnd::rnd;

use super::passages::{mark_passages, passnum, sync_rooms_to_c, CorridorPlan, Passage};
use super::roomgraph::{RoomGraph, MAX_ROOMS};
use super::rooms::{build_generated_rooms, DoorKind, Room};
use super::structure::Structure;
use super::tile::Tile;

unsafe extern "C" {
    fn msg(fmt: *const c_char, ...);
}

/// Map height in cells. Matches the C `places` grid (32 rows), the largest
/// on-screen area a dungeon level can occupy.
pub const LEVEL_HEIGHT: usize = 32;
/// Map width in cells. Matches the C `places` grid (80 columns).
pub const LEVEL_WIDTH: usize = 80;

/// Per-cell flat-flag data for the level.
///
/// Mirrors the bits carried by the C `places` grid's `p_flags` field while
/// level generation runs, so no C globals need to be touched until the whole
/// level is finalized and copied over by [`copy_flags_to_c`]
/// (see [`super::passages`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelFlags {
    /// `false` marks a non-real (secret) wall or door cell.
    pub real: Vec<bool>,
    /// `true` marks a passage (`#`) cell.
    pub passage: Vec<bool>,
    /// `true` marks a cell already drawn by [`add_pass`](super::passages::add_pass).
    pub seen: Vec<bool>,
    /// Passage component number (0-15) assigned by `passnum`.
    pub passnum: Vec<u8>,
}

impl LevelFlags {
    fn cleared() -> Self {
        let cells = LEVEL_HEIGHT * LEVEL_WIDTH;
        Self {
            real: vec![true; cells],
            passage: vec![false; cells],
            seen: vec![false; cells],
            passnum: vec![0; cells],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub depth: i32,
    pub rooms: Vec<Room>,
    pub room_graph: RoomGraph,
    pub passages: Vec<Passage>,
    pub map: Structure,
    pub flags: LevelFlags,
}

impl Level {
    pub fn new() -> Self {
        Self {
            depth: 0,
            rooms: (0..MAX_ROOMS)
                .map(|_| Room::new(IVec2::ZERO, IVec2::ZERO))
                .collect(),
            room_graph: RoomGraph::new(),
            passages: Vec::new(),
            map: Structure::new(LEVEL_HEIGHT, LEVEL_WIDTH, Tile::Empty),
            flags: LevelFlags::cleared(),
        }
    }

    /// Linear cell index for absolute map coordinates, or `None` out of bounds.
    fn cell_index(&self, y: i32, x: i32) -> Option<usize> {
        if y < 0 || x < 0 {
            return None;
        }
        let (y, x) = (y as usize, x as usize);
        if y < LEVEL_HEIGHT && x < LEVEL_WIDTH {
            Some(y * LEVEL_WIDTH + x)
        } else {
            None
        }
    }

    /// Reset every flag grid to a fresh-level state.
    pub(crate) fn reset_flags(&mut self) {
        self.flags = LevelFlags::cleared();
    }

    /// Stamp a passage tile at absolute map position `pos`.
    ///
    /// Marks the cell as [`Tile::Passage`] in the level map so it becomes
    /// part of the canonical grid (mirrored to the C `places` grid by
    /// `copy_flags_to_c`). Returns `pos` so callers can record it both as
    /// a tile of the current corridor and, when applicable, an entry point.
    fn putpass(&mut self, pos: IVec2) -> IVec2 {
        let (y, x) = (pos.y, pos.x);
        if let Some(idx) = self.cell_index(y, x) {
            self.map.set(y as usize, x as usize, Tile::Passage);
            self.flags.passage[idx] = true;
        }
        pos
    }

    /// Place a door at `pos` on the boundary of `self.rooms[room_index]`.
    ///
    /// Registers `pos` as an exit of the room and, unless the room is a maze,
    /// places a door on the room itself (see [`Room::place_door`]). The door's
    /// kind (open `+` or a wall segment depending on depth and randomness) is
    /// decided here. Returns `pos` so the caller can record it both as a
    /// passage tile and as an entry point of the current corridor.
    fn door(&mut self, room_index: usize, pos: IVec2) -> IVec2 {
        let depth = self.depth;
        let (is_maze, position, size) = {
            let room = &self.rooms[room_index];
            (room.is_maze(), room.position, room.size)
        };

        if is_maze {
            self.rooms[room_index].add_entry_point(pos - position);
            return pos;
        }

        let kind = if rnd(10) + 1 < depth && rnd(5) == 0 {
            if pos.y == position.y || pos.y == position.y + size.y - 1 {
                DoorKind::WallH
            } else {
                DoorKind::WallV
            }
        } else {
            DoorKind::Open
        };

        let local = pos - position;
        if let Some(idx) = self.cell_index(pos.y, pos.x) {
            if kind == DoorKind::Open {
                self.map.set(pos.y as usize, pos.x as usize, Tile::Door);
            } else {
                self.flags.real[idx] = false;
            }
        }
        self.rooms[room_index].place_door(local, kind);

        pos
    }

    /// Place one end of a corridor on the boundary of `self.rooms[room_index]`.
    ///
    /// If the room is still present, a door is registered on its boundary via
    /// [`Level::door`]; if it was removed (`ISGONE`), a plain passage tile is
    /// laid instead (see [`Level::putpass`]). The placed coordinate is
    /// recorded into `tiles` (and into `entry_points` for doors) so the
    /// caller can reconstruct the corridor as a [`Passage`].
    fn place_corridor_end(
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
    fn dig_corridor(&mut self, plan: &CorridorPlan, tiles: &mut Vec<IVec2>) {
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
    fn entry_point(&self, room_index: usize, direc: char, start: bool) -> IVec2 {
        let room = &self.rooms[room_index];
        let mut p = room.position;
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
    fn corridor_direction(r1: usize, r2: usize) -> (char, usize) {
        if r1 < r2 {
            let direc = if r1 + 1 == r2 { 'r' } else { 'd' };
            (direc, r1)
        } else {
            let direc = if r2 + 1 == r1 { 'r' } else { 'd' };
            (direc, r2)
        }
    }

    /// Compute the full geometric plan for a corridor between rooms `r1`/`r2`.
    ///
    /// Determines the corridor direction from the room indices, picks random
    /// entry points on both room boundaries, and derives the straight run, the
    /// perpendicular turn, and the random position of the turn.
    fn plan_corridor(&self, r1: usize, r2: usize) -> CorridorPlan {
        let (direc, base_room) = Self::corridor_direction(r1, r2);
        let partner_room = if direc == 'd' { base_room + 3 } else { base_room + 1 };

        let step = if direc == 'd' {
            IVec2::new(0, 1)
        } else {
            IVec2::new(1, 0)
        };

        let start = self.entry_point(base_room, direc, true);
        let end = self.entry_point(partner_room, direc, false);

        let (distance, turn_step, turn_distance) = if direc == 'd' {
            (
                (start.y - end.y).abs() - 1,
                IVec2::new(if start.x < end.x { 1 } else { -1 }, 0),
                (start.x - end.x).abs(),
            )
        } else {
            (
                (start.x - end.x).abs() - 1,
                IVec2::new(0, if start.y < end.y { 1 } else { -1 }),
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

    /// Dig a single corridor between two adjacent rooms `r1` and `r2`.
    ///
    /// Plans an L-shaped corridor (see [`Level::plan_corridor`]), registers
    /// its doors on both room boundaries (see [`Level::place_corridor_end`]),
    /// and lays its tiles (see [`Level::dig_corridor`]). The laid tiles are
    /// collected locally and wrapped into a [`Passage`] by
    /// [`Level::finish_passage`]. All of the digging happens against this
    /// level's own rooms and tile map.
    fn conn(&mut self, r1: usize, r2: usize) {
        let mut tiles = Vec::new();
        let mut entry_points = Vec::new();

        let plan = self.plan_corridor(r1, r2);

        self.place_corridor_end(plan.base_room, plan.start, &mut tiles, &mut entry_points);
        self.place_corridor_end(plan.partner_room, plan.end, &mut tiles, &mut entry_points);

        self.dig_corridor(&plan, &mut tiles);

        self.finish_passage(tiles, entry_points);
    }

    /// Dig all corridors that connect the rooms of this level.
    ///
    /// Consumes the room-connection plan recorded in this level's room graph:
    /// for each pair, [`Level::conn`] digs an actual corridor into the level
    /// map. The level then draws its registered doors onto the C `places`
    /// grid, mirrors the rooms' entry points and the map's passage tiles back
    /// to C, and finally numbers the resulting passage network.
    /// Uses globals: `places` (via [`Level::draw_doors`]),
    /// `rooms`/`passages` (via `sync_rooms_to_c`/`passnum`).
    pub(crate) fn do_passages(&mut self) {
        let connections = self.room_graph.connections().to_vec();
        for (r1, r2) in &connections {
            self.conn(*r1, *r2);
        }

        unsafe { sync_rooms_to_c(self) };
        mark_passages(self);

        passnum(self);
    }

    /// Wrap the tiles laid while digging a corridor into a [`Passage`].
    ///
    /// Takes the tile and entry-point coordinates collected while digging the
    /// corridor, computes their bounding box, and stores the resulting
    /// [`Passage`] with its coordinates made relative to the bounding box
    /// origin.
    fn finish_passage(&mut self, tiles: Vec<IVec2>, entry_points: Vec<IVec2>) {
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

static mut CURRENT_LEVEL: Option<Level> = None;

pub unsafe fn current_level_mut() -> &'static mut Level {
    if CURRENT_LEVEL.is_none() {
        CURRENT_LEVEL = Some(Level::new());
    }
    CURRENT_LEVEL.as_mut().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::raw::c_int;

    /// Test-only definition of the C `msg` symbol.
    ///
    /// Test builds link without the C engine, so `dig_corridor`'s connectivity
    /// warning (which calls the variadic C `msg`) needs a local symbol. The
    /// non-variadic stub matches the single-argument call site; its body is
    /// never reached in the current tests.
    #[no_mangle]
    extern "C" fn msg(_fmt: *const c_char) -> c_int {
        0
    }

    /// A door placed through [`Level::door`] is recorded on the room and both
    /// the entry point and the tile map reflect it.
    #[test]
    fn door_records_on_room_and_stamps_tile_map() {
        let mut level = Level::new();
        level.depth = 1;
        level.rooms[0] = Room::new(IVec2::new(10, 10), IVec2::new(6, 4));

        // `door` decides the kind randomly (depth 1 → always open).
        let pos = level.door(0, IVec2::new(15, 11));

        assert_eq!(pos, IVec2::new(15, 11));
        let room = &level.rooms[0];
        assert_eq!(room.doors.len(), 1);
        assert_eq!(room.doors[0].position, IVec2::new(5, 1));
        assert_eq!(room.doors[0].kind, DoorKind::Open);
        assert_eq!(room.entry_point_count, 1);
        // Open doors are stamped into the tile map.
        assert_eq!(level.map.get(11, 15), Some(Tile::Door));
    }

    /// Generate a level with a fixed depth and verify that every active
    /// room's tile model was stamped into the level map.
    #[test]
    fn generation_stamps_rooms_into_map() {
        let mut level = Level::new();
        level.depth = 1;

        // 9 room slots with default geometry (0 size → skipped as gone).
        let rooms = std::array::from_fn(|_| Room::new(IVec2::ZERO, IVec2::ZERO));
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

    /// Stamping a passage tile records it in the level map and flag grids.
    #[test]
    fn putpass_stamps_passage_into_map() {
        let mut level = Level::new();
        let pos = level.putpass(IVec2::new(5, 7));

        assert_eq!(pos, IVec2::new(5, 7));
        assert_eq!(level.map.get(7, 5), Some(Tile::Passage));
        assert!(level.flags.passage[7 * LEVEL_WIDTH + 5]);
        // Passage placement clears no real-wall flag.
        assert!(level.flags.real[7 * LEVEL_WIDTH + 5]);
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
        level.rooms[0] = Room::new(IVec2::new(10, 10), IVec2::new(6, 4));
        level.rooms[1] = Room::new(IVec2::new(18, 10), IVec2::new(6, 4));

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

    /// `conn` digs a corridor between two side-by-side rooms, registering
    /// doors on both boundaries and storing one passage with both entry
    /// points reachable.
    #[test]
    fn conn_digs_corridor_between_adjacent_rooms() {
        let mut level = Level::new();
        level.depth = 1;
        level.rooms[0] = Room::new(IVec2::new(10, 10), IVec2::new(6, 4));
        level.rooms[1] = Room::new(IVec2::new(18, 10), IVec2::new(6, 4));

        level.conn(0, 1);

        // A passage was recorded.
        assert_eq!(level.passages.len(), 1);
        let passage = &level.passages[0];
        assert_eq!(passage.entry_points.len(), 2);

        // Doors were registered on both rooms' boundaries.
        assert_eq!(level.rooms[0].doors.len(), 1);
        assert_eq!(level.rooms[1].doors.len(), 1);
        assert_eq!(level.rooms[0].entry_point_count, 1);
        assert_eq!(level.rooms[1].entry_point_count, 1);

        // The interior passage tiles were stamped into the level map and the
        // Rust flag grids as passages (the door cells are `Tile::Door`).
        let interior = passage.tiles.len() - passage.entry_points.len();
        let mut count = 0;
        for y in 0..LEVEL_HEIGHT {
            for x in 0..LEVEL_WIDTH {
                if matches!(level.map.get(y, x), Some(Tile::Passage)) {
                    count += 1;
                }
                if level.flags.passage[y * LEVEL_WIDTH + x] {
                    assert_eq!(
                        level.map.get(y, x),
                        Some(Tile::Passage),
                        "passage flag set on non-passage cell ({y},{x})"
                    );
                }
            }
        }
        assert_eq!(count, interior, "expected {interior} interior passage tiles, found {count}");

        // The two entry points lie on the two rooms' facing walls.
        for ep in &passage.entry_points {
            let abs = *ep + passage.position;
            assert!(
                (abs.x == 15 && (11..=12).contains(&abs.y))
                    || (abs.x == 18 && (11..=12).contains(&abs.y)),
                "entry point {abs:?} not on a facing wall"
            );
        }
    }
}