use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::draw::place_at;
use crate::player::{CCoord, CPlace, CRoom, CThingMonster};

use glam::IVec2;

use super::structure::Structure;
use super::tile::Tile;

const MAXROOMS: usize = 9;
const MAXPASS: usize = 13;
const NUMCOLS: c_int = 80;
const NUMLINES: c_int = 24;

const ISGONE: c_short = 0o000002;
const ISMAZE: c_short = 0o000004;

const PASSAGE: c_char = b'#' as c_char;
const DOOR: c_char = b'+' as c_char;
const F_PASS: c_char = 0x80u8 as c_char;
const F_REAL: c_char = 0x10u8 as c_char;
const F_PNUM: c_char = 0x0fu8 as c_char;
const F_SEEN: c_char = 0x40u8 as c_char;

const FALSE: c_uchar = 0;
const TRUE: c_uchar = 1;

/// A corridor connecting two rooms.
///
/// Mirrors the [`Room`](super::rooms::Room) abstraction: a bounding box
/// (`position`/`size`) plus the relative coordinates of every passage tile
/// and entry point. Use [`Passage::to_structure`] to derive the tile
/// [`Structure`] from those coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passage {
    pub position: IVec2,
    pub size: IVec2,
    /// Coordinates of every passage tile, relative to `position`.
    pub tiles: Vec<IVec2>,
    /// Coordinates of the doors joining adjacent rooms, relative to `position`.
    pub entry_points: Vec<IVec2>,
}

impl Default for Passage {
    fn default() -> Self {
        Self {
            position: IVec2::ZERO,
            size: IVec2::ZERO,
            tiles: Vec::new(),
            entry_points: Vec::new(),
        }
    }
}

impl Passage {
    /// Register `relative_pos` as an entry point where this corridor joins a
    /// room. Entry points are stored relative to the passage's `position`.
    pub fn add_entry_point(&mut self, relative_pos: IVec2) {
        self.entry_points.push(relative_pos);
    }

    /// Record a passage tile coordinate, relative to the passage's `position`.
    pub fn add_tile(&mut self, relative_pos: IVec2) {
        self.tiles.push(relative_pos);
    }

    /// Build the tile [`Structure`] described by this passage.
    ///
    /// Returns a `size`-sized grid with every recorded tile laid as
    /// `Tile::Passage` and every entry point laid as `Tile::Door`.
    pub fn to_structure(&self) -> Structure {
        let height = self.size.y as usize;
        let width = self.size.x as usize;
        let mut structure = Structure::new(height, width, Tile::Empty);
        for pos in &self.tiles {
            let _ = structure.set(pos.y as usize, pos.x as usize, Tile::Passage);
        }
        for pos in &self.entry_points {
            let _ = structure.set(pos.y as usize, pos.x as usize, Tile::Door);
        }
        structure
    }
}

/// Number of the passage currently being scanned by [`passnum`]/[`numpass`].
static mut PNUM: c_int = 0;

/// Whether the next cell reached by [`numpass`] starts a new passage number.
static mut NEW_PNUM: c_uchar = FALSE;

unsafe extern "C" {
    static mut level: c_int;
    static mut rooms: [CRoom; MAXROOMS];
    static mut passages: [CRoom; MAXPASS];
    static mut places: [CPlace; 32 * 80];

    fn rnd(range: c_int) -> c_int;
    fn msg(fmt: *const c_char, ...);
    fn r#move(y: c_int, x: c_int) -> c_int;
    fn addch(ch: c_uint) -> c_int;
    fn standout() -> c_int;
    fn standend() -> c_int;
}

/// Read the character at `(y, x)` from the C `places` grid.
/// Uses globals: `places`.
#[inline]
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_ch
}

/// Read the flat flags at `(y, x)` from the C `places` grid.
/// Uses globals: `places`.
#[inline]
unsafe fn flat_at(y: c_int, x: c_int) -> c_char {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_flags
}

/// Clear `flag` from the flat flags of the `places` cell at `(y, x)`.
/// Uses globals: `places`.
#[inline]
unsafe fn clear_flat_flag(y: c_int, x: c_int, flag: c_char) {
    let pp = place_at((&raw mut places) as *mut CPlace, y, x);
    (*pp).p_flags = (((*pp).p_flags as u8) & !(flag as u8)) as c_char;
}

/// Whether two coordinates are equal.
/// Uses globals: none.
#[inline]
unsafe fn coord_eq(a: CCoord, b: CCoord) -> bool {
    a.x == b.x && a.y == b.y
}

/// Dig all corridors that connect the rooms of the current level.
///
/// `connections` lists the room pairs to connect, as produced by
/// `RoomGraph::generate` (which the caller is responsible for invoking).
/// For each pair, [`conn`] digs an actual corridor into the C map.
/// Finally [`passnum`] numbers the resulting passage network.
/// Uses globals: none directly.
pub(super) unsafe fn do_passages(connections: &[(usize, usize)]) {
    for (r1, r2) in connections {
        conn(*r1 as c_int, *r2 as c_int);
    }

    passnum();
}

/// Geometric plan of the L-shaped corridor between two rooms.
///
/// Produced by [`plan_corridor`] and consumed by [`conn`] to register the
/// corridor's doors and lay its tiles. All coordinates are absolute C-map
/// coordinates.
struct CorridorPlan {
    /// Index of the room the corridor leaves (the lower room index).
    base_room: usize,
    /// Index of the room the corridor enters (the room paired with `base_room`).
    partner_room: usize,
    /// Per-cell step of the straight run: `(0, 1)` for vertical corridors,
    /// `(1, 0)` for horizontal ones.
    step: CCoord,
    /// Entry point on `base_room`'s boundary.
    start: CCoord,
    /// Exit point on `partner_room`'s boundary.
    end: CCoord,
    /// Number of cells laid along `step` before the turn.
    distance: c_int,
    /// Per-cell step of the perpendicular turn.
    turn_step: CCoord,
    /// Number of cells laid along `turn_step`.
    turn_distance: c_int,
    /// Position along the straight run at which the turn begins.
    turn_spot: c_int,
}

/// Determine the direction of the corridor between rooms `r1` and `r2`.
///
/// Rooms side by side (indices differing by one) are connected by a
/// horizontal corridor (`'r'`); rooms stacked (any other pair) by a
/// vertical corridor (`'d'`). Also returns the smaller index, which anchors
/// the corridor's start.
/// Uses globals: none.
fn corridor_direction(r1: c_int, r2: c_int) -> (char, usize) {
    if r1 < r2 {
        let direc = if r1 + 1 == r2 { 'r' } else { 'd' };
        (direc, r1 as usize)
    } else {
        let direc = if r2 + 1 == r1 { 'r' } else { 'd' };
        (direc, r2 as usize)
    }
}

/// Pick the point where the corridor meets `room`'s boundary.
///
/// For a vertical corridor (`direc == 'd'`) the point sits on the room's
/// bottom wall when `start` is set (the room the corridor leaves) or on its
/// top wall otherwise, randomizing the x coordinate. For a horizontal
/// corridor it sits on the right (`start`) or left wall, randomizing the y
/// coordinate. In maze rooms the point is redrawn until it lands on an
/// existing passage so the corridor always joins the maze. If the room was
/// removed (`ISGONE`), its top-left corner is returned unchanged.
/// Uses globals: `places`.
unsafe fn entry_point(room: &CRoom, direc: char, start: bool) -> CCoord {
    let mut p = CCoord {
        x: room.r_pos.x,
        y: room.r_pos.y,
    };
    if (room.r_flags & ISGONE) == 0 {
        loop {
            if direc == 'd' {
                p.x = room.r_pos.x + rnd(room.r_max.x - 2) + 1;
                p.y = if start { room.r_pos.y + room.r_max.y - 1 } else { room.r_pos.y };
            } else {
                p.y = room.r_pos.y + rnd(room.r_max.y - 2) + 1;
                p.x = if start { room.r_pos.x + room.r_max.x - 1 } else { room.r_pos.x };
            }
            if (room.r_flags & ISMAZE) == 0 || (flat_at(p.y, p.x) as u8 & F_PASS as u8) != 0 {
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
/// Uses globals: `rooms`, `places`.
unsafe fn plan_corridor(r1: c_int, r2: c_int) -> CorridorPlan {
    let (direc, base_room) = corridor_direction(r1, r2);
    let partner_room = if direc == 'd' { base_room + 3 } else { base_room + 1 };

    let base = &rooms[base_room];
    let partner = &rooms[partner_room];

    let step = if direc == 'd' {
        CCoord { x: 0, y: 1 }
    } else {
        CCoord { x: 1, y: 0 }
    };

    let start = entry_point(base, direc, true);
    let end = entry_point(partner, direc, false);

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

/// Place one end of the corridor on `room`'s boundary.
///
/// If the room is still present, a door is registered on its boundary via
/// [`door`]; if it was removed (`ISGONE`), a plain passage tile is laid
/// instead. The placed coordinate is recorded into `tiles` (and into
/// `entry_points` for doors) so [`finish_passage`] can reconstruct the
/// corridor.
/// Uses globals: `rooms`, `places`, `level`.
unsafe fn place_corridor_end(
    room: *mut CRoom,
    pos: &mut CCoord,
    tiles: &mut Vec<IVec2>,
    entry_points: &mut Vec<IVec2>,
) {
    if room.is_null() || ((*room).r_flags & ISGONE) != 0 {
        tiles.push(putpass(pos));
    } else {
        let door_pos = door(room, pos);
        tiles.push(door_pos);
        entry_points.push(door_pos);
    }
}

/// Lay the passage tiles of the corridor described by `plan`.
///
/// Walks an L-shaped path: from `start` it steps along `step` for
/// `distance` cells, making a perpendicular run of `turn_distance` cells
/// starting at `turn_spot`, so the corridor ends up aligned with `end`. A
/// final check warns if the path did not reach the expected end point.
/// Every laid tile is recorded into `tiles`.
/// Uses globals: `places`, `level`.
unsafe fn dig_corridor(plan: &CorridorPlan, tiles: &mut Vec<IVec2>) {
    let mut curr = plan.start;
    let mut distance = plan.distance;

    while distance > 0 {
        curr.x += plan.step.x;
        curr.y += plan.step.y;

        if distance == plan.turn_spot {
            let mut remaining = plan.turn_distance;
            while remaining > 0 {
                tiles.push(putpass(&mut curr));
                curr.x += plan.turn_step.x;
                curr.y += plan.turn_step.y;
                remaining -= 1;
            }
        }

        tiles.push(putpass(&mut curr));
        distance -= 1;
    }

    curr.x += plan.step.x;
    curr.y += plan.step.y;
    if !coord_eq(curr, plan.end) {
        msg(b"warning, connectivity problem on this level\0".as_ptr() as *const c_char);
    }
}

/// Dig a single corridor between two adjacent rooms `r1` and `r2`.
///
/// Plans an L-shaped corridor (see [`plan_corridor`]), registers its doors
/// on both room boundaries, and lays its tiles (see [`dig_corridor`]). The
/// laid tiles are collected locally and wrapped into a [`Passage`] by
/// [`finish_passage`].
/// Uses globals: `rooms`, `places`.
unsafe fn conn(r1: c_int, r2: c_int) {
    let mut tiles = Vec::new();
    let mut entry_points = Vec::new();

    let mut plan = plan_corridor(r1, r2);

    place_corridor_end(&raw mut rooms[plan.base_room], &mut plan.start, &mut tiles, &mut entry_points);
    place_corridor_end(&raw mut rooms[plan.partner_room], &mut plan.end, &mut tiles, &mut entry_points);

    dig_corridor(&plan, &mut tiles);

    finish_passage(tiles, entry_points);
}

/// Wrap the tiles laid by [`conn`] into a [`Passage`].
///
/// Takes the tile and entry-point coordinates collected while digging the
/// corridor, computes their bounding box, and stores the resulting
/// [`Passage`] with its coordinates made relative to the bounding box
/// origin.
/// Uses globals: none.
unsafe fn finish_passage(tiles: Vec<IVec2>, entry_points: Vec<IVec2>) {
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
    super::current_level_mut().add_passage(passage);
}

/// Place a passage tile at `cp`.
///
/// Marks the cell as a passage (`F_PASS`) and occasionally renders it as a
/// real wall (`-`/`|`) instead of `#`. Returns the absolute coordinate of
/// the placed tile so callers that need to reconstruct the corridor can
/// record it.
/// Uses globals: `places`, `level`.
pub(super) unsafe fn putpass(cp: *mut CCoord) -> IVec2 {
    if cp.is_null() {
        return IVec2::ZERO;
    }

    let pos = IVec2::new((*cp).x, (*cp).y);

    let pp = place_at((&raw mut places) as *mut CPlace, (*cp).y, (*cp).x);

    (*pp).p_flags = (((*pp).p_flags as u8) | (F_PASS as u8)) as c_char;
    if rnd(10) + 1 < level && rnd(40) == 0 {
        clear_flat_flag((*cp).y, (*cp).x, F_REAL);
    } else {
        (*pp).p_ch = PASSAGE;
    }

    pos
}

/// Place a door at `cp` on the boundary of room `rm`.
///
/// Registers the coordinate as an exit of the room and draws a `+` door or
/// a real wall segment depending on depth and randomness. Returns the
/// absolute coordinate so the caller can record it both as a passage tile
/// and as an entry point of the current corridor.
/// Uses globals: `places`, `level`.
unsafe fn door(rm: *mut CRoom, cp: *mut CCoord) -> IVec2 {
    if rm.is_null() || cp.is_null() {
        return IVec2::ZERO;
    }

    let rm_ref = &mut *rm;
    rm_ref.r_exit[rm_ref.r_nexits as usize] = *cp;
    rm_ref.r_nexits += 1;

    let pos = IVec2::new((*cp).x, (*cp).y);

    if (rm_ref.r_flags & ISMAZE) != 0 {
        return pos;
    }

    let pp = place_at((&raw mut places) as *mut CPlace, (*cp).y, (*cp).x);

    if rnd(10) + 1 < level && rnd(5) == 0 {
        if (*cp).y == rm_ref.r_pos.y || (*cp).y == rm_ref.r_pos.y + rm_ref.r_max.y - 1 {
            (*pp).p_ch = b'-' as c_char;
        } else {
            (*pp).p_ch = b'|' as c_char;
        }
        clear_flat_flag((*cp).y, (*cp).x, F_REAL);
    } else {
        (*pp).p_ch = DOOR;
    }

    pos
}

/// Draw all passage and door tiles for the current level (FFI export).
///
/// Iterates the C `places` grid and redraws every cell marked as a passage
/// or a door, marking it seen (`F_SEEN`). Exported with `#[no_mangle]` so
/// the C engine can call it during screen redraw.
/// Uses globals: `places`.
#[no_mangle]
pub unsafe extern "C" fn add_pass() {
    for y in 1..NUMLINES - 1 {
        for x in 0..NUMCOLS {
            let pp = place_at((&raw mut places) as *mut CPlace, y, x);
            let flags = (*pp).p_flags;
            let ch = (*pp).p_ch;
            if (((flags as u8) & (F_PASS as u8)) != 0)
                || ch == DOOR
                || (((flags as u8) & (F_REAL as u8)) == 0 && (ch == b'|' as c_char || ch == b'-' as c_char))
            {
                let mut out_ch = ch;
                if ((flags as u8) & (F_PASS as u8)) != 0 {
                    out_ch = PASSAGE;
                }
                (*pp).p_flags = (((*pp).p_flags as u8) | (F_SEEN as u8)) as c_char;
                r#move(y, x);
                if !(*pp).p_monst.is_null() {
                    let monst = (*pp).p_monst as *mut CThingMonster;
                    (*monst).t_oldch = (*pp).p_ch;
                } else if ((flags as u8) & (F_REAL as u8)) != 0 {
                    addch(out_ch as c_uint);
                } else {
                    standout();
                    addch(if (flags as u8) & (F_PASS as u8) != 0 { PASSAGE as c_uint } else { DOOR as c_uint });
                    standend();
                }
            }
        }
    }
}

/// Number the passages reachable from every room exit.
///
/// Resets the passage table, then flood-fills from each room exit using
/// [`numpass`]. Every contiguous passage network is assigned a number used
/// to index the C `passages` array.
/// Uses globals: `PNUM`, `NEW_PNUM`, `passages`, `rooms`.
unsafe fn passnum() {
    PNUM = 0;
    NEW_PNUM = FALSE;
    for rp in &mut passages[..MAXPASS] {
        rp.r_nexits = 0;
    }
    for rp in &mut rooms[..MAXROOMS] {
        for i in 0..rp.r_nexits as usize {
            NEW_PNUM = TRUE;
            numpass(rp.r_exit[i].y, rp.r_exit[i].x);
        }
    }
}

/// Recursively flood-fill a passage network, numbering its cells.
///
/// Stops at the screen edge, already-numbered cells, or tiles that are
/// neither passages nor doors, then recurses into the four neighbours.
/// Each new contiguous component increments the current passage number and
/// its exits are registered in the C `passages` array.
/// Uses globals: `PNUM`, `NEW_PNUM`, `passages`, `places`.
unsafe fn numpass(y: c_int, x: c_int) {
    if x >= NUMCOLS || x < 0 || y >= NUMLINES || y <= 0 {
        return;
    }

    let pp = place_at((&raw mut places) as *mut CPlace, y, x);
    if ((*pp).p_flags as u8 & F_PNUM as u8) != 0 {
        return;
    }
    if NEW_PNUM != 0 {
        PNUM += 1;
        NEW_PNUM = FALSE;
    }

    let ch = chat_at(y, x);
    if ch == DOOR || (((flat_at(y, x) as u8) & (F_REAL as u8)) == 0 && (ch == b'|' as c_char || ch == b'-' as c_char)) {
        let rp = &mut passages[PNUM as usize];
        rp.r_exit[rp.r_nexits as usize].y = y;
        rp.r_exit[rp.r_nexits as usize].x = x;
        rp.r_nexits += 1;
    } else if ((flat_at(y, x) as u8) & (F_PASS as u8)) == 0 {
        return;
    }

    (*pp).p_flags = (((*pp).p_flags as u8) | (PNUM as u8)) as c_char;
    numpass(y + 1, x);
    numpass(y - 1, x);
    numpass(y, x + 1);
    numpass(y, x - 1);
}
