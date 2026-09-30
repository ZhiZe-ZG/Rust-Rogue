//! Dungeon-level generation and lifecycle orchestration.
//!
//! Owns the level-transition entry points ([`new_level`] and [`door_open`])
//! that build and populate a fresh dungeon level, driving the room/passage
//! generation and population passes owned by [`crate::level`].

mod generation;
mod presence;

pub use generation::{door_open, new_level};
pub(crate) use presence::find_floor;
