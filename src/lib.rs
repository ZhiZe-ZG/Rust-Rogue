//! Rust Rogue — a Rust reimplementation of Rogue: Exploring the Dungeons of Doom.
//!
//! The crate mirrors the module layout of the original C sources: each module
//! is a direct port of the corresponding `.c` file and preserves its C ABI
//! wherever the legacy engine still depends on it.
pub mod colors;
pub mod command;
pub mod config;
pub mod daemon;
pub mod daemons;
pub mod draw;
pub mod entity;
pub mod game;
pub mod globals;
pub mod help;
pub mod init;
pub mod item;
pub mod level;
pub mod machdep;
pub mod mdport;
pub mod misc;
pub mod options;
pub mod rip;
pub mod rnd;
pub mod save;
pub mod score;
pub mod startup;
pub mod state;
pub mod structure;
pub mod tile;
pub mod ui;
pub mod vers;
pub mod wizard;
