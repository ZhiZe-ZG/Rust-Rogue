//! Room layout and connection planning for one dungeon level.
//!
//! The legacy C engine kept a per-level `rdes` array describing which of the
//! nine grid slots are geometrically adjacent and which of those connections
//! have actually been dug. This module keeps that plan entirely in Rust:
//!
//! * [`generate_rooms`] places the room rectangles and assigns each slot its
//!   per-level flags (gone/dark/maze). Placement is driven by the fixed
//!   three-by-three grid described by [`grid_top_left`].
//! * [`RoomGraph`] records the passage connections produced by
//!   [`plan_connections`], which grows a spanning tree over the live rooms and
//!   then adds a few extra links for loopiness.

use crate::rnd::rnd;
use glam::IVec2;

use super::structure::Room;
use crate::config::GameConfig;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Rows in the fixed three-by-three room grid.
const GRID_ROWS: usize = 3;
/// Columns in the fixed three-by-three room grid.
const GRID_COLS: usize = 3;

/// Upper bound for the roll that removes rooms at the start of a new level.
const GONE_ROOM_ROLLS: i32 = 4;
/// Roll threshold (`rnd(10) < depth - 1`) that marks a room dark.
const DARK_ROOM_ROLL: i32 = 10;
/// One-in-N chance that a dark room becomes a maze.
const MAZE_ROOM_CHANCE: i32 = 15;
/// Upper bound for the number of extra passage links beyond the spanning tree.
const EXTRA_CONNECTION_ROLLS: i32 = 5;

// ---------------------------------------------------------------------------
// Connection plan
// ---------------------------------------------------------------------------

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

    /// Planned passage pairs, each a `(from, to)` pair of room indexes.
    pub(crate) fn connections(&self) -> &[(usize, usize)] {
        &self.connections
    }

    /// Recompute the connection plan for `rooms`, replacing any previous plan.
    pub(crate) fn generate(&mut self, rooms: &[Room]) {
        self.connections = plan_connections(rooms);
    }

    /// Discard the current per-level connection plan.
    pub fn reset(&mut self) {
        self.connections.clear();
    }
}

/// Plan which room pairs get connected, storing the result on this graph.
fn plan_connections(rooms: &[Room]) -> Vec<(usize, usize)> {
    let live_rooms = non_gone_count(rooms);
    if live_rooms <= 1 {
        return Vec::new();
    }

    let mut connections = Vec::new();
    build_spanning_tree(rooms, live_rooms, &mut connections);
    add_extra_connections(rooms, &mut connections);
    connections
}

/// Grow a spanning tree over the live rooms.
///
/// "Gone" rooms act as pass-through cells in the 3x3 grid: they may appear in
/// the tree, but only non-gone rooms must become reachable.
fn build_spanning_tree(
    rooms: &[Room],
    live_rooms: usize,
    connections: &mut Vec<(usize, usize)>,
) {
    let mut in_graph = [false; GameConfig::MAX_ROOMS];
    let mut reached = 1;
    let mut current = pick_non_gone(rooms);
    in_graph[current] = true;

    while reached < live_rooms {
        match next_unreached(current, &in_graph) {
            Some(next) => {
                in_graph[next] = true;
                if !rooms[next].is_gone() {
                    reached += 1;
                }
                connections.push((current, next));
                current = next;
            }
            None => current = pick_in_graph(&in_graph),
        }
    }
}

/// Add a few extra connecting passages for loopiness.
fn add_extra_connections(rooms: &[Room], connections: &mut Vec<(usize, usize)>) {
    let mut extra = rnd(EXTRA_CONNECTION_ROLLS);
    while extra > 0 {
        let from = pick_non_gone(rooms);
        if let Some(to) = next_unconnected(from, connections) {
            connections.push((from, to));
        }
        extra -= 1;
    }
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

// ---------------------------------------------------------------------------
// Room geometry
// ---------------------------------------------------------------------------

/// Generate the room geometry, sizes, and flags for one level in place.
pub(crate) fn generate_rooms(rooms: &mut [Room], bsze: IVec2, depth: i32) {
    reset_rooms(rooms);
    mark_random_rooms_gone(rooms);
    for slot in 0..GameConfig::MAX_ROOMS {
        layout_room(&mut rooms[slot], slot, bsze, depth);
    }
}

/// Reset per-room generation state before laying out a level.
fn reset_rooms(rooms: &mut [Room]) {
    for room in rooms {
        room.goldval = 0;
        room.entry_point_count = 0;
        room.clear_flags();
    }
}

/// Randomly mark a few room slots as removed for this level.
fn mark_random_rooms_gone(rooms: &mut [Room]) {
    for _ in 0..rnd(GONE_ROOM_ROLLS) {
        rooms[pick_non_gone(rooms)].mark_gone();
    }
}

/// Compute the geometry, size, and flags for one room slot.
fn layout_room(room: &mut Room, slot: usize, bsze: IVec2, depth: i32) {
    let top = grid_top_left(slot, bsze);

    if room.is_gone() {
        place_off_map_room(room, top, bsze);
        return;
    }

    if rnd(DARK_ROOM_ROLL) < depth - 1 {
        room.mark_dark();
        if rnd(MAZE_ROOM_CHANCE) == 0 {
            room.set_maze();
        }
    }

    if room.is_maze() {
        place_maze_room(room, top, bsze);
    } else {
        place_regular_room(room, top, bsze);
    }
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

// ---------------------------------------------------------------------------
// Grid helpers
// ---------------------------------------------------------------------------

/// Top-left corner of the grid cell that room `slot` belongs to.
fn grid_top_left(slot: usize, bsze: IVec2) -> IVec2 {
    IVec2::new(
        (slot as i32 % GRID_COLS as i32) * bsze.x + 1,
        (slot as i32 / GRID_COLS as i32) * bsze.y,
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

fn random_room_index() -> usize {
    rnd(GameConfig::MAX_ROOMS as i32) as usize
}

fn non_gone_count(rooms: &[Room]) -> usize {
    rooms.iter().filter(|room| !room.is_gone()).count()
}

fn pick_non_gone(rooms: &[Room]) -> usize {
    loop {
        let idx = random_room_index();
        if !rooms[idx].is_gone() {
            return idx;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nine live (non-gone) room slots with a small positive size.
    fn live_rooms() -> [Room; GameConfig::MAX_ROOMS] {
        std::array::from_fn(|_| Room::new(IVec2::ZERO, IVec2::new(4, 4)))
    }

    #[test]
    fn grid_top_left_maps_slots_to_cells() {
        let bsze = IVec2::new(26, 8);
        assert_eq!(grid_top_left(0, bsze), IVec2::new(1, 0));
        assert_eq!(grid_top_left(1, bsze), IVec2::new(27, 0));
        assert_eq!(grid_top_left(3, bsze), IVec2::new(1, 8));
        assert_eq!(grid_top_left(8, bsze), IVec2::new(53, 16));
    }

    #[test]
    fn neighbors_are_orthogonal_and_in_grid() {
        assert_eq!(neighbors(0), vec![1, 3]);
        assert_eq!(neighbors(2), vec![1, 5]);
        assert_eq!(neighbors(4), vec![1, 3, 5, 7]);
        assert_eq!(neighbors(8), vec![5, 7]);
    }

    #[test]
    fn plan_connections_with_single_live_room_is_empty() {
        let mut rooms = live_rooms();
        for (i, room) in rooms.iter_mut().enumerate() {
            if i != 4 {
                room.mark_gone();
            }
        }
        assert!(plan_connections(&rooms).is_empty());
    }

    #[test]
    fn plan_connections_connects_all_live_rooms() {
        let connections = plan_connections(&live_rooms());
        let live = GameConfig::MAX_ROOMS;

        // Spanning tree yields exactly |V| - 1 links; extra links are added on
        // top, bounded by EXTRA_CONNECTION_ROLLS.
        assert!(connections.len() >= live - 1);
        assert!(connections.len() <= live - 1 + EXTRA_CONNECTION_ROLLS as usize);

        // Every link joins orthogonally adjacent slots.
        for &(a, b) in &connections {
            assert!(neighbors(a).contains(&b), "{a} is not adjacent to {b}");
        }

        // The resulting graph reaches every live room.
        assert!(reaches_all_rooms(&connections, live));
    }

    #[test]
    fn room_graph_generate_and_reset() {
        let mut graph = RoomGraph::new();
        assert!(graph.connections().is_empty());

        graph.generate(&live_rooms());
        assert!(!graph.connections().is_empty());

        graph.reset();
        assert!(graph.connections().is_empty());
    }

    fn reaches_all_rooms(connections: &[(usize, usize)], room_count: usize) -> bool {
        if room_count == 0 {
            return true;
        }

        let mut visited = vec![false; room_count];
        let mut stack = vec![0];
        visited[0] = true;
        let mut seen = 1;

        while let Some(node) = stack.pop() {
            for &(a, b) in connections {
                let next = if a == node {
                    b
                } else if b == node {
                    a
                } else {
                    continue;
                };
                if !visited[next] {
                    visited[next] = true;
                    seen += 1;
                    stack.push(next);
                }
            }
        }

        seen == room_count
    }
}