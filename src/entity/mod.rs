//! Player and monster entities and their interactions.
//!
//! Entity data, movement, pursuit, combat, and monster behavior are grouped
//! here so their tightly coupled gameplay rules share one module boundary.

pub mod chase;
pub mod fight;
pub mod monsters;
pub mod player;
pub mod rndmove;
pub mod stats;
