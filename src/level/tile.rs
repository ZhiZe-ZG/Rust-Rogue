//! Tile vocabulary for the Rust-side level representation.
//!
//! A [`Tile`] describes logical map content. Its on-screen glyph is chosen at
//! draw time, and [`Tile::is_walkable`] reports whether a cell can be entered
//! or crossed.

use super::trap::Trap;

/// Semantic tile kinds for the level map.
///
/// Orientation-sensitive tiles such as [`Tile::Wall`] have their on-screen
/// character (`-` vs `|`) decided at draw time from the neighbouring cells.
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
    /// Hidden trap that can trigger gameplay effects; renders as floor until
    /// seen. The payload identifies the [`Trap`] kind.
    Trap(Trap),
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
            Tile::Trap(_) => 7,
        }
    }

    /// Inverse of [`Tile::to_u8`], returning [`None`] for unknown
    /// discriminants.
    pub const fn from_u8(v: u8) -> Option<Tile> {
        match v {
            0 => Some(Tile::Empty),
            1 => Some(Tile::Floor),
            2 => Some(Tile::Passage),
            3 => Some(Tile::Door),
            4 => Some(Tile::Wall),
            5 => Some(Tile::HiddenDoor),
            6 => Some(Tile::Stairs),
            7 => Some(Tile::Trap(Trap::Door)),
            _ => None,
        }
    }

    /// Whether this [`Tile`] can be entered or crossed.
    ///
    /// Blank cells, walls, and hidden doors block movement; floors, passages,
    /// doors, stairs, and traps are traversable. Monster occupancy is not
    /// encoded in a tile — it is checked separately against the per-cell
    /// monster map.
    ///
    /// This is a `const fn`, so it can also be evaluated at compile time.
    pub const fn is_walkable(self) -> bool {
        !matches!(self, Tile::Empty | Tile::Wall | Tile::HiddenDoor)
    }
}

#[cfg(test)]
mod tests {
    use super::{Tile, Trap};

    #[test]
    fn tile_walkability_matches_map_semantics() {
        for blocked in [Tile::Empty, Tile::Wall, Tile::HiddenDoor] {
            assert!(!blocked.is_walkable());
        }

        for walkable in [
            Tile::Floor,
            Tile::Passage,
            Tile::Door,
            Tile::Stairs,
            Tile::Trap(Trap::Door),
        ] {
            assert!(walkable.is_walkable());
        }
    }
}
