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
