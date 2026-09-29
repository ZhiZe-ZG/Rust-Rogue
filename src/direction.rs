use glam::IVec2;

/// A normalized movement direction used by command and run handling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    #[default]
    None,
    West,
    South,
    North,
    East,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

impl Direction {
    /// Convert a Rogue movement key to its normalized direction.
    pub const fn from_byte(key: u8) -> Option<Self> {
        match key.to_ascii_lowercase() {
            b'h' => Some(Self::West),
            b'j' => Some(Self::South),
            b'k' => Some(Self::North),
            b'l' => Some(Self::East),
            b'y' => Some(Self::NorthWest),
            b'u' => Some(Self::NorthEast),
            b'b' => Some(Self::SouthWest),
            b'n' => Some(Self::SouthEast),
            _ => None,
        }
    }

    /// The canonical lowercase Rogue movement key for this direction.
    pub const fn to_byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::West => b'h',
            Self::South => b'j',
            Self::North => b'k',
            Self::East => b'l',
            Self::NorthWest => b'y',
            Self::NorthEast => b'u',
            Self::SouthWest => b'b',
            Self::SouthEast => b'n',
        }
    }

    /// Grid delta `(x, y)` for this movement direction.
    pub const fn delta(self) -> IVec2 {
        match self {
            Self::None => IVec2::ZERO,
            Self::West => IVec2::new(-1, 0),
            Self::South => IVec2::new(0, 1),
            Self::North => IVec2::new(0, -1),
            Self::East => IVec2::new(1, 0),
            Self::NorthWest => IVec2::new(-1, -1),
            Self::NorthEast => IVec2::new(1, -1),
            Self::SouthWest => IVec2::new(-1, 1),
            Self::SouthEast => IVec2::new(1, 1),
        }
    }
}
