/// Semantic tile kinds used by Rust level-generation structures.
///
/// These values describe logical map content, independent of the concrete
/// glyphs rendered by the C/ncurses side.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tile {
    /// Outside playable geometry / uninitialized map cell.
    Empty,
    /// Walkable room interior.
    Floor,
    /// Walkable corridor cell connecting rooms.
    Passage,
    /// Doorway at a room/corridor boundary.
    Door,
    /// Solid room boundary wall.
    Wall,
    /// Down staircase to the next dungeon level.
    Stairs,
    /// Trap tile that can trigger gameplay effects.
    Trap,
}
