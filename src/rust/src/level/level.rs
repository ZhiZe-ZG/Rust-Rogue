//! In-memory dungeon level state.
//!
//! The `Level` type tracks the depth, room layouts, passages, and tile map
//! for the current dungeon level, plus the process-wide singleton holding
//! the live level.

use glam::IVec2;

use super::passages::{
    apply_passage, build_passage, collect_corridor_end, corridor_tiles, mark_passages,
    number_passages, plan_corridor, stamp_door, stamp_passage, Passage, PassageLinks,
};
use super::roomgraph::{RoomGraph, MAX_ROOMS};
use super::rooms::{build_generated_rooms, Room};
use super::structure::Structure;
use super::tile::Tile;

/// Map height in cells. Matches the C `places` grid (32 rows), the largest
/// on-screen area a dungeon level can occupy.
pub const LEVEL_HEIGHT: usize = 32;
/// Map width in cells. Matches the C `places` grid (80 columns).
pub const LEVEL_WIDTH: usize = 80;

/// Per-cell flat-flag data for the level.
///
/// Mirrors the bits carried by the C `places` grid's `p_flags` field while
/// level generation runs, so no C globals need to be touched until the whole
/// level is finalized and copied over by `copy_flags_to_c`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelFlags {
    /// `false` marks a non-real (secret) wall or door cell.
    pub real: Vec<bool>,
    /// `true` marks a passage (`#`) cell.
    pub passage: Vec<bool>,
    /// `true` marks a cell already drawn by `add_pass`.
    pub seen: Vec<bool>,
    /// Passage component number (0-15) assigned by `number_passages`.
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
    /// Door exits of each numbered passage component, index-aligned with the
    /// C `passages` array and copied over by `sync_passages_to_c`.
    pub passage_links: Vec<PassageLinks>,
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
            passage_links: Vec::new(),
        }
    }

    /// Reset every flag grid to a fresh-level state.
    pub fn reset_flags(&mut self) {
        self.flags = LevelFlags::cleared();
    }

    /// Dig a single corridor between two adjacent rooms `r1` and `r2`.
    ///
    /// Works in three phases: first the corridor geometry is generated purely
    /// (see [`plan_corridor`], [`collect_corridor_end`], and
    /// [`corridor_tiles`]) and modelled as a [`Passage`] (see
    /// [`build_passage`]); only then is the model copied into this level's
    /// room records and tile map (see [`apply_passage`]).
    fn conn(&mut self, r1: usize, r2: usize) {
        let plan = plan_corridor(&self.rooms, &self.map, r1, r2);

        // Phase 1 — generate the corridor geometry purely, without touching
        // the level map or room records.
        let mut tiles = Vec::new();
        let mut entry_points = Vec::new();
        collect_corridor_end(&self.rooms, plan.base_room, plan.start, &mut tiles, &mut entry_points);
        collect_corridor_end(&self.rooms, plan.partner_room, plan.end, &mut tiles, &mut entry_points);
        tiles.extend(corridor_tiles(&plan));

        // Phase 2 — build the Passage model from the collected geometry.
        let passage = match build_passage(tiles, entry_points) {
            Some(p) => p,
            None => return,
        };

        // Phase 3 — copy the model into the level map and room records.
        apply_passage(
            &mut self.map,
            &mut self.flags,
            &mut self.rooms,
            &passage,
            &plan,
            self.depth,
        );
        self.passages.push(passage);
    }

    /// Dig all corridors that connect the rooms of this level.
    ///
    /// Consumes the room-connection plan recorded in this level's room graph:
    /// for each pair, [`Level::conn`] digs an actual corridor into the level
    /// map. The level then flags and numbers the passage network on the Rust
    /// side; the C `rooms`/`passages`/`places` globals are only written later
    /// by `write_rust_data_back_to_c_and_ncurses`.
    pub fn do_passages(&mut self) {
        let connections = self.room_graph.connections().to_vec();
        for (r1, r2) in &connections {
            self.conn(*r1, *r2);
        }

        mark_passages(&self.map, &mut self.flags, self.depth);
        number_passages(&self.map, &mut self.flags, &self.rooms, &mut self.passage_links);
    }

    pub fn generate_rooms_and_connections(
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
    use std::os::raw::{c_char, c_int};

    /// Test-only definition of the C `msg` symbol.
    ///
    /// Test builds link without the C engine, so `corridor_tiles`'s
    /// connectivity warning (which calls the variadic C `msg`) needs a local
    /// symbol. The non-variadic stub matches the single-argument call site;
    /// its body is never reached in the current tests.
    #[no_mangle]
    extern "C" fn msg(_fmt: *const c_char) -> c_int {
        0
    }

    /// A door placed through [`stamp_door`] is recorded on the room and both
    /// the entry point and the tile map reflect it.
    #[test]
    fn door_records_on_room_and_stamps_tile_map() {
        let mut level = Level::new();
        level.depth = 1;
        level.rooms[0] = Room::new(IVec2::new(10, 10), IVec2::new(6, 4));

        // `stamp_door` decides the kind randomly (depth 1 → always open).
        super::stamp_door(
            &mut level.map,
            &mut level.flags,
            &mut level.rooms,
            0,
            IVec2::new(15, 11),
            level.depth,
        );

        let room = &level.rooms[0];
        assert_eq!(room.doors.len(), 1);
        assert_eq!(room.doors[0].position, IVec2::new(5, 1));
        assert_eq!(room.doors[0].kind, crate::level::rooms::DoorKind::Open);
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
        super::stamp_passage(&mut level.map, &mut level.flags, IVec2::new(5, 7));

        assert_eq!(level.map.get(7, 5), Some(Tile::Passage));
        assert!(level.flags.passage[7 * LEVEL_WIDTH + 5]);
        // Passage placement clears no real-wall flag.
        assert!(level.flags.real[7 * LEVEL_WIDTH + 5]);
    }

    /// Out-of-bounds passage placement is ignored without panicking.
    #[test]
    fn putpass_ignores_out_of_bounds_positions() {
        let mut level = Level::new();

        super::stamp_passage(&mut level.map, &mut level.flags, IVec2::new(-1, 7));
        assert_eq!(level.map.get(7, 0), Some(Tile::Empty));

        super::stamp_passage(&mut level.map, &mut level.flags, IVec2::new(5, -3));
        assert_eq!(level.map.get(0, 5), Some(Tile::Empty));
    }

    /// `build_passage` wraps generated tiles into a [`Passage`] with
    /// coordinates made relative to the bounding-box origin.
    #[test]
    fn build_passage_builds_relative_passage() {
        let passage = super::build_passage(
            vec![IVec2::new(2, 3), IVec2::new(3, 3), IVec2::new(4, 3), IVec2::new(4, 4)],
            vec![IVec2::new(2, 3)],
        )
        .expect("passage should be built");

        assert_eq!(passage.position, IVec2::new(2, 3));
        assert_eq!(passage.size, IVec2::new(3, 2));
        assert_eq!(
            passage.tiles,
            vec![IVec2::new(0, 0), IVec2::new(1, 0), IVec2::new(2, 0), IVec2::new(2, 1)]
        );
        assert_eq!(passage.entry_points, vec![IVec2::new(0, 0)]);
    }

    /// `build_passage` returns `None` for empty tile lists.
    #[test]
    fn build_passage_ignores_empty_tiles() {
        let passage = super::build_passage(Vec::new(), Vec::new());
        assert!(passage.is_none());
    }

    /// `conn` generates the corridor model first, then stamps it onto the map
    /// and room records; the stored model matches the stamped map.
    #[test]
    fn apply_passage_generates_then_stamps_map() {
        let mut level = Level::new();
        level.depth = 1;
        level.rooms[0] = Room::new(IVec2::new(10, 10), IVec2::new(6, 4));
        level.rooms[1] = Room::new(IVec2::new(18, 10), IVec2::new(6, 4));

        level.conn(0, 1);

        // Exactly one passage model exists and matches the stamped map: each
        // relative tile appears as `Tile::Passage` (or a `Tile::Door` entry
        // point) at its absolute position.
        assert_eq!(level.passages.len(), 1);
        let passage = &level.passages[0];
        for rel in &passage.tiles {
            let abs = *rel + passage.position;
            let stamped = level.map.get(abs.y as usize, abs.x as usize);
            let is_entry = passage
                .entry_points
                .iter()
                .any(|ep| *ep + passage.position == abs);
            if is_entry {
                assert_eq!(stamped, Some(Tile::Door));
            } else {
                assert_eq!(stamped, Some(Tile::Passage));
            }
        }
    }

    /// `plan_corridor` anchors the plan at the two rooms' boundaries.
    #[test]
    fn plan_corridor_anchors_at_room_boundaries() {
        let mut level = Level::new();
        level.rooms[0] = Room::new(IVec2::new(10, 10), IVec2::new(6, 4));
        level.rooms[1] = Room::new(IVec2::new(18, 10), IVec2::new(6, 4));

        let plan = super::plan_corridor(&level.rooms, &level.map, 0, 1);

        assert_eq!(plan.base_room, 0);
        assert_eq!(plan.partner_room, 1);
        assert_eq!((plan.step.x, plan.step.y), (1, 0));
        // Start on the base room's right wall; end on the partner's left wall.
        assert_eq!(plan.start.x, 15);
        assert_eq!(plan.end.x, 18);
        assert!((11..=12).contains(&plan.start.y));
        assert!((11..=12).contains(&plan.end.y));
    }

    /// `number_passages` flood-fills the corridor from each room entry point,
    /// assigning a component number and collecting the door exits into
    /// `passage_links`.
    #[test]
    fn number_passages_collects_door_exits() {
        let mut level = Level::new();
        level.depth = 10;
        level.rooms[0] = Room::new(IVec2::new(10, 10), IVec2::new(6, 4));
        level.rooms[1] = Room::new(IVec2::new(18, 10), IVec2::new(6, 4));

        // Dig the corridor first so the map/flags are populated.
        level.conn(0, 1);
        super::mark_passages(&level.map, &mut level.flags, level.depth);
        super::number_passages(&level.map, &mut level.flags, &level.rooms, &mut level.passage_links);

        // One connected component exists with both facing-wall doors.
        assert_eq!(level.passage_links.len(), 1);
        let links = &level.passage_links[0];

        // Every exit must be a door cell on the map.
        let passage = &level.passages[0];
        assert_eq!(links.exits.len(), passage.entry_points.len());
        for exit in &links.exits {
            assert_eq!(level.map.get(exit.y as usize, exit.x as usize), Some(Tile::Door));
        }

        // Every interior passage tile carries component number 1.
        for y in 0..LEVEL_HEIGHT {
            for x in 0..LEVEL_WIDTH {
                if level.flags.passage[y * LEVEL_WIDTH + x] {
                    assert_eq!(level.flags.passnum[y * LEVEL_WIDTH + x], 1);
                }
            }
        }
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