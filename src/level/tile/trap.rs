//! Trap kinds and their messages.
//!
//! [`Trap`] is the trap-kind vocabulary stored in each dungeon cell.
//! [`Trap::msg`] determines the message the hero sees when a trap fires; the
//! trap effect itself is applied by the movement code in
//! [`crate::entity::player`].

use crate::rnd::rnd;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trap {
    Door = 0,
    Arrow = 1,
    Sleep = 2,
    Bear = 3,
    Teleport = 4,
    Dart = 5,
    Rust = 6,
    Mystery = 7,
}

/// Outcome of a damaging trap attack, used to select the right [`Trap::msg`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapHit {
    /// The attack missed.
    Miss,
    /// The attack connected but was not fatal.
    Hit,
    /// The attack killed the hero.
    Kill,
}

impl Trap {
    #[inline]
    pub const fn from_raw(value: u8) -> Self {
        match value {
            0 => Self::Door,
            1 => Self::Arrow,
            2 => Self::Sleep,
            3 => Self::Bear,
            4 => Self::Teleport,
            5 => Self::Dart,
            6 => Self::Rust,
            7 => Self::Mystery,
            _ => panic!("invalid trap type"),
        }
    }

    /// Determine the message the hero sees when this trap fires, or `None` for
    /// traps that print no message (teleport). `hit` selects among the miss,
    /// hit, and fatal variants used by the damaging traps.
    pub fn msg(&self, hit: TrapHit) -> Option<String> {
        match self {
            Trap::Door => Some("you fell into a trap!".to_string()),
            Trap::Bear => Some("you are caught in a bear trap".to_string()),
            Trap::Sleep => {
                Some("a strange white mist envelops you and you fall asleep".to_string())
            }
            Trap::Rust => Some("a gush of water hits you on the head".to_string()),
            Trap::Mystery => match rnd(11) {
                0 => Some("you are suddenly in a parallel dimension".to_string()),
                1 => Some(format!(
                    "the light in here suddenly seems {}",
                    crate::colors::random_color()
                )),
                2 => Some("you feel a sting in the side of your neck".to_string()),
                3 => Some("multi-colored lines swirl around you, then fade".to_string()),
                4 => Some(format!(
                    "a {} light flashes in your eyes",
                    crate::colors::random_color()
                )),
                5 => Some("a spike shoots past your ear!".to_string()),
                6 => Some(format!(
                    "{} sparks dance across your armor",
                    crate::colors::random_color()
                )),
                7 => Some("you suddenly feel very thirsty".to_string()),
                8 => Some("you feel time speed up suddenly".to_string()),
                9 => Some("time now seems to be going slower".to_string()),
                10 => Some(format!("you pack turns {}!", crate::colors::random_color())),
                _ => None,
            },
            Trap::Arrow => Some(match hit {
                TrapHit::Miss => "an arrow shoots past you".to_string(),
                TrapHit::Hit => "oh no! An arrow shot you".to_string(),
                TrapHit::Kill => "an arrow killed you".to_string(),
            }),
            Trap::Dart => Some(match hit {
                TrapHit::Miss => "a small dart whizzes by your ear and vanishes".to_string(),
                TrapHit::Hit => "a small dart just hit you in the shoulder".to_string(),
                TrapHit::Kill => "a poisoned dart killed you".to_string(),
            }),
            Trap::Teleport => None,
        }
    }
}
