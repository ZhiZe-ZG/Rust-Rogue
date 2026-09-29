//! Corridor/passage model for level generation.
//!
//! A [`Passage`] mirrors the [`Room`](crate::structure::Room) abstraction: a
//! bounding box (`position`/`size`) plus the relative coordinates of every
//! passage tile and entry point. It is produced by level generation and stored
//! in the owning [`Level`](crate::level::Level).

use glam::IVec2;

/// A corridor connecting two rooms.
///
/// Mirrors the [`Room`](crate::structure::Room) abstraction: a bounding box
/// (`position`/`size`) plus the relative coordinates of every passage tile
/// and entry point.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Passage {
    pub position: IVec2,
    pub size: IVec2,
    /// Coordinates of every passage tile, relative to `position`.
    pub tiles: Vec<IVec2>,
    /// Coordinates of the doors joining adjacent rooms, relative to `position`.
    pub entry_points: Vec<IVec2>,
}