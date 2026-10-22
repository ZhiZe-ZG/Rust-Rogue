//! Dungeon-level generation and lifecycle orchestration.
//!
//! Owns the [`Dungeon`] singleton (the live level plus its `max_level`/`no_food`
//! counters) and the level-transition entry points ([`Dungeon::new_level`] and
//! [`Dungeon::door_open`]) that build and populate a fresh dungeon level,
//! driving the room/passage generation and population passes owned by
//! [`crate::level`].

use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use std::sync::RwLock;

use glam::IVec2;

use crate::game::Player;
use crate::item::arena::ThingArena;
use crate::level::Level;
use crate::rnd::Random;
use crate::tile::Tile;

mod generation;
mod monster_list;
mod monster_map;
mod presence;

pub use monster_list::{MonsterId, MonsterList};
pub use monster_map::MonsterMap;

/// Process-wide owner of the live dungeon level, its counters, and the live
/// monsters, objects, and player.
///
/// Replaces the scattered process-wide globals of the legacy engine (the
/// `CURRENT_LEVEL` singleton, the `max_level`/`no_food` counters, the
/// `amulet`/`ntraps`/`seenstairs` counters, and the
/// `mlist`/`DUNGEON.monster_map` monster owners) with one owner, keeping the
/// same scoped-closure access pattern (see [`Dungeon::with_level`]).
pub struct Dungeon {
    /// The live [`Level`], created lazily on first access.
    level: RwLock<Option<Level>>,
    /// Highest dungeon depth reached so far.
    max_level: AtomicI32,
    /// Whether food generation is disabled.
    no_food: AtomicI32,
    /// Number of traps on the current level.
    ntraps: AtomicI32,
    /// Whether the player has seen the current level's staircase.
    seenstairs: AtomicU8,
    /// Whether the player carries the amulet.
    amulet: AtomicU8,
    /// The live monsters for the current level.
    pub monster_list: MonsterList,
    /// The per-cell monster occupancy grid for the current level.
    pub monster_map: MonsterMap,
    /// Generational arena owning the dungeon's live objects (including pack items).
    pub objects: ThingArena,
    /// The hero actor and its equipped object handles.
    pub player: Player,
}

impl Dungeon {
    /// Unlink all floor items, then discard them from the object arena.
    /// The level lock is released before acquiring any arena lock.
    pub fn clear_items(&self) {
        let items = self.with_level_mut(|level| std::mem::take(&mut level.items));
        for id in items {
            let _ = self.objects.remove(id);
        }
    }

    /// Build an empty dungeon, ready for lazy initialization.
    pub const fn empty() -> Self {
        Self {
            level: RwLock::new(None),
            max_level: AtomicI32::new(0),
            no_food: AtomicI32::new(0),
            ntraps: AtomicI32::new(0),
            seenstairs: AtomicU8::new(0),
            amulet: AtomicU8::new(0),
            monster_list: MonsterList::new(),
            monster_map: MonsterMap::new(),
            objects: ThingArena::new(),
            player: Player::EMPTY,
        }
    }

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

    /// Run `operation` with immutable access to the live level.
    #[inline]
    pub fn with_level<R>(&self, operation: impl FnOnce(&Level) -> R) -> R {
        self.ensure_initialized();
        let level = self
            .level
            .read()
            .unwrap_or_else(|poison| poison.into_inner());
        operation(level.as_ref().unwrap())
    }

    /// Run `operation` with mutable access to the live level.
    #[inline]
    pub fn with_level_mut<R>(&self, operation: impl FnOnce(&mut Level) -> R) -> R {
        self.ensure_initialized();
        let mut level = self
            .level
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        operation(level.as_mut().unwrap())
    }

    /// The current dungeon depth (`Level::depth`).
    #[inline]
    pub fn current_depth(&self) -> i32 {
        self.with_level(|level| level.depth)
    }

    /// Set the current dungeon depth (`Level::depth`).
    #[inline]
    pub fn set_current_depth(&self, depth: i32) {
        self.with_level_mut(|level| level.depth = depth);
    }

    /// The current down-staircase position (`Level::stairs`).
    #[inline]
    pub fn stairs(&self) -> IVec2 {
        self.with_level(|level| level.stairs)
    }

    /// Set the current down-staircase position (`Level::stairs`).
    #[inline]
    pub fn set_stairs(&self, pos: IVec2) {
        self.with_level_mut(|level| level.stairs = pos);
    }

    /// The highest dungeon depth reached so far.
    #[inline]
    pub fn max_depth(&self) -> i32 {
        self.max_level.load(Ordering::Relaxed)
    }

    /// Set the recorded maximum dungeon depth.
    #[inline]
    pub fn set_max_depth(&self, depth: i32) {
        self.max_level.store(depth, Ordering::Relaxed);
    }

    /// Raise the recorded maximum dungeon depth to at least `depth`.
    #[inline]
    pub fn record_max_depth(&self, depth: i32) {
        self.max_level.fetch_max(depth, Ordering::Relaxed);
    }

    /// The `no_food` counter.
    #[inline]
    pub fn no_food(&self) -> i32 {
        self.no_food.load(Ordering::Relaxed)
    }

    /// Set the `no_food` counter.
    #[inline]
    pub fn set_no_food(&self, value: i32) {
        self.no_food.store(value, Ordering::Relaxed);
    }

    /// Increment the `no_food` counter and return the new value.
    #[inline]
    pub fn bump_no_food(&self) -> i32 {
        self.no_food.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// The number of traps on the current level.
    #[inline]
    pub fn ntraps(&self) -> i32 {
        self.ntraps.load(Ordering::Relaxed)
    }

    /// Set the number of traps on the current level.
    #[inline]
    pub fn set_ntraps(&self, value: i32) {
        self.ntraps.store(value, Ordering::Relaxed);
    }

    /// Whether the player has seen the current level's staircase.
    #[inline]
    pub fn seen_stairs(&self) -> bool {
        self.seenstairs.load(Ordering::Relaxed) != 0
    }

    /// Set whether the player has seen the current level's staircase.
    #[inline]
    pub fn set_seen_stairs(&self, seen: bool) {
        self.seenstairs.store(seen as u8, Ordering::Relaxed);
    }

    /// Whether the player carries the amulet.
    #[inline]
    pub fn has_amulet(&self) -> bool {
        self.amulet.load(Ordering::Relaxed) != 0
    }

    /// Set whether the player carries the amulet.
    #[inline]
    pub fn set_amulet(&self, carries: bool) {
        self.amulet.store(carries as u8, Ordering::Relaxed);
    }

    /// Find a floor cell in the live level to place something, optionally
    /// avoiding monsters.
    ///
    /// If `room_idx` is `None` a random room slot is tried each iteration via
    /// [`Level::rnd_room`]; otherwise the cell is chosen inside that room. The
    /// candidate cell is validated against the level's tile map and the
    /// per-cell monster occupancy grid. Returns the chosen cell, or `None` when
    /// `limit` (if nonzero) attempts are exhausted.
    pub(crate) fn find_floor(
        &self,
        room_idx: Option<usize>,
        limit: i32,
        monst: bool,
        random: &mut Random,
    ) -> Option<IVec2> {
        self.with_level(|level| {
            let mut cnt = limit;
            // Safety bound: unlimited scans must eventually give up rather than
            // hang level generation on a packed level.
            let mut guard = 0u32;
            loop {
                if limit != 0 {
                    if cnt == 0 {
                        return None;
                    }
                    cnt -= 1;
                }
                guard += 1;
                if guard > 1_000_000 {
                    return None;
                }

                let idx = room_idx.unwrap_or_else(|| level.rnd_room(random));
                let room = &level.rooms[idx];
                let expected_tile = if room.is_maze() {
                    Tile::Passage
                } else {
                    Tile::Floor
                };
                let pos = level.rnd_pos(room, random);

                // The candidate cell is validated against the map tile
                // directly; an object overlay does not count as a free cell.
                let tile = level.tile_at(pos.y as usize, pos.x as usize);

                if monst {
                    let occupied = self
                        .monster_map
                        .at(pos.y as usize, pos.x as usize)
                        .is_some();
                    if !occupied && tile.is_walkable() {
                        return Some(pos);
                    }
                } else if tile == expected_tile {
                    return Some(pos);
                }
            }
        })
    }
}

/// Process-wide dungeon singleton.
pub static DUNGEON: Dungeon = Dungeon::empty();

// ---------------------------------------------------------------------------
// Dungeon-global accessors
// ---------------------------------------------------------------------------
//
// Small helpers so callers read/update the dungeon counters without touching
// the singleton directly.

/// Raise the recorded maximum dungeon depth to at least `depth`.
pub(crate) fn record_max_depth(depth: i32) {
    DUNGEON.record_max_depth(depth);
}

/// The highest dungeon depth reached so far.
pub(crate) fn max_depth() -> i32 {
    DUNGEON.max_depth()
}

/// Set the recorded maximum dungeon depth.
pub(crate) fn set_max_depth(depth: i32) {
    DUNGEON.set_max_depth(depth);
}

/// The current `no_food` counter.
pub(crate) fn no_food() -> i32 {
    DUNGEON.no_food()
}

/// Set the `no_food` counter.
pub(crate) fn set_no_food(value: i32) {
    DUNGEON.set_no_food(value);
}

/// Increment the `no_food` counter.
pub(crate) fn bump_no_food() {
    DUNGEON.bump_no_food();
}

#[cfg(test)]
mod tests {
    use super::Dungeon;

    #[test]
    fn players_are_owned_by_each_dungeon() {
        use crate::entity::player::MonsterFlags;
        use glam::IVec2;

        let first = Dungeon::empty();
        let second = Dungeon::empty();
        let weapon = first.objects.new_item_id();
        first.player.set_pos(IVec2::new(3, 4));
        first.player.set_room(Some(2));
        first.player.add_flag(MonsterFlags::BLIND);
        first.player.with_stats_mut(|stats| stats.hit_points = 7);
        first.player.push_pack_front(weapon);
        first.player.set_weapon_id(Some(weapon));

        assert_eq!(second.player.pos(), IVec2::ZERO);
        assert_eq!(second.player.room(), None);
        assert!(!second.player.has_flag(MonsterFlags::BLIND));
        assert_eq!(second.player.stats().hit_points, 0);
        assert!(second.player.pack().is_empty());
        assert_eq!(second.player.weapon_id(), None);

        second.player.set_pos(IVec2::new(5, 6));
        assert_eq!(first.player.pos(), IVec2::new(3, 4));
        assert_eq!(first.player.room(), Some(2));
        assert!(first.player.has_flag(MonsterFlags::BLIND));
        assert_eq!(first.player.stats().hit_points, 7);
        assert_eq!(first.player.pack(), vec![weapon]);
        assert_eq!(first.player.weapon_id(), Some(weapon));
    }

    #[test]
    fn clear_items_removes_only_floor_objects_and_is_repeatable() {
        use crate::entity::player::{Thing, ThingObject};
        let dungeon = Dungeon::empty();
        let floor = dungeon.objects.insert(Thing::object(ThingObject::default()));
        let stale = dungeon.objects.insert(Thing::object(ThingObject::default()));
        dungeon.objects.remove(stale);
        let unrelated = dungeon.objects.insert(Thing::object(ThingObject::default()));
        dungeon.with_level_mut(|level| {
            level.add_item(floor);
            level.add_item(stale);
            level.add_item(floor);
        });

        dungeon.clear_items();
        assert!(dungeon.with_level(|level| level.item_ids().is_empty()));
        assert!(!dungeon.objects.contains(floor));
        assert!(dungeon.objects.contains(unrelated));
        dungeon.clear_items();
        assert!(dungeon.objects.contains(unrelated));
        dungeon.objects.remove(unrelated);
    }

    #[test]
    fn object_arenas_are_owned_by_each_dungeon() {
        let first = Dungeon::empty();
        let second = Dungeon::empty();
        let id = first.objects.new_item_id();
        assert!(first.objects.contains(id));
        assert!(second.objects.is_empty());
        assert!(!second.objects.contains(id));
        first.with_level_mut(|level| level.add_item(id));
        second.clear_items();
        assert!(first.objects.contains(id));
        first.clear_items();
        assert!(first.objects.is_empty());
    }

    #[test]
    fn level_initializes_once_and_shares_scoped_views() {
        let dungeon = Dungeon::empty();

        // A mutation through `with_level_mut` is visible to a later
        // `with_level`, confirming both scoped views observe the same
        // lazily-created level.
        dungeon.with_level_mut(|level| level.depth = 7);
        assert_eq!(dungeon.with_level(|level| level.depth), 7);
    }

    #[test]
    fn counters_round_trip() {
        let dungeon = Dungeon::empty();
        dungeon.record_max_depth(3);
        dungeon.record_max_depth(2);
        assert_eq!(dungeon.max_depth(), 3);
        dungeon.set_max_depth(9);
        assert_eq!(dungeon.max_depth(), 9);

        assert_eq!(dungeon.no_food(), 0);
        assert_eq!(dungeon.bump_no_food(), 1);
        dungeon.set_no_food(5);
        assert_eq!(dungeon.no_food(), 5);

        dungeon.set_ntraps(4);
        assert_eq!(dungeon.ntraps(), 4);
        assert!(!dungeon.seen_stairs());
        dungeon.set_seen_stairs(true);
        assert!(dungeon.seen_stairs());
        assert!(!dungeon.has_amulet());
        dungeon.set_amulet(true);
        assert!(dungeon.has_amulet());
    }
}