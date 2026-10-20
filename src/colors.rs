//! Colour vocabulary.

use crate::rnd::Random;

/// The colour names used to describe potions etc.
///
/// The order is fixed: the save file stores each potion's colour as an index
/// into this table, so entries must not be reordered or removed.
pub const POTION_COLORS: [&str; 27] = [
    "amber",
    "aquamarine",
    "black",
    "blue",
    "brown",
    "clear",
    "crimson",
    "cyan",
    "ecru",
    "gold",
    "green",
    "grey",
    "magenta",
    "orange",
    "pink",
    "plaid",
    "purple",
    "red",
    "silver",
    "tan",
    "tangerine",
    "topaz",
    "turquoise",
    "vermilion",
    "violet",
    "white",
    "yellow",
];

/// Number of potion colours.
pub const POTION_COLOR_COUNT: usize = POTION_COLORS.len();

/// A random potion colour, drawn from the supplied generator.
pub fn random_color(random: &mut Random) -> &'static str {
    POTION_COLORS[random.rnd(POTION_COLOR_COUNT as i32) as usize]
}
