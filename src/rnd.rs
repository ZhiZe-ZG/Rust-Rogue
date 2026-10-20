//! Random number generation.
//!
//! Provides the legacy random-number contracts through the shared, seedable
//! [`RNG`] owner so generated levels and save files stay deterministic.
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::{LazyLock, Mutex};

/// A seedable random-number generator.
///
/// Owns only the generator state; synchronization is provided by [`RNG`].
pub struct Random {
    state: StdRng,
}

/// The single process-wide random-number generator.
///
/// Release the mutex guard before calling code that may use this generator
/// again. In compound expressions, use a scoped local guard and return only
/// the generated value so nested draws cannot attempt to lock it twice.
pub static RNG: LazyLock<Mutex<Random>> = LazyLock::new(|| Mutex::new(Random::new()));

impl Random {
    fn new() -> Self {
        Self {
            state: StdRng::from_seed([0x5eu8; 32]),
        }
    }

    /// Return a pseudo-random integer in `[0, range)` for positive ranges.
    ///
    /// Matches the legacy Rogue contract: zero yields zero without consuming a
    /// draw, and negative ranges negate a draw from `[0, range.abs())`.
    pub fn rnd(&mut self, range: i32) -> i32 {
        if range == 0 {
            return 0;
        }

        let magnitude = range.abs();
        let value = self.state.gen_range(0..magnitude);

        if range < 0 {
            -value
        } else {
            value
        }
    }

    /// Reset this generator so startup code can preserve deterministic seeds.
    pub fn set_seed(&mut self, seed: i32) {
        self.state = StdRng::seed_from_u64(seed as u64);
    }

    /// Sum `number` successive results of `self.rnd(sides) + 1`.
    ///
    /// Consumes this generator's state without acquiring any locks itself.
    /// Nonpositive dice counts return zero without consuming any draws.
    pub fn roll(&mut self, mut number: i32, sides: i32) -> i32 {
        let mut total = 0;
        while number > 0 {
            total += self.rnd(sides) + 1;
            number -= 1;
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sequence_matches_legacy_generator() {
        let mut random = Random::new();
        let mut expected = StdRng::from_seed([0x5e; 32]);
        for _ in 0..100 {
            assert_eq!(random.rnd(1000), expected.gen_range(0..1000));
        }
    }

    #[test]
    fn reseeding_preserves_signed_seed_conversion_and_sequence() {
        let mut random = Random::new();
        for seed in [0, 42, -1, i32::MIN, i32::MAX] {
            random.set_seed(seed);
            let mut expected = StdRng::seed_from_u64(seed as u64);
            let sequence: Vec<_> = (0..100).map(|_| random.rnd(1000)).collect();
            for &value in &sequence {
                assert_eq!(value, expected.gen_range(0..1000));
            }
            random.set_seed(seed);
            assert_eq!(
                sequence,
                (0..100).map(|_| random.rnd(1000)).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn ranges_preserve_sign_and_zero_does_not_advance_state() {
        let mut random = Random::new();
        let mut reference = Random::new();
        assert_eq!(random.rnd(0), 0);
        for range in [1, -1, 2, -2, i32::MAX, -i32::MAX] {
            for _ in 0..100 {
                assert_eq!(random.rnd(0), 0);
                let value = random.rnd(range);
                let expected = reference.rnd(range.abs());
                assert_eq!(value, if range < 0 { -expected } else { expected });
                if range > 0 {
                    assert!((0..range).contains(&value));
                } else {
                    assert!(value <= 0 && value > range);
                }
            }
        }
    }

    #[test]
    fn random_color_uses_the_supplied_generator() {
        let mut random = Random::new();
        let mut reference = Random::new();
        for _ in 0..100 {
            let expected = crate::colors::POTION_COLORS
                [reference.rnd(crate::colors::POTION_COLOR_COUNT as i32) as usize];
            assert_eq!(crate::colors::random_color(&mut random), expected);
        }
        assert_eq!(random.rnd(1000), reference.rnd(1000));
    }

    #[test]
    fn trap_messages_use_only_the_supplied_generator() {
        use crate::tile::{TrapHit, TrapType};

        let mut random = Random::new();
        let mut reference = Random::new();
        for trap in [
            TrapType::Door,
            TrapType::Arrow,
            TrapType::Sleep,
            TrapType::Bear,
            TrapType::Teleport,
            TrapType::Dart,
            TrapType::Rust,
        ] {
            for hit in [TrapHit::Miss, TrapHit::Hit, TrapHit::Kill] {
                assert_eq!(
                    trap.msg(hit, &mut random).is_none(),
                    trap == TrapType::Teleport
                );
                assert_eq!(random.rnd(1000), reference.rnd(1000));
            }
        }
        for _ in 0..100 {
            let choice = reference.rnd(11);
            let color = if matches!(choice, 1 | 4 | 6 | 10) {
                Some(crate::colors::random_color(&mut reference))
            } else {
                None
            };
            let message = TrapType::Mystery.msg(TrapHit::Miss, &mut random).unwrap();
            if let Some(color) = color {
                assert!(message.contains(color));
            }
            assert_eq!(random.rnd(1000), reference.rnd(1000));
        }
    }

    #[test]
    fn rolls_match_successive_draws_and_preserve_state() {
        let mut random = Random::new();
        let mut reference = Random::new();
        for (number, sides) in [(0, 6), (-3, 6), (4, 0), (1, 1), (10, 6), (3, -6)] {
            let expected: i32 = (0..number).map(|_| reference.rnd(sides) + 1).sum();
            assert_eq!(random.roll(number, sides), expected);
            assert_eq!(random.rnd(1000), reference.rnd(1000));
        }
    }
}
