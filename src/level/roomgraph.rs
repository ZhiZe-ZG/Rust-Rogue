//! Room-connection adjacency graph.
//!
//! The legacy C engine kept a per-level `rdes` array describing which of the
//! nine rooms are geometrically adjacent and which of those connections have
//! actually been dug. This module provides pure-Rust room-layout generation and
//! the [`RoomGraph`] connection plan so level generation can run without C
//! globals.

use crate::rnd::rnd;
use glam::IVec2;

use super::structure::Room;
use crate::config::GameConfig;

/// Rows in the fixed three-by-three room grid.
const GRID_ROWS: usize = 3;
/// Columns in the fixed three-by-three room grid.
const GRID_COLS: usize = 3;

/// Planned room-to-room passage connections for one generation pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomGraph {
    connections: Vec<(usize, usize)>,
}

impl RoomGraph {
    /// Build an empty connection plan.
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn connections(&self) -> &[(usize, usize)] {
        &self.connections
    }

    /// Plan which room pairs get connected, storing the result on this graph.
    pub(crate) fn generate_connections(&mut self, rooms: &[Room]) {
        self.connections = generate_for_rooms(rooms);
    }

    /// Reset the per-level connection plan.
    pub fn reset(&mut self) {
        self.connections.clear();
    }
}

/// Generate the room geometry, sizes, and flags for one level in place.
pub(crate) fn generate_rooms(rooms: &mut [Room], bsze: IVec2, depth: i32) {
    // Reset per-room state before generating the level layout.
    for room in rooms.iter_mut() {
        room.goldval = 0;
        room.entry_point_count = 0;
        room.clear_flags();
    }

    // Randomly mark a few rooms as removed for this level.
    for _ in 0..rnd(4) {
        rooms[pick_non_gone(rooms)].mark_gone();
    }

    // Compute geometry, sizes, and flags for every room slot.
    for i in 0..GameConfig::MAX_ROOMS {
        let top = grid_top_left(i, bsze);
        let room = &mut rooms[i];

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

/// Grow a spanning tree over the live rooms, then add a few extra passages for
/// loopiness. "Gone" rooms act as pass-through cells in the 3x3 grid, but only
/// non-gone rooms must become reachable.
fn generate_for_rooms(room_states: &[Room]) -> Vec<(usize, usize)> {
    let non_gone_total = non_gone_count(room_states);
    if non_gone_total <= 1 {
        return Vec::new();
    }

    let mut connections = Vec::new();
    let mut in_graph = [false; GameConfig::MAX_ROOMS];

    // Spanning stage: grow passages until all non-gone rooms are reachable.
    let mut reached_non_gone = 1;
    let mut r1_idx = pick_non_gone(room_states);
    in_graph[r1_idx] = true;

    while reached_non_gone < non_gone_total {
        if let Some(idx) = next_unreached(r1_idx, &in_graph) {
            in_graph[idx] = true;
            if !room_states[idx].is_gone() {
                reached_non_gone += 1;
            }
            connections.push((r1_idx, idx));
            r1_idx = idx;
        } else {
            r1_idx = pick_in_graph(&in_graph);
        }
    }

    // Add a few extra connecting passages for loopiness.
    let mut extra = rnd(5);
    while extra > 0 {
        let r1_idx = pick_non_gone(room_states);
        if let Some(idx) = next_unconnected(r1_idx, &connections) {
            connections.push((r1_idx, idx));
        }
        extra -= 1;
    }

    connections
}

/// Pick a uniformly random adjacent room reachable from `from` that is not yet
/// part of the connected graph.
fn next_unreached(from: usize, in_graph: &[bool; GameConfig::MAX_ROOMS]) -> Option<usize> {
    pick_unconnected(neighbors(from), |room| in_graph[room])
}

/// Pick a uniformly random adjacent room reachable from `from` that has no dug
/// connection to `from` yet.
fn next_unconnected(from: usize, connections: &[(usize, usize)]) -> Option<usize> {
    pick_unconnected(neighbors(from), |room| is_connected(connections, from, room))
}

/// Pick a uniformly random room already part of the connected graph.
fn pick_in_graph(in_graph: &[bool; GameConfig::MAX_ROOMS]) -> usize {
    loop {
        let idx = random_room_index();
        if in_graph[idx] {
            return idx;
        }
    }
}

fn is_connected(connections: &[(usize, usize)], a: usize, b: usize) -> bool {
    connections.contains(&(a, b)) || connections.contains(&(b, a))
}

/// Pick a uniformly random candidate that is not blocked, or `None` when every
/// candidate is blocked.
fn pick_unconnected(
    candidates: impl IntoIterator<Item = usize>,
    is_blocked: impl Fn(usize) -> bool,
) -> Option<usize> {
    let mut count = 0;
    let mut pick = None;
    for room in candidates {
        if is_blocked(room) {
            continue;
        }
        count += 1;
        if rnd(count) == 0 {
            pick = Some(room);
        }
    }
    pick
}

/// Top-left corner of the grid cell that room `i` belongs to.
fn grid_top_left(i: usize, bsze: IVec2) -> IVec2 {
    IVec2::new(
        (i as i32 % GRID_COLS as i32) * bsze.x + 1,
        (i as i32 / GRID_COLS as i32) * bsze.y,
    )
}

/// Indexes of the rooms orthogonally adjacent to `room`, in ascending order.
fn neighbors(room: usize) -> Vec<usize> {
    let row = room / GRID_COLS;
    let col = room % GRID_COLS;
    let mut result = Vec::with_capacity(4);

    if row > 0 {
        result.push(room - GRID_COLS);
    }
    if col > 0 {
        result.push(room - 1);
    }
    if col + 1 < GRID_COLS {
        result.push(room + 1);
    }
    if row + 1 < GRID_ROWS {
        result.push(room + GRID_COLS);
    }

    result
}

/// Randomly move a removed room's top-left corner off the visible map.
fn place_off_map_room(room: &mut Room, top: IVec2, bsze: IVec2) {
    // Keep rerolling until the off-map placeholder position is valid.
    loop {
        room.position.x = top.x + rnd(bsze.x - 2) + 1;
        room.position.y = top.y + rnd(bsze.y - 2) + 1;
        room.size = IVec2::new(-GameConfig::SCREEN_COLS, -GameConfig::SCREEN_LINES);
        if room.position.y > 0 && room.position.y < GameConfig::SCREEN_LINES - 1 {
            break;
        }
    }
}

/// Size a maze room to fill its grid cell.
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
    for _ in 0..GameConfig::MAX_ROOM_PLACEMENT_ATTEMPTS {
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
    rnd(GameConfig::MAX_ROOMS as i32) as usize
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