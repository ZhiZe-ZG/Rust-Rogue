/// Semantic tile kinds used by Rust level-generation structures.
///
/// These values describe logical map content, independent of the concrete
/// glyphs rendered by the C/ncurses side. Orientation-sensitive tiles such as
/// [`Tile::Wall`] have their on-screen character (`-` vs `|`) decided at draw
/// time from the neighbouring cells.
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
    /// Room boundary wall segment; renders as `-` or `|` depending on its
    /// neighbours.
    Wall,
    /// Door disguised as a wall segment until it is revealed (renders like a
    /// [`Tile::Wall`]).
    HiddenDoor,
    /// Down staircase to the next dungeon level.
    Stairs,
    /// Hidden trap that can trigger gameplay effects; renders like floor
    /// until it is revealed.
    Trap,
}
