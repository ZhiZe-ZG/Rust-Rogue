//! Room-connection adjacency graph.
//!
//! The legacy C engine kept a per-level `rdes` array describing which of the
//! nine rooms are geometrically adjacent and which of those connections have
//! actually been dug. This module provides a pure-Rust [`RoomGraph`] that owns
//! room slots, fixed adjacency, and per-level connection state so level
//! generation can run without C globals.

use std::os::raw::c_int;

use crate::rnd::rnd;
use glam::IVec2;

use super::rooms::Room;

/// Maximum number of rooms on a level.
pub const MAX_ROOMS: usize = 9;

const NUMCOLS: i32 = 80;
const NUMLINES: i32 = 24;
const MAX_ROOM_TRIES: usize = 100;

type AdjacentArray = [[u8; MAX_ROOMS]; MAX_ROOMS];

/// Room grid plus adjacency and connection state for one generation pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomGraph {
    rooms: [Room; MAX_ROOMS],
    adjacent: AdjacentArray,
    isconn: AdjacentArray,
    ingraph: [u8; MAX_ROOMS],
    connections: Vec<(usize, usize)>,
}

impl Default for RoomGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl RoomGraph {
    /// Build an empty graph with default room slots.
    pub fn new() -> Self {
        Self::with_rooms_and_adjacency(empty_rooms(), build_base_adjacency())
    }

    /// Build and populate room layout/flags for one level generation pass.
    pub(crate) fn for_level(rooms: [Room; MAX_ROOMS], bsze: IVec2, depth: i32) -> Self {
        let mut graph = Self::with_rooms_and_adjacency(rooms, build_base_adjacency());
        graph.determine_room_layouts(bsze, depth);
        graph
    }

    pub(crate) fn into_rooms(self) -> [Room; MAX_ROOMS] {
        self.rooms
    }

    pub(crate) fn connections(&self) -> &[(usize, usize)] {
        &self.connections
    }

    pub(crate) fn generate_connections_for_rooms(&mut self) {
        self.connections = self.generate_for_rooms();
    }

    /// Reset per-level connection state, keeping the fixed adjacency.
    pub fn reset(&mut self) {
        self.isconn = [[0; MAX_ROOMS]; MAX_ROOMS];
        self.ingraph = [0; MAX_ROOMS];
        self.connections.clear();
    }

    /// Create a graph around pre-existing room slots and adjacency.
    fn with_rooms_and_adjacency(rooms: [Room; MAX_ROOMS], adjacent: AdjacentArray) -> Self {
        Self {
            rooms,
            adjacent,
            isconn: [[0; MAX_ROOMS]; MAX_ROOMS],
            ingraph: [0; MAX_ROOMS],
            connections: Vec::new(),
        }
    }

    fn determine_room_layouts(&mut self, bsze: IVec2, depth: i32) {
        // Reset per-room state before generating the level layout.
        for room in &mut self.rooms {
            room.goldval = 0;
            room.entry_point_count = 0;
            room.clear_flags();
        }

        // Randomly mark a few rooms as removed for this level.
        for _ in 0..rnd(4) {
            self.rooms[pick_non_gone(&self.rooms)].mark_gone();
        }

        // Compute geometry, sizes, and flags for every room slot.
        for i in 0..MAX_ROOMS {
            let top = grid_top_left(i, bsze);
            let room = &mut self.rooms[i];

            if room.is_gone() {
                place_off_map_room(room, top, bsze);
                continue;
            }

            if rnd(10) < depth - 1 {
                room.mark_dark();
                if rnd(15) == 0 {
                    room.set_maze();
                }
            }

            if room.is_maze() {
                place_maze_room(room, top, bsze);
            } else {
                place_regular_room(room, top, bsze);
            }
        }
    }

    /// Mark `room` as part of the connected passage graph.
    fn mark_in_graph(&mut self, room: usize) {
        self.ingraph[room] = 1;
    }

    /// Record that a passage between `a` and `b` has been dug.
    fn connect(&mut self, a: usize, b: usize) {
        self.isconn[a][b] = 1;
        self.isconn[b][a] = 1;
    }

    /// Pick a uniformly random adjacent room that is not yet in the graph.
    fn next_unreached(&self, from: usize) -> Option<usize> {
        pick_unconnected_adjacent(&self.adjacent[from], &self.ingraph)
    }

    /// Pick a uniformly random adjacent room with no dug connection yet.
    fn next_unconnected(&self, from: usize) -> Option<usize> {
        pick_unconnected_adjacent(&self.adjacent[from], &self.isconn[from])
    }

    /// Pick a uniformly random room already part of the graph.
    fn pick_in_graph(&self) -> usize {
        loop {
            let idx = random_room_index();
            if self.ingraph[idx] != 0 {
                return idx;
            }
        }
    }

    /// Decide which room pairs get connected, using this graph's room layout.
    ///
    /// This treats "gone" rooms as pass-through cells in the 3x3 grid so
    /// remaining rooms can still be connected through them, but only requires
    /// non-gone rooms to be fully reachable in the spanning stage.
    pub(crate) fn generate_for_rooms(&self) -> Vec<(usize, usize)> {
        let mut graph = self.clone();
        let mut connections = Vec::new();
        let room_states = &self.rooms;

        let non_gone_total = non_gone_count(room_states);
        if non_gone_total <= 1 {
            return connections;
        }

        // Grow passages until all non-gone rooms are reachable.
        let mut reached_non_gone = 1;
        let mut r1_idx = pick_non_gone(room_states);
        graph.mark_in_graph(r1_idx);

        while reached_non_gone < non_gone_total {
            if let Some(idx) = graph.next_unreached(r1_idx) {
                graph.mark_in_graph(idx);
                if !room_states[idx].is_gone() {
                    reached_non_gone += 1;
                }
                connections.push((r1_idx, idx));
                graph.connect(r1_idx, idx);
                r1_idx = idx;
            } else {
                r1_idx = graph.pick_in_graph();
            }
        }

        // Add a few extra connecting passages for loopiness.
        let mut extra = rnd(5);
        while extra > 0 {
            let r1_idx = pick_non_gone(room_states);
            if let Some(idx) = graph.next_unconnected(r1_idx) {
                connections.push((r1_idx, idx));
                graph.connect(r1_idx, idx);
            }
            extra -= 1;
        }

        connections
    }
}

fn empty_rooms() -> [Room; MAX_ROOMS] {
    std::array::from_fn(|_| Room::new(IVec2::ZERO, IVec2::ZERO))
}

/// Top-left corner of the 3x3 grid cell that room `i` belongs to.
fn grid_top_left(i: usize, bsze: IVec2) -> IVec2 {
    IVec2::new((i as i32 % 3) * bsze.x + 1, (i as i32 / 3) * bsze.y)
}

fn build_base_adjacency() -> AdjacentArray {
    let mut adjacent = [[0; MAX_ROOMS]; MAX_ROOMS];

    for idx in 0..MAX_ROOMS {
        let row = idx / 3;
        let col = idx % 3;

        if col > 0 {
            adjacent[idx][idx - 1] = 1;
        }
        if col < 2 {
            adjacent[idx][idx + 1] = 1;
        }
        if row > 0 {
            adjacent[idx][idx - 3] = 1;
        }
        if row < 2 {
            adjacent[idx][idx + 3] = 1;
        }
    }

    adjacent
}

/// Randomly move a removed room's top-left corner off the visible map.
fn place_off_map_room(room: &mut Room, top: IVec2, bsze: IVec2) {
    // Keep rerolling until the off-map placeholder position is valid.
    loop {
        room.position.x = top.x + rnd(bsze.x - 2) + 1;
        room.position.y = top.y + rnd(bsze.y - 2) + 1;
        room.size = IVec2::new(-NUMCOLS, -NUMLINES);
        if room.position.y > 0 && room.position.y < NUMLINES - 1 {
            break;
        }
    }
}

/// Size a maze room to fill its 3x3 grid cell.
fn place_maze_room(room: &mut Room, top: IVec2, bsze: IVec2) {
    room.size.x = bsze.x - 1;
    room.size.y = bsze.y - 1;
    room.position.x = top.x;
    if room.position.x == 1 {
        room.position.x = 0;
    }
    room.position.y = top.y;
    if room.position.y == 0 {
        room.position.y += 1;
        room.size.y -= 1;
    }
}

/// Try to fit a plain room in its grid cell, marking it `gone` if it never
/// lands on a valid (non-top-row) position.
fn place_regular_room(room: &mut Room, top: IVec2, bsze: IVec2) {
    for _ in 0..MAX_ROOM_TRIES {
        room.size.x = rnd(bsze.x - 4) + 4;
        room.size.y = rnd(bsze.y - 4) + 4;
        room.position.x = top.x + rnd(bsze.x - room.size.x);
        room.position.y = top.y + rnd(bsze.y - room.size.y);
        if room.position.y != 0 {
            return;
        }
    }
    room.mark_gone();
}

fn random_room_index() -> usize {
    rnd(MAX_ROOMS as c_int) as usize
}

fn non_gone_count(room_states: &[Room]) -> usize {
    room_states.iter().filter(|room| !room.is_gone()).count()
}

fn pick_non_gone(room_states: &[Room]) -> usize {
    loop {
        let idx = random_room_index();
        if !room_states[idx].is_gone() {
            return idx;
        }
    }
}

/// Pick a uniformly random index where `available` is nonzero and `blocked`
/// is zero, or `None` when no such index exists.
fn pick_unconnected_adjacent(
    available: &[u8; MAX_ROOMS],
    blocked: &[u8; MAX_ROOMS],
) -> Option<usize> {
    let mut count = 0;
    let mut pick = None;
    for i in 0..MAX_ROOMS {
        if available[i] != 0 && blocked[i] == 0 {
            count += 1;
            if rnd(count as c_int) == 0 {
                pick = Some(i);
            }
        }
    }
    pick
}
