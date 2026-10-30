//! Inventory items and their gameplay behavior.
//!
//! Item categories, inventory management, allocation, naming, and equipment
//! behavior live under this module.
//!
//! Dependencies are separate function parameters, not a shared item context.
//! Raw state pointers must stay valid for the call, with no overlapping
//! exclusive borrows across reentrant UI, combat, or daemon helpers. Immutable
//! metadata slices must cover every indexed kind and must not be mutated for
//! the duration of a call; mutable metadata tables are passed as raw slices.
//! External gameplay/UI helpers still form singleton-backed boundaries outside
//! this module: supplying a separate dungeon does not retarget those helpers.

pub mod arena;
pub mod armor;
pub mod item_type;
pub mod pack;
pub mod potions;
pub mod rings;
pub mod scrolls;
pub mod sticks;
pub mod things;
pub mod weapons;
