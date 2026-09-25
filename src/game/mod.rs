//! Game state.
//!
//! Owns the process-wide game state that the legacy C engine previously kept
//! in globals:
//!
//! * the **live dungeon level** — the [`Level`](crate::level::Level) singleton
//!   (tile map, flags, rooms, passages, floor items, and per-cell monster
//!   occupancy) for the current depth, together with the current equipment and
//!   the stable player actor;
//! * the **monster list** — the [`MLIST`] head of the live monster linked list.
//!
//! The sub-modules are private, matching the [`crate::level`] layout; the
//! public items are re-exported here so callers keep using `crate::game::…`.

mod game;
mod monster_list;

pub use game::*;
pub use monster_list::{MonsterId, MonsterList, MLIST};
