//! Room-connection adjacency graph.
//!
//! The legacy C engine kept a per-level `rdes` array describing which of the
//! nine grid slots are geometrically adjacent and which of those connections
//! have actually been dug. This module keeps that plan entirely in Rust:
//! [`RoomGraph`] records which room pairs get passages, computed by
//! [`plan_connections`], which grows a spanning tree over the live rooms and
//! then adds a few extra links for loopiness.

use crate::rnd::rnd;

use super::structure::Room;
use crate::config::GameConfig;

/// Rows in the fixed three-by-three room grid.
const GRID_ROWS: usize = 3;
/// Columns in the fixed three-by-three room grid.
pub(crate) const GRID_COLS: usize = 3;

/// Upper bound for the number of extra passage links beyond the spanning tree.
const EXTRA_CONNECTION_ROLLS: i32 = 5;

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

/// Plan which room pairs get connected.
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

/// Pick a uniformly random room slot that has not been removed for the level.
pub(crate) fn pick_non_gone(rooms: &[Room]) -> usize {
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
        std::array::from_fn(|_| Room::new(glam::IVec2::ZERO, glam::IVec2::new(4, 4)))
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