//! Game state.
//!
//! Owns the process-wide game state that the legacy C engine previously kept
//! in globals:
//!
//! * the **live dungeon** — the [`Dungeon`](crate::dungeon::Dungeon) singleton,
//!   which now also owns the **monster list** and **monster map** (see
//!   [`crate::dungeon`]);
//! * the **player** — the stable actor and its [`Equipment`](crate::game::Player::equipment).
//!
//! The sub-modules are private, matching the [`crate::level`] layout; the
//! public items are re-exported here so callers keep using `crate::game::…`.

mod game;
pub mod globals;
mod player;

pub use crate::dungeon::{Dungeon, MonsterId, MonsterList, MonsterMap, DUNGEON};
pub use game::*;
pub use player::{player_remove_flag, Player, PLAYER};