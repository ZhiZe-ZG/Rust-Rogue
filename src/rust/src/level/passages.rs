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
/// (`position`/`size`) plus a [`Structure`] holding all passage tiles, and
/// the entry points where the corridor joins rooms (relative to `position`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passage {
    pub position: IVec2,
    pub size: IVec2,
    pub structure: Structure,
    pub entry_points: Vec<IVec2>,
}

impl Default for Passage {
    fn default() -> Self {
        Self {
            position: IVec2::ZERO,
            size: IVec2::ZERO,
            structure: Structure::new(0, 0, Tile::Empty),
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

    /// Place `tile` at `(local_y, local_x)` inside this passage's structure.
    pub fn place_tile(&mut self, local_y: usize, local_x: usize, tile: Tile) -> bool {
        self.structure.set(local_y, local_x, tile)
    }
}

/// Absolute coordinates of the current passage being built.
///
/// `putpass`/`door` accumulate here as the corridor is dug; when `conn`
/// finishes, the bounding box is computed and wrapped into a [`Passage`].
static mut CURRENT_TILES: Vec<IVec2> = Vec::new();
static mut CURRENT_ENTRY_POINTS: Vec<IVec2> = Vec::new();

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

/// Dig a single corridor between two adjacent rooms `r1` and `r2`.
///
/// Picks a vertical (`d`) or horizontal (`r`) corridor from the room
/// layout, chooses a random entry point on each room boundary, then walks
/// a straight or L-shaped path, laying passage tiles via [`putpass`] and
/// doors via [`door`]. The accumulated tiles are wrapped into a
/// [`Passage`] by [`finish_passage`].
/// Uses globals: `CURRENT_TILES`, `CURRENT_ENTRY_POINTS`, `rooms`, `places`.
unsafe fn conn(r1: c_int, r2: c_int) {
    let mut rmt: c_int = 0;
    let mut distance = 0;
    let turn_spot;
    let mut turn_distance = 0;
    let mut direc = 'd';
    let rm: usize;

    let mut del = CCoord { x: 0, y: 0 };
    let mut curr = CCoord { x: 0, y: 0 };

    let mut turn_delta = CCoord { x: 0, y: 0 };
    let mut spos = CCoord { x: 0, y: 0 };
    let mut epos = CCoord { x: 0, y: 0 };

    // Start a fresh passage; `putpass`/`door` append to the accumulator
    // statics as the corridor is dug. Accessed through raw pointers to avoid
    // creating references to the mutable statics (matches the codebase's
    // FFI-driven style).
    *std::ptr::addr_of_mut!(CURRENT_TILES) = Vec::new();
    *std::ptr::addr_of_mut!(CURRENT_ENTRY_POINTS) = Vec::new();

    if r1 < r2 {
        rm = r1 as usize;
        if r1 + 1 == r2 {
            direc = 'r';
        }
    } else {
        rm = r2 as usize;
        if r2 + 1 == r1 {
            direc = 'r';
        }
    }

    let rpf = &mut rooms[rm];

    if direc == 'd' {
        rmt = rm as c_int + 3;
        let rpt = &mut rooms[rmt as usize];
        del.x = 0;
        del.y = 1;
        spos.x = rpf.r_pos.x;
        spos.y = rpf.r_pos.y;
        epos.x = rpt.r_pos.x;
        epos.y = rpt.r_pos.y;
        if (rpf.r_flags & ISGONE) == 0 {
            loop {
                spos.x = rpf.r_pos.x + rnd(rpf.r_max.x - 2) + 1;
                spos.y = rpf.r_pos.y + rpf.r_max.y - 1;
                if (rpf.r_flags & ISMAZE) == 0 || (flat_at(spos.y, spos.x) as u8 & F_PASS as u8) != 0 {
                    break;
                }
            }
        }
        if (rpt.r_flags & ISGONE) == 0 {
            loop {
                epos.x = rpt.r_pos.x + rnd(rpt.r_max.x - 2) + 1;
                if (rpt.r_flags & ISMAZE) == 0 || (flat_at(epos.y, epos.x) as u8 & F_PASS as u8) != 0 {
                    break;
                }
            }
        }
        distance = (spos.y - epos.y).abs() - 1;
        turn_delta.y = 0;
        turn_delta.x = if spos.x < epos.x { 1 } else { -1 };
        turn_distance = (spos.x - epos.x).abs();
    } else if direc == 'r' {
        rmt = rm as c_int + 1;
        let rpt = &mut rooms[rmt as usize];
        del.x = 1;
        del.y = 0;
        spos.x = rpf.r_pos.x;
        spos.y = rpf.r_pos.y;
        epos.x = rpt.r_pos.x;
        epos.y = rpt.r_pos.y;
        if (rpf.r_flags & ISGONE) == 0 {
            loop {
                spos.x = rpf.r_pos.x + rpf.r_max.x - 1;
                spos.y = rpf.r_pos.y + rnd(rpf.r_max.y - 2) + 1;
                if (rpf.r_flags & ISMAZE) == 0 || (flat_at(spos.y, spos.x) as u8 & F_PASS as u8) != 0 {
                    break;
                }
            }
        }
        if (rpt.r_flags & ISGONE) == 0 {
            loop {
                epos.y = rpt.r_pos.y + rnd(rpt.r_max.y - 2) + 1;
                if (rpt.r_flags & ISMAZE) == 0 || (flat_at(epos.y, epos.x) as u8 & F_PASS as u8) != 0 {
                    break;
                }
            }
        }
        distance = (spos.x - epos.x).abs() - 1;
        turn_delta.y = if spos.y < epos.y { 1 } else { -1 };
        turn_delta.x = 0;
        turn_distance = (spos.y - epos.y).abs();
    }

    if distance > 1 {
        turn_spot = rnd(distance - 1) + 1;
    } else {
        turn_spot = 1;
    }

    if (rpf.r_flags & ISGONE) == 0 {
        door(rpf, &mut spos);
    } else {
        putpass(&mut spos);
    }

    let rpt = &rooms[rmt as usize];
    if (rpt.r_flags & ISGONE) == 0 {
        door(&raw const rooms[rmt as usize] as *mut CRoom, &mut epos);
    } else {
        putpass(&mut epos);
    }

    curr.x = spos.x;
    curr.y = spos.y;
    while distance > 0 {
        curr.x += del.x;
        curr.y += del.y;
        if distance == turn_spot {
            let mut remaining = turn_distance;
            while remaining > 0 {
                putpass(&mut curr);
                curr.x += turn_delta.x;
                curr.y += turn_delta.y;
                remaining -= 1;
            }
        }
        putpass(&mut curr);
        distance -= 1;
    }

    curr.x += del.x;
    curr.y += del.y;
    if !coord_eq(curr, epos) {
        msg(b"warning, connectivity problem on this level\0".as_ptr() as *const c_char);
    }

    finish_passage();
}

/// Wrap the tiles accumulated by [`conn`] into a [`Passage`].
///
/// Takes the tiles and entry points collected in the static accumulators,
/// computes their bounding box, lays them into a [`Structure`], and stores
/// the resulting [`Passage`] on the current level.
/// Uses globals: `CURRENT_TILES`, `CURRENT_ENTRY_POINTS`.
unsafe fn finish_passage() {
    let tiles = std::mem::take(&mut *std::ptr::addr_of_mut!(CURRENT_TILES));
    let entry_points = std::mem::take(&mut *std::ptr::addr_of_mut!(CURRENT_ENTRY_POINTS));

    if tiles.is_empty() {
        return;
    }

    let min_x = tiles.iter().map(|p| p.x).min().unwrap_or(0);
    let max_x = tiles.iter().map(|p| p.x).max().unwrap_or(0);
    let min_y = tiles.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = tiles.iter().map(|p| p.y).max().unwrap_or(0);

    let width = (max_x - min_x + 1) as usize;
    let height = (max_y - min_y + 1) as usize;
    let position = IVec2::new(min_x, min_y);
    let size = IVec2::new(max_x - min_x + 1, max_y - min_y + 1);

    let mut structure = Structure::new(height, width, Tile::Empty);
    for pos in &tiles {
        let _ = structure.set((pos.y - min_y) as usize, (pos.x - min_x) as usize, Tile::Passage);
    }
    for pos in &entry_points {
        let _ = structure.set((pos.y - min_y) as usize, (pos.x - min_x) as usize, Tile::Door);
    }

    let relative_entry_points = entry_points
        .into_iter()
        .map(|p| IVec2::new(p.x - min_x, p.y - min_y))
        .collect();

    let passage = Passage {
        position,
        size,
        structure,
        entry_points: relative_entry_points,
    };
    super::current_level_mut().add_passage(passage);
}

/// Place a passage tile at `cp`.
///
/// Records the coordinate in `CURRENT_TILES` so [`finish_passage`] can
/// reconstruct the corridor, marks the cell as a passage (`F_PASS`), and
/// occasionally renders it as a real wall (`-`/`|`) instead of `#`.
/// Uses globals: `CURRENT_TILES`, `places`, `level`.
pub(super) unsafe fn putpass(cp: *mut CCoord) {
    if cp.is_null() {
        return;
    }

    (*std::ptr::addr_of_mut!(CURRENT_TILES)).push(IVec2::new((*cp).x, (*cp).y));

    let pp = place_at((&raw mut places) as *mut CPlace, (*cp).y, (*cp).x);

    (*pp).p_flags = (((*pp).p_flags as u8) | (F_PASS as u8)) as c_char;
    if rnd(10) + 1 < level && rnd(40) == 0 {
        clear_flat_flag((*cp).y, (*cp).x, F_REAL);
    } else {
        (*pp).p_ch = PASSAGE;
    }
}

/// Place a door at `cp` on the boundary of room `rm`.
///
/// Records the coordinate both as a passage tile and as an entry point of
/// the current corridor, registers it as an exit of the room, and draws a
/// `+` door or a real wall segment depending on depth and randomness.
/// Uses globals: `CURRENT_TILES`, `CURRENT_ENTRY_POINTS`, `places`, `level`.
unsafe fn door(rm: *mut CRoom, cp: *mut CCoord) {
    if rm.is_null() || cp.is_null() {
        return;
    }

    let rm_ref = &mut *rm;
    rm_ref.r_exit[rm_ref.r_nexits as usize] = *cp;
    rm_ref.r_nexits += 1;

    let pos = IVec2::new((*cp).x, (*cp).y);
    (*std::ptr::addr_of_mut!(CURRENT_TILES)).push(pos);
    (*std::ptr::addr_of_mut!(CURRENT_ENTRY_POINTS)).push(pos);

    if (rm_ref.r_flags & ISMAZE) != 0 {
        return;
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
