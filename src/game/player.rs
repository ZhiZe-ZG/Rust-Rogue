//! Process-wide player actor and equipment state.
//!
//! The legacy C engine kept the hero in a single global `player` variable and
//! the equipped armor/rings/weapon in individual globals. This module groups
//! them into one stable, process-lifetime owner:
//!
//! * [`PLAYER`] — the hero [`Thing`] plus the [`Equipment`] currently in use
//!   (the equipped objects themselves remain owned by the player's pack).
//!
//! The hero is stored behind an [`RwLock`] and reached only through scoped
//! `with`/`with_mut` closures plus a set of value accessors. There is no public
//! raw hero pointer: callers read or mutate individual fields through safe
//! methods, so the previous `player_ptr`/`Player::ptr` bridge is gone.
//!
//! Equipment slots are stored as [`NonNull`] handles rather than raw pointers: a
//! slot simply holds `None` when empty, so there is no null-pointer sentinel to
//! dereference.

use std::ptr::NonNull;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::entity::player::{MonsterFlags, Stats, Thing, ThingMonster};
use glam::IVec2;

/// Interior-mutable, lock-guarded slot holding an optional [`Thing`] handle.
///
/// The game is single-threaded, but the slot is reachable through the
/// process-wide [`PLAYER`] `static` (which must be `Sync`). [`NonNull`] is
/// neither `Send` nor `Sync`, so this wrapper opts in explicitly and only ever
/// dereferences the handle from the owning (single-threaded) game logic.
#[derive(Default)]
struct Slot(RwLock<Option<NonNull<Thing>>>);

// SAFETY: access to the handle is always guarded by the inner `RwLock`, and the
// handle is only dereferenced by the single-threaded gameplay loop.
unsafe impl Send for Slot {}
unsafe impl Sync for Slot {}

impl Slot {
    const fn new() -> Self {
        Self(RwLock::new(None))
    }

    #[inline]
    fn get(&self) -> *mut Thing {
        self.0
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .map_or(std::ptr::null_mut(), NonNull::as_ptr)
    }

    #[inline]
    fn set(&self, ptr: *mut Thing) {
        *self.0.write().unwrap_or_else(|poison| poison.into_inner()) = NonNull::new(ptr);
    }
}

/// The player's equipped objects.
///
/// Holds non-owning handles to the armor, the two rings, and the weapon the
/// player currently has in hand. A `null` handle means the slot is empty; the
/// objects remain owned by the player's pack (or the item arena).
pub struct Equipment {
    armor: Slot,
    rings: [Slot; 2],
    weapon: Slot,
}

impl Equipment {
    const EMPTY: Self = Self {
        armor: Slot::new(),
        rings: [Slot::new(), Slot::new()],
        weapon: Slot::new(),
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

/// The zero-valued actor [`Thing`] used to seed the global player.
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
            t_dest_hero: false,
            t_flags: MonsterFlags::NONE,
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

/// Borrow the actor payload of a hero `Thing`.
///
/// The global player is always constructed as a monster, so the object arm is
/// unreachable in practice.
#[inline]
fn hero_monster(thing: &Thing) -> &ThingMonster {
    match thing {
        Thing::Monster { data, .. } => data,
        Thing::Object { .. } => unreachable!("the player is always an actor"),
    }
}

/// Mutably borrow the actor payload of a hero `Thing`.
#[inline]
fn hero_monster_mut(thing: &mut Thing) -> &mut ThingMonster {
    match thing {
        Thing::Monster { data, .. } => data,
        Thing::Object { .. } => unreachable!("the player is always an actor"),
    }
}

/// A safe, process-wide owner for the hero actor and its equipment.
///
/// The game is single-threaded, but the player must be reachable from many
/// modules as one stable, process-lifetime object. The hero [`Thing`] lives
/// behind an `RwLock`; callers use the scoped [`Player::with`] /
/// [`Player::with_mut`] closures or the value accessors below, so no raw hero
/// pointer escapes the module. It replaces the legacy `#[no_mangle] static mut
/// player` global and folds in the former `EQUIPMENT` global.
pub struct Player {
    hero: RwLock<Option<Thing>>,
    equipment: Equipment,
}

impl Player {
    const EMPTY: Self = Self {
        hero: RwLock::new(None),
        equipment: Equipment::EMPTY,
    };

    /// Ensure the hero exists, initializing it on first access.
    #[inline]
    fn ensure_initialized(&self) {
        {
            let hero = self.hero.read().unwrap_or_else(|poison| poison.into_inner());
            if hero.is_some() {
                return;
            }
        }
        let mut hero = self.hero.write().unwrap_or_else(|poison| poison.into_inner());
        if hero.is_none() {
            *hero = Some(default_player());
        }
    }

    #[inline]
    fn read_hero(&self) -> RwLockReadGuard<'_, Option<Thing>> {
        self.ensure_initialized();
        self.hero.read().unwrap_or_else(|poison| poison.into_inner())
    }

    #[inline]
    fn write_hero(&self) -> RwLockWriteGuard<'_, Option<Thing>> {
        self.ensure_initialized();
        self.hero.write().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Run `operation` with immutable access to the hero.
    ///
    /// The closure must not call back into [`Player`] (the read lock is held
    /// for its duration); copy values out instead.
    #[inline]
    pub fn with<R>(&self, operation: impl FnOnce(&Thing) -> R) -> R {
        let hero = self.read_hero();
        operation(hero.as_ref().expect("hero initialized"))
    }

    /// Run `operation` with mutable access to the hero.
    ///
    /// The closure must not call back into [`Player`] (the write lock is held
    /// for its duration).
    #[inline]
    pub fn with_mut<R>(&self, operation: impl FnOnce(&mut Thing) -> R) -> R {
        let mut hero = self.write_hero();
        operation(hero.as_mut().expect("hero initialized"))
    }

    /// Run `operation` with immutable access to the hero's actor payload.
    #[inline]
    pub fn with_monster<R>(&self, operation: impl FnOnce(&ThingMonster) -> R) -> R {
        self.with(|thing| operation(hero_monster(thing)))
    }

    /// Run `operation` with mutable access to the hero's actor payload.
    #[inline]
    pub fn with_monster_mut<R>(&self, operation: impl FnOnce(&mut ThingMonster) -> R) -> R {
        self.with_mut(|thing| operation(hero_monster_mut(thing)))
    }

    /// The player's current map position.
    #[inline]
    pub fn pos(&self) -> IVec2 {
        self.with_monster(|hero| hero.t_pos)
    }

    /// Set the player's current map position.
    #[inline]
    pub fn set_pos(&self, pos: IVec2) {
        self.with_monster_mut(|hero| hero.t_pos = pos);
    }

    /// The player's current room reference.
    #[inline]
    pub fn room(&self) -> Option<usize> {
        self.with_monster(|hero| hero.t_room)
    }

    /// Set the player's current room reference.
    #[inline]
    pub fn set_room(&self, room: Option<usize>) {
        self.with_monster_mut(|hero| hero.t_room = room);
    }

    /// The player's actor flags.
    #[inline]
    pub fn flags(&self) -> MonsterFlags {
        self.with_monster(|hero| hero.t_flags)
    }

    /// Whether `flag` is set on the player.
    #[inline]
    pub fn has_flag(&self, flag: MonsterFlags) -> bool {
        self.with_monster(|hero| hero.t_flags.contains(flag))
    }

    /// Add `flag` to the player's actor flags.
    #[inline]
    pub fn add_flag(&self, flag: MonsterFlags) {
        self.with_monster_mut(|hero| hero.t_flags.insert(flag));
    }

    /// Remove `flag` from the player's actor flags.
    #[inline]
    pub fn remove_flag(&self, flag: MonsterFlags) {
        self.with_monster_mut(|hero| hero.t_flags.remove(flag));
    }

    /// The player's current statistics (copied).
    #[inline]
    pub fn stats(&self) -> Stats {
        self.with_monster(|hero| hero.t_stats)
    }

    /// Replace the player's statistics.
    #[inline]
    pub fn set_stats(&self, stats: Stats) {
        self.with_monster_mut(|hero| hero.t_stats = stats);
    }

    /// Run `operation` with mutable access to the player's statistics.
    #[inline]
    pub fn with_stats_mut<R>(&self, operation: impl FnOnce(&mut Stats) -> R) -> R {
        self.with_monster_mut(|hero| operation(&mut hero.t_stats))
    }

    /// The player's current experience level.
    #[inline]
    pub fn level(&self) -> i32 {
        self.with_monster(|hero| hero.t_stats.level)
    }

    /// The player's pack head as a raw handle (or null when empty).
    #[inline]
    pub fn pack(&self) -> *mut Thing {
        self.with_monster(|hero| hero.t_pack.map_or(std::ptr::null_mut(), |p| p.as_ptr()))
    }

    /// Set the player's pack head from a raw handle.
    #[inline]
    pub fn set_pack(&self, pack: *mut Thing) {
        self.with_monster_mut(|hero| hero.t_pack = NonNull::new(pack));
    }

    /// The player's equipped objects.
    #[inline]
    pub fn equipment(&self) -> &Equipment {
        &self.equipment
    }

    /// The armor the player is wearing (or a null handle).
    #[inline]
    pub fn armor(&self) -> *mut Thing {
        self.equipment.armor()
    }

    /// The protection value of the currently worn armor, or `None` when the
    /// player is not wearing any armor.
    #[inline]
    pub fn armor_value(&self) -> Option<i32> {
        let armor = self.armor();
        if armor.is_null() {
            None
        } else {
            Some(unsafe { (*crate::entity::player::thing_o(armor)).o_arm })
        }
    }

    /// Set (or clear) the armor the player is wearing.
    #[inline]
    pub fn set_armor(&self, armor: *mut Thing) {
        self.equipment.set_armor(armor);
    }

    /// The ring on the player's left hand (or a null handle).
    #[inline]
    pub fn left_ring(&self) -> *mut Thing {
        self.equipment.left_ring()
    }

    /// The ring on the player's right hand (or a null handle).
    #[inline]
    pub fn right_ring(&self) -> *mut Thing {
        self.equipment.right_ring()
    }

    /// Set (or clear) the ring on the player's left hand.
    #[inline]
    pub fn set_left_ring(&self, ring: *mut Thing) {
        self.equipment.set_left_ring(ring);
    }

    /// Set (or clear) the ring on the player's right hand.
    #[inline]
    pub fn set_right_ring(&self, ring: *mut Thing) {
        self.equipment.set_right_ring(ring);
    }

    /// The weapon the player is wielding (or a null handle).
    #[inline]
    pub fn weapon(&self) -> *mut Thing {
        self.equipment.weapon()
    }

    /// Set (or clear) the weapon the player is wielding.
    #[inline]
    pub fn set_weapon(&self, weapon: *mut Thing) {
        self.equipment.set_weapon(weapon);
    }
}

/// Process-wide owner of the player's actor and equipment.
pub static PLAYER: Player = Player::EMPTY;

/// Remove `flag` from the global player's actor flags.
#[inline]
pub fn player_remove_flag(flag: MonsterFlags) {
    PLAYER.remove_flag(flag);
}

#[cfg(test)]
mod tests {
    use super::{Equipment, Player};
    use crate::entity::player::{MonsterFlags, Thing};
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
    fn player_defaults_and_mutates_through_accessors() {
        let player = Player::EMPTY;
        player.set_pos(glam::IVec2 { x: 3, y: 4 });
        assert_eq!(player.pos(), glam::IVec2 { x: 3, y: 4 });
        player.add_flag(MonsterFlags::BLIND);
        assert!(player.has_flag(MonsterFlags::BLIND));
        player.remove_flag(MonsterFlags::BLIND);
        assert!(!player.has_flag(MonsterFlags::BLIND));
        player.with_stats_mut(|stats| stats.hit_points = 7);
        assert_eq!(player.stats().hit_points, 7);
    }

    #[test]
    fn equipment_is_a_player_component() {
        let player = Player::EMPTY;
        assert!(player.equipment().weapon().is_null());
        player.equipment().set_weapon(std::ptr::null_mut());
        assert!(player.equipment().armor().is_null());
    }
}