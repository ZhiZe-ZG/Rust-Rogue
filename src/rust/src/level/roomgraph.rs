//! Room-connection adjacency graph.
//!
//! The legacy engine keeps a per-level `rdes` array describing which of the
//! nine rooms are geometrically adjacent and which of those connections have
//! actually been dug. This module provides a pure-Rust [`RoomGraph`] wrapper
//! around that data so the level generator can work with a self-contained
//! adjacency structure, while still exposing the C-visible `rdes` global for
//! any C code that still reads it. The graph is copied back into `rdes` when
//! level creation finishes.

use std::os::raw::c_int;
use std::os::raw::c_uchar;

use crate::rnd::rnd;

/// Maximum number of rooms on a level.
pub const MAX_ROOMS: usize = 9;

/// A single room descriptor in the legacy `rdes` array.
///
/// `conn` holds which rooms are adjacent in the 3x3 grid, `isconn` records
/// which of those connections have been dug, and `ingraph` marks whether the
/// room is part of the connected passage graph.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RDes {
    pub conn: [c_uchar; MAX_ROOMS],
    pub isconn: [c_uchar; MAX_ROOMS],
    pub ingraph: c_uchar,
}

/// Fixed adjacency of the 3x3 room grid (each room is adjacent to its
/// orthogonal neighbours in the grid).
const BASE_CONN: [[c_uchar; MAX_ROOMS]; MAX_ROOMS] = [
    [0, 1, 0, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 1, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 1, 0, 0, 0],
    [1, 0, 0, 0, 1, 0, 1, 0, 0],
    [0, 1, 0, 1, 0, 1, 0, 1, 0],
    [0, 0, 1, 0, 1, 0, 0, 0, 1],
    [0, 0, 0, 1, 0, 0, 0, 1, 0],
    [0, 0, 0, 0, 1, 0, 1, 0, 1],
    [0, 0, 0, 0, 0, 1, 0, 1, 0],
];

/// C-visible global mirroring the current level's room graph.
///
/// Kept exported with the C ABI so legacy C code that reads `rdes` still
/// finds it. The Rust generator works through [`RoomGraph`] and copies the
/// result back here when level creation finishes.
#[no_mangle]
pub static mut rdes: [RDes; MAX_ROOMS] = [
    RDes { conn: [0, 1, 0, 1, 0, 0, 0, 0, 0], isconn: [0; MAX_ROOMS], ingraph: 0 },
    RDes { conn: [1, 0, 1, 0, 1, 0, 0, 0, 0], isconn: [0; MAX_ROOMS], ingraph: 0 },
    RDes { conn: [0, 1, 0, 0, 0, 1, 0, 0, 0], isconn: [0; MAX_ROOMS], ingraph: 0 },
    RDes { conn: [1, 0, 0, 0, 1, 0, 1, 0, 0], isconn: [0; MAX_ROOMS], ingraph: 0 },
    RDes { conn: [0, 1, 0, 1, 0, 1, 0, 1, 0], isconn: [0; MAX_ROOMS], ingraph: 0 },
    RDes { conn: [0, 0, 1, 0, 1, 0, 0, 0, 1], isconn: [0; MAX_ROOMS], ingraph: 0 },
    RDes { conn: [0, 0, 0, 1, 0, 0, 0, 1, 0], isconn: [0; MAX_ROOMS], ingraph: 0 },
    RDes { conn: [0, 0, 0, 0, 1, 0, 1, 0, 1], isconn: [0; MAX_ROOMS], ingraph: 0 },
    RDes { conn: [0, 0, 0, 0, 0, 1, 0, 1, 0], isconn: [0; MAX_ROOMS], ingraph: 0 },
];

/// Pure-Rust abstraction of the room-connection adjacency graph.
///
/// Owns the fixed grid adjacency plus the per-level connection state, and
/// exposes the same random-growth operations the legacy `do_passages` used
/// directly on the `rdes` array.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomGraph {
    conn: [[c_uchar; MAX_ROOMS]; MAX_ROOMS],
    isconn: [[c_uchar; MAX_ROOMS]; MAX_ROOMS],
    ingraph: [c_uchar; MAX_ROOMS],
}

impl Default for RoomGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl RoomGraph {
    /// Build a fresh graph for the fixed 3x3 room grid.
    pub fn new() -> Self {
        Self {
            conn: BASE_CONN,
            isconn: [[0; MAX_ROOMS]; MAX_ROOMS],
            ingraph: [0; MAX_ROOMS],
        }
    }

    /// Reset per-level connection state, keeping the fixed adjacency.
    pub fn reset(&mut self) {
        self.isconn = [[0; MAX_ROOMS]; MAX_ROOMS];
        self.ingraph = [0; MAX_ROOMS];
    }

    /// Mark `room` as part of the connected passage graph.
    pub fn mark_in_graph(&mut self, room: usize) {
        self.ingraph[room] = 1;
    }

    /// Whether `room` is part of the connected passage graph.
    pub fn is_in_graph(&self, room: usize) -> bool {
        self.ingraph[room] != 0
    }

    /// Whether a passage between `a` and `b` has already been dug.
    pub fn is_connected(&self, a: usize, b: usize) -> bool {
        self.isconn[a][b] != 0
    }

    /// Record that a passage between `a` and `b` has been dug.
    pub fn connect(&mut self, a: usize, b: usize) {
        self.isconn[a][b] = 1;
        self.isconn[b][a] = 1;
    }

    /// Pick a uniformly random adjacent room that is not yet in the graph.
    ///
    /// Returns `None` when every adjacent room is already in the graph.
    pub fn next_unreached(&self, from: usize) -> Option<usize> {
        let mut count = 0;
        let mut pick = None;
        for i in 0..MAX_ROOMS {
            if self.conn[from][i] != 0 && self.ingraph[i] == 0 {
                count += 1;
                if rnd(count as c_int) == 0 {
                    pick = Some(i);
                }
            }
        }
        pick
    }

    /// Pick a uniformly random adjacent room with no dug connection yet.
    ///
    /// Returns `None` when every adjacent room is already connected.
    pub fn next_unconnected(&self, from: usize) -> Option<usize> {
        let mut count = 0;
        let mut pick = None;
        for i in 0..MAX_ROOMS {
            if self.conn[from][i] != 0 && self.isconn[from][i] == 0 {
                count += 1;
                if rnd(count as c_int) == 0 {
                    pick = Some(i);
                }
            }
        }
        pick
    }

    /// Pick a uniformly random room that is already part of the graph.
    pub fn pick_in_graph(&self) -> usize {
        loop {
            let idx = rnd(MAX_ROOMS as c_int) as usize;
            if self.ingraph[idx] != 0 {
                return idx;
            }
        }
    }

    /// Pick a uniformly random room index (0..MAX_ROOMS).
    pub fn pick_any(&self) -> usize {
        rnd(MAX_ROOMS as c_int) as usize
    }

    /// Copy this graph back into the C-visible `rdes` global.
    ///
    /// # Safety
    ///
    /// Writes to the shared `static mut rdes`; callers must ensure no other
    /// code is reading or writing it concurrently.
    pub unsafe fn write_to_c(&self) {
        let rdes_ptr = std::ptr::addr_of_mut!(rdes);
        for i in 0..MAX_ROOMS {
            (*rdes_ptr)[i].conn = self.conn[i];
            (*rdes_ptr)[i].isconn = self.isconn[i];
            (*rdes_ptr)[i].ingraph = self.ingraph[i];
        }
    }
}
