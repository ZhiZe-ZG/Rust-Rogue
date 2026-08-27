//! Corridor/passage digging helpers, Rust-side per-cell flags, and the C
//! global mirroring.
//!
//! Level generation writes per-cell flags into [`Level::flags`] and the door
//! exits of each numbered passage component into [`Level::passage_links`]
//! (see [`Level::mark_passages`] and [`Level::number_passages`]) instead of
//! poking the C `places`/`rooms`/`passages` globals directly. Once the whole
//! level is generated, [`crate::level::ffi::copy_flags_to_c`],
//! [`crate::level::ffi::sync_rooms_to_c`], and
//! [`crate::level::ffi::sync_passages_to_c`] translate those Rust structures
//! into the C arrays the engine consumes.

use std::os::raw::c_int;

use glam::IVec2;

/// Size of the C `passages` room array (also the cap on numbered components).
pub(crate) const MAX_PASSAGES: usize = 13;
/// Max exits writeable into one C `r_exit` array.
pub(crate) const MAX_EXITS: usize = 12;
/// Width of the playable C `places` screen.
pub(crate) const SCREEN_COLS: c_int = 80;
/// Height of the playable C `places` screen.
pub(crate) const SCREEN_LINES: c_int = 24;

/// A corridor connecting two rooms.
///
/// Mirrors the [`Room`](super::rooms::Room) abstraction: a bounding box
/// (`position`/`size`) plus the relative coordinates of every passage tile
/// and entry point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passage {
    pub position: IVec2,
    pub size: IVec2,
    /// Coordinates of every passage tile, relative to `position`.
    pub tiles: Vec<IVec2>,
    /// Coordinates of the doors joining adjacent rooms, relative to `position`.
    pub entry_points: Vec<IVec2>,
}

/// Door exits of one numbered passage component.
///
/// Produced by [`Level::number_passages`] and mirrored to one slot of the C
/// `passages` array (a `CRoom` used as an exit table) by
/// [`crate::level::ffi::sync_passages_to_c`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PassageLinks {
    /// Absolute map coordinates of the component's doorways.
    pub exits: Vec<IVec2>,
}

/// Geometric plan of the L-shaped corridor between two rooms.
///
/// Produced by [`Level::plan_corridor`] and consumed by [`Level::dig_corridor`]
/// to register the corridor's doors and lay its tiles. All coordinates are
/// absolute map coordinates in the level's Rust tile grid.
pub(crate) struct CorridorPlan {
    /// Index of the room the corridor leaves (the lower room index).
    pub(crate) base_room: usize,
    /// Index of the room the corridor enters (the room paired with `base_room`).
    pub(crate) partner_room: usize,
    /// Per-cell step of the straight run: `(0, 1)` for vertical corridors,
    /// `(1, 0)` for horizontal ones.
    pub(crate) step: IVec2,
    /// Entry point on `base_room`'s boundary.
    pub(crate) start: IVec2,
    /// Exit point on `partner_room`'s boundary.
    pub(crate) end: IVec2,
    /// Number of cells laid along `step` before the turn.
    pub(crate) distance: i32,
    /// Per-cell step of the perpendicular turn.
    pub(crate) turn_step: IVec2,
    /// Number of cells laid along `turn_step`.
    pub(crate) turn_distance: i32,
    /// Position along the straight run at which the turn begins.
    pub(crate) turn_spot: i32,
}