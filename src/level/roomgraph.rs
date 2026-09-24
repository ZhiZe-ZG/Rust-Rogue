//! Room-connection adjacency graph.
//!
//! The legacy C engine kept a per-level `rdes` array describing which of the
//! nine grid slots are geometrically adjacent and which of those connections
//! have actually been dug. This module keeps that plan entirely in Rust:
//! [`RoomGraph`] records which room pairs get passages, computed by
//! [`plan_connections`] from each slot's `gone` flag, growing a spanning tree
//! over the live rooms and then adding a few extra links for loopiness.

use crate::config::GameConfig;
use crate::rnd::rnd;

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

    /// Recompute the connection plan from the rooms' `gone` flags.
    pub(crate) fn generate(&mut self, gone: &[bool]) {
        self.connections = plan_connections(gone);
    }

    /// Discard the current connection plan.
    pub fn reset(&mut self) {
        self.connections.clear();
    }
}

/// Plan which room pairs get connected.
fn plan_connections(gone: &[bool]) -> Vec<(usize, usize)> {
    let live_rooms = gone.iter().filter(|&&is_gone| !is_gone).count();
    if live_rooms <= 1 {
        return Vec::new();
    }

    let mut connections = Vec::new();
    build_spanning_tree(gone, live_rooms, &mut connections);
    add_extra_connections(gone, &mut connections);
    connections
}

/// Grow a spanning tree over the live rooms.
///
/// "Gone" rooms act as pass-through cells in the 3x3 grid: they may appear in
/// the tree, but only non-gone rooms must become reachable.
fn build_spanning_tree(
    gone: &[bool],
    live_rooms: usize,
    connections: &mut Vec<(usize, usize)>,
) {
    let mut in_graph = [false; GameConfig::MAX_ROOMS];
    let mut current = pick_random(|idx| !gone[idx]);
    in_graph[current] = true;
    let mut reached = 1;

    while reached < live_rooms {
        match next_unreached(current, &in_graph) {
            Some(next) => {
                in_graph[next] = true;
                if !gone[next] {
                    reached += 1;
                }
                connections.push((current, next));
                current = next;
            }
            None => current = pick_random(|idx| in_graph[idx]),
        }
    }
}

/// Add a few extra connecting passages for loopiness.
fn add_extra_connections(gone: &[bool], connections: &mut Vec<(usize, usize)>) {
    let mut extra = rnd(GameConfig::EXTRA_CONNECTION_ROLLS);
    while extra > 0 {
        let from = pick_random(|idx| !gone[idx]);
        if let Some(to) = next_unconnected(from, connections) {
            connections.push((from, to));
        }
        extra -= 1;
    }
}

/// Pick a uniformly random adjacent room not yet part of the connected graph.
fn next_unreached(from: usize, in_graph: &[bool; GameConfig::MAX_ROOMS]) -> Option<usize> {
    pick_unconnected(neighbors(from), |room| in_graph[room])
}

/// Pick a uniformly random adjacent room with no dug connection to `from` yet.
fn next_unconnected(from: usize, connections: &[(usize, usize)]) -> Option<usize> {
    pick_unconnected(neighbors(from), |room| is_connected(connections, from, room))
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

/// Pick a uniformly random room index satisfying `predicate`.
fn pick_random(predicate: impl Fn(usize) -> bool) -> usize {
    loop {
        let idx = rnd(GameConfig::MAX_ROOMS as i32) as usize;
        if predicate(idx) {
            return idx;
        }
    }
}

/// Indexes of the rooms orthogonally adjacent to `room`, in ascending order.
fn neighbors(room: usize) -> impl Iterator<Item = usize> {
    let row = room / GameConfig::GRID_COLS;
    let col = room % GameConfig::GRID_COLS;

    [
        (row > 0).then(|| room - GameConfig::GRID_COLS),
        (col > 0).then(|| room - 1),
        (col + 1 < GameConfig::GRID_COLS).then(|| room + 1),
        (row + 1 < GameConfig::GRID_ROWS).then(|| room + GameConfig::GRID_COLS),
    ]
    .into_iter()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_are_orthogonal_and_in_grid() {
        assert_eq!(neighbors(0).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(neighbors(2).collect::<Vec<_>>(), vec![1, 5]);
        assert_eq!(neighbors(4).collect::<Vec<_>>(), vec![1, 3, 5, 7]);
        assert_eq!(neighbors(8).collect::<Vec<_>>(), vec![5, 7]);
    }

    #[test]
    fn plan_connections_with_single_live_room_is_empty() {
        let mut gone = [false; GameConfig::MAX_ROOMS];
        for (i, is_gone) in gone.iter_mut().enumerate() {
            if i != 4 {
                *is_gone = true;
            }
        }
        assert!(plan_connections(&gone).is_empty());
    }

    #[test]
    fn plan_connections_connects_all_live_rooms() {
        let gone = [false; GameConfig::MAX_ROOMS];
        let connections = plan_connections(&gone);
        let live = GameConfig::MAX_ROOMS;

        // Spanning tree yields |V| - 1 links; extra links are added on top,
        // bounded by EXTRA_CONNECTION_ROLLS.
        assert!(connections.len() >= live - 1);
        assert!(connections.len() <= live - 1 + GameConfig::EXTRA_CONNECTION_ROLLS as usize);

        // Every link joins orthogonally adjacent slots.
        for &(a, b) in &connections {
            assert!(neighbors(a).any(|n| n == b), "{a} is not adjacent to {b}");
        }

        // The resulting graph reaches every live room.
        assert!(reaches_all_rooms(&connections, live));
    }

    #[test]
    fn room_graph_generate_and_reset() {
        let gone = [false; GameConfig::MAX_ROOMS];
        let mut graph = RoomGraph::new();
        assert!(graph.connections().is_empty());

        graph.generate(&gone);
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