//! Corridor/passage digging helpers, Rust-side per-cell flags, and the C
//! global mirroring.
//!
//! Level generation writes per-cell flags into [`LevelFlags`] and the door
//! exits of each numbered passage component into `Level::passage_links` (see
//! [`mark_passages`] and [`number_passages`]) instead of poking the C
//! `places`/`rooms`/`passages` globals directly. Once the whole level is
//! generated, `crate::level::mirror::copy_flags_to_c` /
//! `crate::level::mirror::sync_rooms_to_c` / `crate::level::mirror::sync_passages_to_c`
//! translate those Rust structures into the C arrays the engine consumes.

use std::os::raw::c_int;

use glam::IVec2;

use crate::rnd::rnd;

use super::level::{LevelFlags, LEVEL_HEIGHT, LEVEL_WIDTH};
use super::rooms::{DoorKind, Room};
use super::structure::Structure;
use super::tile::Tile;

/// Size of the C `passages` room array (also the cap on numbered components).
pub(crate) const MAX_PASSAGES: usize = 13;
/// Max exits writeable into one C `r_exit` array.
pub(crate) const MAX_EXITS: usize = 12;
/// Width of the playable C `places` screen.
pub(crate) const SCREEN_COLS: c_int = 80;
/// Height of the playable C `places` screen.
pub(crate) const SCREEN_LINES: c_int = 24;

/// A corridor connecting two rooms.
///
/// Mirrors the [`Room`](super::rooms::Room) abstraction: a bounding box
/// (`position`/`size`) plus the relative coordinates of every passage tile
/// and entry point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passage {
    pub position: IVec2,
    pub size: IVec2,
    /// Coordinates of every passage tile, relative to `position`.
    pub tiles: Vec<IVec2>,
    /// Coordinates of the doors joining adjacent rooms, relative to `position`.
    pub entry_points: Vec<IVec2>,
}

/// Door exits of one numbered passage component.
///
/// Produced by [`number_passages`] and mirrored to one slot of the C
/// `passages` array (a `CRoom` used as an exit table) by
/// `crate::level::mirror::sync_passages_to_c`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PassageLinks {
    /// Absolute map coordinates of the component's doorways.
    pub exits: Vec<IVec2>,
}

/// Geometric plan of the L-shaped corridor between two rooms.
///
/// Produced by [`plan_corridor`] and consumed by [`corridor_tiles`] to
/// compute the corridor's cells. All coordinates are absolute map coordinates
/// in the level's Rust tile grid.
pub(crate) struct CorridorPlan {
    /// Index of the room the corridor leaves (the lower room index).
    pub(crate) base_room: usize,
    /// Index of the room the corridor enters (the room paired with `base_room`).
    pub(crate) partner_room: usize,
    /// Per-cell step of the straight run: `(0, 1)` for vertical corridors,
    /// `(1, 0)` for horizontal ones.
    pub(crate) step: IVec2,
    /// Entry point on `base_room`'s boundary.
    pub(crate) start: IVec2,
    /// Exit point on `partner_room`'s boundary.
    pub(crate) end: IVec2,
    /// Number of cells laid along `step` before the turn.
    pub(crate) distance: i32,
    /// Per-cell step of the perpendicular turn.
    pub(crate) turn_step: IVec2,
    /// Number of cells laid along `turn_step`.
    pub(crate) turn_distance: i32,
    /// Position along the straight run at which the turn begins.
    pub(crate) turn_spot: i32,
}

// ---------------------------------------------------------------------------
// Corridor geometry (pure: no map or room records are mutated)
// ---------------------------------------------------------------------------

/// Linear cell index for absolute map coordinates, or `None` out of bounds.
fn cell_index(y: i32, x: i32) -> Option<usize> {
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

/// Determine the direction of the corridor between rooms `r1` and `r2`.
///
/// Rooms side by side (indices differing by one) are connected by a
/// horizontal corridor (`'r'`); rooms stacked (any other pair) by a vertical
/// corridor (`'d'`). Also returns the smaller index, which anchors the
/// corridor's start.
pub(crate) fn corridor_direction(r1: usize, r2: usize) -> (char, usize) {
    if r1 < r2 {
        let direc = if r1 + 1 == r2 { 'r' } else { 'd' };
        (direc, r1)
    } else {
        let direc = if r2 + 1 == r1 { 'r' } else { 'd' };
        (direc, r2)
    }
}

/// Pick the point where the corridor meets `rooms[room_index]`'s boundary.
///
/// For a vertical corridor (`direc == 'd'`) the point sits on the room's
/// bottom wall when `start` is set (the room the corridor leaves) or on its
/// top wall otherwise, randomizing the x coordinate. For a horizontal
/// corridor it sits on the right (`start`) or left wall, randomizing the y
/// coordinate. In maze rooms the point is redrawn until it lands on an
/// existing passage so the corridor always joins the maze. If the room was
/// removed ([`Room::is_gone`]), its top-left corner is returned unchanged.
pub(crate) fn entry_point(
    rooms: &[Room],
    map: &Structure,
    room_index: usize,
    direc: char,
    start: bool,
) -> IVec2 {
    let room = &rooms[room_index];
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
            if !room.is_maze() || matches!(map.get(p.y as usize, p.x as usize), Some(Tile::Passage)) {
                break;
            }
        }
    }
    p
}

/// Compute the full geometric plan for a corridor between rooms `r1`/`r2`.
///
/// Determines the corridor direction from the room indices, picks random
/// entry points on both room boundaries, and derives the straight run, the
/// perpendicular turn, and the random position of the turn.
pub(crate) fn plan_corridor(rooms: &[Room], map: &Structure, r1: usize, r2: usize) -> CorridorPlan {
    let (direc, base_room) = corridor_direction(r1, r2);
    let partner_room = if direc == 'd' { base_room + 3 } else { base_room + 1 };

    let step = if direc == 'd' {
        IVec2::new(0, 1)
    } else {
        IVec2::new(1, 0)
    };

    let start = entry_point(rooms, map, base_room, direc, true);
    let end = entry_point(rooms, map, partner_room, direc, false);

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

/// Compute the absolute map coordinates of the corridor described by `plan`.
///
/// Walks an L-shaped path: from `plan.start` it steps along `plan.step`
/// for `plan.distance` cells, making a perpendicular run of
/// `plan.turn_distance` cells starting at `plan.turn_spot`, so the
/// corridor ends up aligned with `plan.end`. A final check panics if the
/// path did not reach the expected end point (a geometry/setup bug rather
/// than a recoverable in-game condition). Pure: the level map is not
/// touched — the caller applies the returned tiles to the map later.
pub(crate) fn corridor_tiles(plan: &CorridorPlan) -> Vec<IVec2> {
    let mut tiles = Vec::new();
    let mut curr = plan.start;
    let mut distance = plan.distance;

    while distance > 0 {
        curr.x += plan.step.x;
        curr.y += plan.step.y;

        if distance == plan.turn_spot {
            let mut remaining = plan.turn_distance;
            while remaining > 0 {
                tiles.push(IVec2::new(curr.x, curr.y));
                curr.x += plan.turn_step.x;
                curr.y += plan.turn_step.y;
                remaining -= 1;
            }
        }

        tiles.push(IVec2::new(curr.x, curr.y));
        distance -= 1;
    }

    curr.x += plan.step.x;
    curr.y += plan.step.y;
    assert!(
        curr.x == plan.end.x && curr.y == plan.end.y,
        "connectivity problem on this level: corridor ended at ({}, {}) instead of ({}, {})",
        curr.x,
        curr.y,
        plan.end.x,
        plan.end.y,
    );
    tiles
}

/// Record one end of a corridor at `pos` on `rooms[room_index]`.
///
/// Pure: records `pos` into `tiles` always, and into `entry_points` only
/// when the room is still present (so the end will be a door when the
/// passage is applied). If the room was removed (`ISGONE`), the end will
/// be stamped as a plain passage tile instead.
pub(crate) fn collect_corridor_end(
    rooms: &[Room],
    room_index: usize,
    pos: IVec2,
    tiles: &mut Vec<IVec2>,
    entry_points: &mut Vec<IVec2>,
) {
    tiles.push(pos);
    if !rooms[room_index].is_gone() {
        entry_points.push(pos);
    }
}

/// Wrap the corridor's generated geometry into a [`Passage`] model.
///
/// Takes the absolute tile and entry-point coordinates generated for a
/// corridor, computes their bounding box, and stores the resulting
/// [`Passage`] with its coordinates made relative to the bounding box origin.
/// Returns the model so the caller can apply it to the level map (see
/// [`apply_passage`]).
pub(crate) fn build_passage(tiles: Vec<IVec2>, entry_points: Vec<IVec2>) -> Option<Passage> {
    if tiles.is_empty() {
        return None;
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

    Some(Passage {
        position,
        size,
        tiles: relative_tiles,
        entry_points: relative_entry_points,
    })
}

// ---------------------------------------------------------------------------
// Map stamping
// ---------------------------------------------------------------------------

/// Stamp a passage tile at absolute map position `pos`.
///
/// Marks the cell as [`Tile::Passage`] in the level map so it becomes part of
/// the canonical grid (mirrored to the C `places` grid by
/// `crate::level::mirror::copy_flags_to_c`).
pub(crate) fn stamp_passage(map: &mut Structure, flags: &mut LevelFlags, pos: IVec2) {
    let (y, x) = (pos.y, pos.x);
    if let Some(idx) = cell_index(y, x) {
        map.set(y as usize, x as usize, Tile::Passage);
        flags.passage[idx] = true;
    }
}

/// Place a door at `pos` on the boundary of `rooms[room_index]`.
///
/// Registers `pos` as an exit of the room and, unless the room is a maze,
/// places a door on the room itself (see [`Room::place_door`]). The door's
/// kind (open `+` or a wall segment depending on depth and randomness) is
/// decided here. Returns `pos` so the caller can record it both as a passage
/// tile and as an entry point of the current corridor.
pub(crate) fn stamp_door(
    map: &mut Structure,
    flags: &mut LevelFlags,
    rooms: &mut [Room],
    room_index: usize,
    pos: IVec2,
    depth: i32,
) -> IVec2 {
    let (is_maze, position, size) = {
        let room = &rooms[room_index];
        (room.is_maze(), room.position, room.size)
    };

    if is_maze {
        rooms[room_index].add_entry_point(pos - position);
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
    if let Some(idx) = cell_index(pos.y, pos.x) {
        if kind == DoorKind::Open {
            map.set(pos.y as usize, pos.x as usize, Tile::Door);
        } else {
            // A wall-segment door stays disguised as a wall in the tile map
            // (rendered `-`/`|` like the wall it replaces) and is marked
            // non-real so the C side can reveal it as `+`.
            map.set(pos.y as usize, pos.x as usize, Tile::HiddenDoor);
            flags.real[idx] = false;
        }
    }
    rooms[room_index].place_door(local, kind);

    pos
}

/// Copy a [`Passage`] model into the level tile map and room records.
///
/// Registers a door on the owning room for every entry point (see
/// [`stamp_door`]) and stamps the remaining corridor cells as passages (see
/// [`stamp_passage`]).
pub(crate) fn apply_passage(
    map: &mut Structure,
    flags: &mut LevelFlags,
    rooms: &mut [Room],
    passage: &Passage,
    plan: &CorridorPlan,
    depth: i32,
) {
    // Doors at the corridor's ends (only entry points reach here — ends on
    // gone rooms were never recorded as entry points).
    for rel in &passage.entry_points {
        let abs = *rel + passage.position;
        let room_index = if abs == plan.start { plan.base_room } else { plan.partner_room };
        stamp_door(map, flags, rooms, room_index, abs, depth);
    }

    // The remaining corridor cells become plain passage tiles.
    for rel in &passage.tiles {
        if !passage.entry_points.contains(rel) {
            stamp_passage(map, flags, *rel + passage.position);
        }
    }
}

// ---------------------------------------------------------------------------
// Marking and numbering the passage network
// ---------------------------------------------------------------------------

/// Mark `map`'s passage cells on the Rust flag grids.
///
/// Sets `passage` on every passage tile so [`number_passages`] and the C-side
/// screen redraw (`crate::level::redraw::add_pass`) can find it. Matching the
/// legacy `putpass`, a cell is occasionally hidden by clearing `real` so it
/// renders as a wall glyph (`-`/`|`) instead of `#`. Pure Rust: the C
/// `places` grid is only written later by `crate::level::mirror::copy_flags_to_c`.
pub(crate) fn mark_passages(map: &Structure, flags: &mut LevelFlags, depth: i32) {
    for y in 0..map.height() {
        for x in 0..map.width() {
            if !matches!(map.get(y, x), Some(Tile::Passage)) {
                continue;
            }
            let idx = y * LEVEL_WIDTH + x;
            flags.passage[idx] = true;
            if rnd(10) + 1 < depth && rnd(40) == 0 {
                flags.real[idx] = false;
            }
        }
    }
}

/// Scan state for [`number_passages`].
///
/// Wraps the flood-fill bookkeeping that the legacy C `passnum`/`numpass`
/// kept in the `PNUM`/`NEW_PNUM` globals: the current passage component
/// number and whether the next reached cell opens a new component.
struct PassageScan {
    /// Current passage component number; 0 before any component is opened.
    num: usize,
    /// Whether the next unnumbered cell reached should open a new component.
    pending_start: bool,
}

impl PassageScan {
    fn new() -> Self {
        Self {
            num: 0,
            pending_start: false,
        }
    }

    /// Mark that the next unnumbered cell starts a new passage component.
    fn open_component(&mut self) {
        self.pending_start = true;
    }
}

/// Number the contiguous passage networks reachable from every room exit.
///
/// Flood-fills from each room's entry points using [`number_passage`],
/// assigning every contiguous component a number stored in `flags.passnum`
/// and a door-exit table in `links` (index-aligned with the C `passages`
/// array, copied over by `crate::level::mirror::sync_passages_to_c`).
pub(crate) fn number_passages(
    map: &Structure,
    flags: &mut LevelFlags,
    rooms: &[Room],
    links: &mut Vec<PassageLinks>,
) {
    links.clear();
    let mut scan = PassageScan::new();
    // Collect absolute entry-point seeds up front so the flood-fill can
    // borrow `flags` mutably while iterating.
    let seeds: Vec<IVec2> = rooms
        .iter()
        .flat_map(|room| room.entry_points.iter().map(|ep| ep + room.position))
        .collect();
    for seed in seeds {
        scan.open_component();
        number_passage(map, flags, links, &mut scan, seed.x, seed.y);
    }
}

/// Recursively flood-fill one passage network, numbering its cells.
///
/// Stops at the screen edge, already-numbered cells, or tiles that are
/// neither passages nor doors, then recurses into the four neighbours. Each
/// numbered component collects its doorways into `links` and stamps its
/// component number into `flags.passnum`.
fn number_passage(
    map: &Structure,
    flags: &mut LevelFlags,
    links: &mut Vec<PassageLinks>,
    scan: &mut PassageScan,
    y: i32,
    x: i32,
) {
    if x >= SCREEN_COLS || x < 0 || y >= SCREEN_LINES || y <= 0 {
        return;
    }

    let idx = (y as usize) * LEVEL_WIDTH + (x as usize);
    if flags.passnum[idx] != 0 {
        return;
    }

    if scan.pending_start {
        scan.num += 1;
        scan.pending_start = false;
        if scan.num <= MAX_PASSAGES {
            links.resize(scan.num, PassageLinks::default());
        }
    }

    let tile = map.get(y as usize, x as usize);
    let is_door = tile == Some(Tile::Door) || tile == Some(Tile::HiddenDoor);
    if is_door {
        if let Some(links) = links.get_mut(scan.num - 1) {
            // Capped at the size of the C `r_exit` table.
            if links.exits.len() < MAX_EXITS {
                links.exits.push(IVec2::new(x, y));
            }
        }
    } else if !flags.passage[idx] {
        return;
    }

    flags.passnum[idx] = (scan.num as u8) & 0x0f;
    number_passage(map, flags, links, scan, y + 1, x);
    number_passage(map, flags, links, scan, y - 1, x);
    number_passage(map, flags, links, scan, y, x + 1);
    number_passage(map, flags, links, scan, y, x - 1);
}