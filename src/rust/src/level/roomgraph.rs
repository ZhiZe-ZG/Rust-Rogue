//! Room-connection adjacency graph.
//!
//! The legacy C engine kept a per-level `rdes` array describing which of the
//! nine rooms are geometrically adjacent and which of those connections have
//! actually been dug. This module provides a pure-Rust [`RoomGraph`] wrapper
//! around that data so the level generator can work with a self-contained
//! adjacency structure. No C code consumes `rdes` anymore, so the graph is
//! purely a Rust-side abstraction.

use std::os::raw::c_int;
use std::os::raw::c_uchar;

use crate::rnd::rnd;

/// Maximum number of rooms on a level.
pub const MAX_ROOMS: usize = 9;

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

/// Pure-Rust abstraction of the room-connection adjacency graph.
///
/// Owns the fixed grid adjacency plus the per-level connection state, and
/// exposes the same random-growth operations the legacy `do_passages` used
/// directly on the `rdes` array. [`RoomGraph::generate`] decides which room
/// pairs get connected; the caller is responsible for actually digging each
/// corridor.
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

    /// Decide which room pairs get connected on this level.
    ///
    /// Grows a connected spanning tree of the room graph, then adds a few
    /// extra connecting corridors so the maze isn't a pure tree. Returns the
    /// list of room pairs in the order they should be dug; the caller is
    /// responsible for actually digging each corridor.
    pub fn generate(&self) -> Vec<(usize, usize)> {
        let mut graph = self.clone();
        let mut connections = Vec::new();

        // Grow a connected spanning tree of the room graph.
        let mut roomcount = 1;
        let mut r1_idx = graph.pick_any();
        graph.mark_in_graph(r1_idx);

        loop {
            if let Some(idx) = graph.next_unreached(r1_idx) {
                graph.mark_in_graph(idx);
                connections.push((r1_idx, idx));
                graph.connect(r1_idx, idx);
                roomcount += 1;
            } else {
                r1_idx = graph.pick_in_graph();
            }

            if roomcount >= MAX_ROOMS as c_int {
                break;
            }
        }

        // Add a few extra connecting passages so the maze isn't a pure tree.
        let mut roomcount = rnd(5);
        while roomcount > 0 {
            let r1_idx = graph.pick_any();
            if let Some(idx) = graph.next_unconnected(r1_idx) {
                connections.push((r1_idx, idx));
                graph.connect(r1_idx, idx);
            }
            roomcount -= 1;
        }

        connections
    }
}
