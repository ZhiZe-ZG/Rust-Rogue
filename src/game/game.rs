//! Game-state sub-module.
//!
//! Owns the process-wide game state that the legacy C engine previously kept
//! in globals:
//!
//! * the **current level** — the [`Level`] singleton (tile map, flags, rooms,
//!   passages, and floor items) for the live dungeon depth;
//! * the **current equipment** — non-owning pointers to the armor, rings, and
//!   weapon selected from the player's pack.
//!
//! Cell display glyphs and flat flags are no longer cached in a legacy per-cell
//! grid (the `p_ch`/`p_flags` members were removed); every access goes through
//! `crate::draw`, which computes them from the [`Level`] tile map and flag
//! grids on the fly.

use std::os::raw::c_int;
use std::sync::{OnceLock, RwLock};

use crate::config::GameConfig;
use crate::entity::player::{Thing, ThingMonster, Stats};
use crate::level::Level;
use crate::tile::Tile;
use glam::IVec2;

use crate::game::MONSTER_MAP;

/// A non-owning, interior-mutable cell for a raw [`CThing`] pointer.
///
/// The game is single-threaded, but these pointers are shared through
/// `static`s (which must be `Sync`). The raw pointer itself is neither `Send`
/// nor `Sync`, so this wrapper opts in explicitly and exposes lock-guarded
/// access.
struct PtrCell(RwLock<*mut Thing>);

// SAFETY: access to the pointer is always guarded by the inner `RwLock`, and
// the pointer is only dereferenced by the owning (single-threaded) game logic.
unsafe impl Send for PtrCell {}
unsafe impl Sync for PtrCell {}

impl PtrCell {
    const fn new() -> Self {
        Self(RwLock::new(std::ptr::null_mut()))
    }

    #[inline]
    fn get(&self) -> *mut Thing {
        *self.0.read().unwrap_or_else(|poison| poison.into_inner())
    }

    #[inline]
    fn set(&self, ptr: *mut Thing) {
        *self.0.write().unwrap_or_else(|poison| poison.into_inner()) = ptr;
    }
}

/// Non-owning pointers to the objects currently equipped by the player.
pub struct Equipment {
    armor: PtrCell,
    rings: [PtrCell; 2],
    weapon: PtrCell,
}

impl Equipment {
    const EMPTY: Self = Self {
        armor: PtrCell::new(),
        rings: [PtrCell::new(), PtrCell::new()],
        weapon: PtrCell::new(),
    };

    #[inline]
    pub fn armor(&self) -> *mut Thing {
        self.armor.get()
    }

    #[inline]
    pub fn set_armor(&self, armor: *mut Thing) {
        self.armor.set(armor);
    }

    #[inline]
    pub fn left_ring(&self) -> *mut Thing {
        self.rings[0].get()
    }

    #[inline]
    pub fn right_ring(&self) -> *mut Thing {
        self.rings[1].get()
    }

    #[inline]
    pub fn set_left_ring(&self, ring: *mut Thing) {
        self.rings[0].set(ring);
    }

    #[inline]
    pub fn set_right_ring(&self, ring: *mut Thing) {
        self.rings[1].set(ring);
    }

    #[inline]
    pub fn weapon(&self) -> *mut Thing {
        self.weapon.get()
    }

    #[inline]
    pub fn set_weapon(&self, weapon: *mut Thing) {
        self.weapon.set(weapon);
    }
}

/// Current player equipment. Items remain owned by the player's pack.
pub static EQUIPMENT: Equipment = Equipment::EMPTY;

/// The zero-valued actor [`CThing`] used to seed the global player.
fn default_player() -> Thing {
    Thing::Monster {
        link: crate::entity::player::ThingLink::empty(),
        data: ThingMonster {
            t_pos: IVec2 { x: 0, y: 0 },
            t_turn: false,
            t_type: 0,
            t_disguise: 0,
            t_oldch: 0,
            t_dest: None,
            t_flags: crate::entity::player::MonsterFlags::NONE,
            t_stats: Stats {
                strength: 0,
                experience: 0,
                level: 0,
                armor: 0,
                hit_points: 0,
                damage: [0; 13],
                max_hit_points: 0,
            },
            t_room: None,
            t_pack: None,
            t_reserved: 0,
        },
    }
}

/// A safe, process-wide holder for the player's actor [`CThing`].
///
/// The game is single-threaded, but the player must be reachable from many
/// modules as one stable, process-lifetime object whose address never changes
/// (chase targets and save code keep raw pointers to `t_pos`/`t_stats`). The
/// `CThing` is boxed and initialized exactly once, so its heap address is
/// stable and [`Player::ptr`] always hands back the same `*mut CThing`. It
/// replaces the legacy `#[no_mangle] static mut player` global.
pub struct Player {
    thing: OnceLock<Box<Thing>>,
}

// SAFETY: the game runs on a single thread, and the boxed `CThing` is only ever
// reached through the stable raw pointer returned by `ptr`.
unsafe impl Sync for Player {}

impl Player {
    const EMPTY: Self = Self {
        thing: OnceLock::new(),
    };

    #[inline]
    fn get(&self) -> &Thing {
        self.thing.get_or_init(|| Box::new(default_player()))
    }

    /// A stable, process-lifetime pointer to the player `CThing`.
    #[inline]
    pub fn ptr(&self) -> *mut Thing {
        self.get() as *const Thing as *mut Thing
    }
}

/// Process-wide owner of the player's actor [`CThing`].
pub static PLAYER: Player = Player::EMPTY;

/// A stable, process-lifetime pointer to the global player [`CThing`].
///
/// This is the safe replacement for the legacy `&raw mut player` accesses: the
/// `CThing` is boxed once inside [`PLAYER`], so the returned address never
/// changes.
#[inline]
pub fn player_ptr() -> *mut Thing {
    PLAYER.ptr()
}

/// Remove `flag` from the global player's actor flags.
#[inline]
pub fn player_remove_flag(flag: crate::entity::player::MonsterFlags) {
    unsafe { (*crate::entity::player::thing_t(player_ptr())).t_flags.remove(flag) }
}

/// Lazily initialized owner of the live dungeon level.
pub struct CurrentLevel {
    level: RwLock<Option<Level>>,
}

impl CurrentLevel {
    const EMPTY: Self = Self {
        level: RwLock::new(None),
    };

    /// Ensure the live level exists, initializing it on first access.
    #[inline]
    fn ensure_initialized(&self) {
        let mut level = self
            .level
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        if level.is_none() {
            *level = Some(Level::new());
        }
    }

    #[inline]
    pub fn with<R>(&self, operation: impl FnOnce(&Level) -> R) -> R {
        self.ensure_initialized();
        let level = self
            .level
            .read()
            .unwrap_or_else(|poison| poison.into_inner());
        operation(level.as_ref().unwrap())
    }

    #[inline]
    pub fn with_mut<R>(&self, operation: impl FnOnce(&mut Level) -> R) -> R {
        self.ensure_initialized();
        let mut level = self
            .level
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        operation(level.as_mut().unwrap())
    }
}

/// Read the monster at `(y, x)` as a raw handle, or null.
///
/// The per-cell map stores a pointer-free [`MonsterId`]; this resolves it to the
/// monster's stable raw address for the legacy engine boundary.
#[inline]
pub unsafe fn monster_at(y: c_int, x: c_int) -> *mut Thing {
    match MONSTER_MAP.at(y as usize, x as usize) {
        Some(id) => crate::game::MLIST.handle(id).unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    }
}

/// Place `tp` at `(y, x)` in the per-cell monster occupancy map.
#[inline]
pub unsafe fn set_monster(y: c_int, x: c_int, tp: *mut Thing) {
    let id = crate::game::MLIST.find(tp);
    MONSTER_MAP.set(y as usize, x as usize, id);
}

/// Read the monster map at `(y, x)` (equivalent to [`monster_at`]).
#[inline]
pub unsafe fn moat_at(y: c_int, x: c_int) -> *mut Thing {
    monster_at(y, x)
}

/// Place a monster in the per-cell monster occupancy map.
#[inline]
pub unsafe fn set_moat_at(y: c_int, x: c_int, tp: *mut Thing) {
    set_monster(y, x, tp);
}

/// Clear every cell's monster pointer for a fresh level.
pub unsafe fn clear_level() {
    MONSTER_MAP.clear();
}

/// Whether the cell at `(y, x)` can be entered: no monster stands there and the
/// terrain tile is walkable.
pub unsafe fn cell_is_walkable(y: c_int, x: c_int) -> bool {
    if !monster_at(y, x).is_null() {
        return false;
    }
    with_current_level(|level| level.tile_at(y as usize, x as usize).is_walkable())
}

/// The tile at `(y, x)`, defaulting to [`Tile::Empty`] outside the map.
pub unsafe fn tile_at(y: c_int, x: c_int) -> Tile {
    with_current_level(|level| level.tile_at(y as usize, x as usize))
}

/// Whether `(y, x)` is a door: an ordinary door, or a hidden door that has been
/// revealed.
pub unsafe fn is_door_at(y: c_int, x: c_int) -> bool {
    with_current_level(|level| level.is_door_at(y as usize, x as usize))
}

/// Process-wide owner for the live dungeon level.
///
/// The canonical holder of the current level. The level is initialized lazily
/// on its first access and is available only through scoped closure access.
pub static CURRENT_LEVEL: CurrentLevel = CurrentLevel::EMPTY;

/// Run `operation` with immutable access to the live level.
#[inline]
pub fn with_current_level<R>(operation: impl FnOnce(&Level) -> R) -> R {
    CURRENT_LEVEL.with(operation)
}

/// Run `operation` with mutable access to the live level.
#[inline]
pub fn with_current_level_mut<R>(operation: impl FnOnce(&mut Level) -> R) -> R {
    CURRENT_LEVEL.with_mut(operation)
}

/// Read the current dungeon depth (`Level::depth`).
#[inline]
pub fn current_depth() -> i32 {
    CURRENT_LEVEL.with(|level| level.depth)
}

/// Set the current dungeon depth (`Level::depth`).
#[inline]
pub fn set_current_depth(depth: i32) {
    CURRENT_LEVEL.with_mut(|level| level.depth = depth);
}

/// Read the current down-staircase position (`Level::stairs`).
#[inline]
pub fn stairs() -> IVec2 {
    CURRENT_LEVEL.with(|level| level.stairs)
}

/// Set the current down-staircase position (`Level::stairs`).
#[inline]
pub fn set_stairs(pos: IVec2) {
    CURRENT_LEVEL.with_mut(|level| level.stairs = pos);
}

/// Whether the room reference `reference` points to a dark room.
#[inline]
pub fn room_dark(room: Option<usize>) -> bool {
    CURRENT_LEVEL.with(|level| level.room_dark(room))
}

/// Whether the room reference `reference` points to a removed room.
#[inline]
pub fn room_gone(room: Option<usize>) -> bool {
    CURRENT_LEVEL.with(|level| level.room_gone(room))
}

/// Whether the room reference `reference` points to a maze room.
#[inline]
pub fn room_maze(room: Option<usize>) -> bool {
    CURRENT_LEVEL.with(|level| level.room_maze(room))
}

/// The value of the gold stash of the room `reference` points to.
#[inline]
pub fn room_goldval(room: Option<usize>) -> i32 {
    CURRENT_LEVEL.with(|level| level.room_goldval(room))
}

/// Set the value of the gold stash of the room `reference` points to.
#[inline]
pub fn set_room_goldval(room: Option<usize>, value: i32) {
    CURRENT_LEVEL.with_mut(|level| level.set_room_goldval(room, value));
}

/// Stable per-room gold positions, mirrored from `Level` so chase targets can
/// hold raw pointers without borrowing the locked level. Kept in sync by level
/// population (`presence::place_room_contents`) and save restore.
pub static mut ROOM_GOLD: [IVec2; GameConfig::MAX_ROOMS] = [IVec2::ZERO; GameConfig::MAX_ROOMS];

/// A stable raw pointer to the gold-stash position of `reference` (or null).
///
/// The pointed-to slot lives in the process-wide [`ROOM_GOLD`] array, so it
/// does not borrow the level lock and is safe for chase-target storage.
#[inline]
pub unsafe fn room_gold_ptr(room: Option<usize>) -> *mut IVec2 {
    match room {
        Some(i) if i < GameConfig::MAX_ROOMS => (&raw mut ROOM_GOLD[i]) as *mut IVec2,
        _ => std::ptr::null_mut(),
    }
}

/// The `(position, size)` of the room `reference` points to.
#[inline]
pub fn room_bounds(room: Option<usize>) -> Option<(IVec2, IVec2)> {
    CURRENT_LEVEL.with(|level| level.room_bounds(room))
}

/// Absolute door-exit coordinates for the room/passage `reference` points to.
#[inline]
pub fn room_exits(room: Option<usize>) -> Vec<IVec2> {
    CURRENT_LEVEL.with(|level| level.room_exits(room))
}

/// Absolute door-exit coordinates for a passage index.
#[inline]
pub fn passage_exits(passage: Option<usize>) -> Vec<IVec2> {
    CURRENT_LEVEL.with(|level| level.passage_exits(passage))
}

/// Convenience alias for the crate-wide level size constants.
pub const GAME_HEIGHT: usize = GameConfig::LEVEL_HEIGHT;
pub const GAME_WIDTH: usize = GameConfig::LEVEL_WIDTH;

#[cfg(test)]
mod tests {
    use super::{CurrentLevel, Equipment, Level};
    use crate::entity::player::Thing;
    use std::mem::MaybeUninit;

    #[test]
    fn ring_accessors_keep_hands_independent() {
        let equipment = Equipment::EMPTY;
        let mut left = MaybeUninit::<Thing>::uninit();
        let mut right = MaybeUninit::<Thing>::uninit();

        equipment.set_left_ring(left.as_mut_ptr());
        equipment.set_right_ring(right.as_mut_ptr());

        assert_eq!(equipment.left_ring(), left.as_mut_ptr());
        assert_eq!(equipment.right_ring(), right.as_mut_ptr());
    }

    #[test]
    fn current_level_initializes_once() {
        let current_level = CurrentLevel::EMPTY;

        let first = current_level.with_mut(|level| level as *mut Level);
        let second = current_level.with(|level| level as *const Level);

        assert_eq!(first.cast_const(), second);
    }
}
