//! Dungeon-level generation and lifecycle orchestration.
//!
//! Owns the level-transition entry points ([`new_level`] and [`door_open`])
//! that build and populate a fresh dungeon level, driving the room/passage
//! generation and population passes owned by [`crate::level`].

mod generation;

pub use generation::{door_open, new_level};