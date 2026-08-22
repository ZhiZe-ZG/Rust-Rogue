//! In-memory dungeon level state.
//!
//! The `Level` type tracks the depth, room layouts, passages, and tile map
//! for the current dungeon level, plus the process-wide singleton holding
//! the live level.

use glam::IVec2;

use super::passages::Passage;
use super::rooms::Room;
use super::structure::Structure;
use super::tile::Tile;

pub const LEVEL_HEIGHT: usize = 24;
pub const LEVEL_WIDTH: usize = 80;
pub const MAX_LEVEL_ROOMS: usize = 9;
pub const MAX_LEVEL_PASSAGES: usize = 13;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub depth: i32,
    pub rooms: Vec<Room>,
    pub room_connections: Vec<(usize, usize)>,
    pub passages: Vec<Passage>,
    pub map: Structure,
}

impl Default for Level {
    fn default() -> Self {
        Self {
            depth: 0,
            rooms: Vec::new(),
            room_connections: Vec::new(),
            passages: Vec::new(),
            map: Structure::new(LEVEL_HEIGHT, LEVEL_WIDTH, Tile::Empty),
        }
    }
}

impl Level {
    pub fn new() -> Self {
        let mut level = Self::default();
        level.rooms = (0..MAX_LEVEL_ROOMS)
            .map(|_| Room::new(IVec2::ZERO, IVec2::ZERO, None, None))
            .collect();
        level.map = Structure::new(LEVEL_HEIGHT, LEVEL_WIDTH, Tile::Empty);
        level
    }

    pub fn create() -> Self {
        Self::new()
    }

    pub fn reset(&mut self) {
        self.depth = 0;
        self.rooms.clear();
        self.room_connections.clear();
        self.passages.clear();
        self.map = Structure::new(LEVEL_HEIGHT, LEVEL_WIDTH, Tile::Empty);
    }

    pub fn add_room(&mut self, room: Room) {
        self.rooms.push(room);
    }

    pub fn add_connection(&mut self, from: usize, to: usize) {
        self.room_connections.push((from, to));
    }

    pub fn add_passage(&mut self, passage: Passage) {
        self.passages.push(passage);
    }
}

pub fn create_level() -> Level {
    Level::new()
}

static mut CURRENT_LEVEL: Option<Level> = None;

pub unsafe fn current_level_mut() -> &'static mut Level {
    if CURRENT_LEVEL.is_none() {
        CURRENT_LEVEL = Some(Level::new());
    }
    CURRENT_LEVEL.as_mut().unwrap()
}

pub unsafe fn set_current_level(level: Level) -> &'static mut Level {
    CURRENT_LEVEL = Some(level);
    CURRENT_LEVEL.as_mut().unwrap()
}
