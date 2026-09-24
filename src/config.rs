//! Compile-time parameters for dungeon generation and storage.

/// The fixed configuration used by the game.
///
/// Keeping these values as associated constants gives array sizes and level
/// algorithms one source of truth without carrying a runtime configuration
/// through the legacy game loop.
pub struct GameConfig;

impl GameConfig {
    /// Rows allocated for each level, including off-screen storage.
    pub const LEVEL_HEIGHT: usize = 32;
    /// Columns allocated for each level.
    pub const LEVEL_WIDTH: usize = 80;
    /// Rows visible in the terminal, including the status and message rows.
    pub const SCREEN_LINES: i32 = 24;
    /// Columns visible in the terminal.
    pub const SCREEN_COLS: i32 = 80;
    /// Room slots in the fixed three-by-three room grid.
    pub const MAX_ROOMS: usize = 9;
    /// Rows in the fixed three-by-three room grid.
    pub const GRID_ROWS: usize = 3;
    /// Columns in the fixed three-by-three room grid.
    pub const GRID_COLS: usize = 3;
    /// Upper bound for the number of extra passage links beyond the spanning
    /// tree in a connection plan.
    pub const EXTRA_CONNECTION_ROLLS: i32 = 5;
    /// Maximum number of numbered passage components.
    pub const MAX_PASSAGES: usize = 13;
    /// Maximum exits retained for one room or passage component.
    pub const MAX_EXITS: usize = 12;
    /// Attempts made to place a regular room before dropping that room.
    pub const MAX_ROOM_PLACEMENT_ATTEMPTS: usize = 100;
    /// Maximum number of treasure items generated in a treasure room.
    pub const MAX_TREASURES: i32 = 10;
    /// Minimum number of treasure items generated in a treasure room.
    pub const MIN_TREASURES: i32 = 2;
    /// Attempts made to find a valid cell for a generated entity.
    pub const MAX_PLACEMENT_ATTEMPTS: i32 = 10;
    /// Number of ordinary object-placement rolls per level.
    pub const MAX_OBJECTS: i32 = 9;
    /// One-in-N chance that a level contains a treasure room.
    pub const TREASURE_ROOM_CHANCE: i32 = 20;
    /// First depth at which the Amulet of Yendor may appear.
    pub const AMULET_LEVEL: i32 = 26;
    /// Maximum traps generated on one level.
    pub const MAX_TRAPS: i32 = 10;
    /// Number of distinct trap kinds available to generation.
    pub const TRAP_KIND_COUNT: i32 = 8;
}
