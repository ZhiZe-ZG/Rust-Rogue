//! Tile and trap vocabulary for the Rust-side level representation.
//!
//! [`Tile`] describes logical map content, and [`Trap`] is the trap-kind
//! vocabulary stored in each dungeon cell.

mod tile;
mod trap;

pub use tile::Tile;
pub use trap::{TrapType, TrapHit};
