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

impl Tile {
    /// Stable serialization discriminant for the save file.
    pub const fn to_u8(self) -> u8 {
        match self {
            Tile::Empty => 0,
            Tile::Floor => 1,
            Tile::Passage => 2,
            Tile::Door => 3,
            Tile::Wall => 4,
            Tile::HiddenDoor => 5,
            Tile::Stairs => 6,
            Tile::Trap => 7,
        }
    }

    /// Inverse of [`Tile::to_u8`].
    pub const fn from_u8(v: u8) -> Option<Tile> {
        match v {
            0 => Some(Tile::Empty),
            1 => Some(Tile::Floor),
            2 => Some(Tile::Passage),
            3 => Some(Tile::Door),
            4 => Some(Tile::Wall),
            5 => Some(Tile::HiddenDoor),
            6 => Some(Tile::Stairs),
            7 => Some(Tile::Trap),
            _ => None,
        }
    }
}
