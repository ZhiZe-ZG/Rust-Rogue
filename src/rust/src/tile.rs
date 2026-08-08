#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tile {
    Empty,
    Floor,
    Passage,
    Door,
    Wall,
    Stairs,
    Trap,
}
