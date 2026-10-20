//! Tile-grid container, room model, and corridor model for level generation.
//!
//! [`Structure`] is a rectangular grid of [`Tile`]s used by generation,
//! [`Room`] is the logical room model, and [`Passage`] is the corridor model.

mod passage;
mod room;
mod structure;

pub use passage::Passage;
pub use room::Room;
pub use structure::Structure;
