//! Game state.
//!
//! Owns the process-wide game state that the legacy C engine previously kept
//! in globals:
//!
//! * the **live dungeon level** — the [`Level`](crate::level::Level) singleton
//!   (tile map, flags, rooms, passages, and floor items) for the current depth;
//! * the **player** — the stable actor and its [`Equipment`](crate::game::Player::equipment);
//! * the **monster list** — the [`MLIST`] head of the live monster linked list;
//! * the **monster map** — the [`MONSTER_MAP`] per-cell monster occupancy grid.
//!
//! The sub-modules are private, matching the [`crate::level`] layout; the
//! public items are re-exported here so callers keep using `crate::game::…`.

mod game;
mod monster_list;
mod monster_map;
mod player;

pub use game::*;
pub use monster_list::{MonsterId, MonsterList, MONSTER_LIST};
pub use monster_map::{MonsterMap, MONSTER_MAP};
pub use player::{player_remove_flag, Player, PLAYER};
