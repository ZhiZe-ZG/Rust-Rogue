//! Character statistics shared by the hero and monsters.
//!
//! Successor to the legacy C `struct stats` (formerly `CStats`). The `#[repr(C)]`
//! layout, the `s_` field prefixes, and the `std::os::raw` integer types are
//! gone now that the port no longer links C code. Save/restore maps each field
//! explicitly, so the on-disk format is unchanged.

use std::os::raw::c_char;

/// Combat and progression statistics shared by the hero and monsters.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Strength; unsigned because it is clamped to a minimum of 3.
    pub strength: u32,
    /// Experience points earned.
    pub experience: i32,
    /// Experience level.
    pub level: i32,
    /// Armor class (lower is better).
    pub armor: i32,
    /// Current hit points.
    pub hit_points: i32,
    /// Damage specification as a NUL-terminated ASCII string, e.g. `"2x4"`
    /// (a 2d4 roll) or `"1x8/1x8/3x10"` for multiple attacks. Kept as a fixed
    /// 13-byte buffer so the legacy save format and libc string parsing
    /// (`atoi`/`strchr`) keep working unchanged.
    pub damage: [c_char; 13],
    /// Maximum hit points.
    pub max_hit_points: i32,
}
