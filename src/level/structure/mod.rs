//! Tile-grid container and room model for level generation.
//!
//! [`Structure`] is a rectangular grid of [`Tile`]s used by generation, and
//! [`Room`] is the logical room model.

mod room;
mod structure;

pub use room::Room;
pub use structure::Structure;
