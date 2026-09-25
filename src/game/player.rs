//! Process-wide player actor and equipment state.
//!
//! The legacy C engine kept the hero in a single global `player` variable and
//! the equipped armor/rings/weapon in individual globals. This module groups
//! them into one stable, process-lifetime owner:
//!
//! * [`PLAYER`] — the boxed hero [`Thing`] whose address never changes, reached
//!   through [`player_ptr`], plus the [`Equipment`] currently in use (the
//!   equipped objects themselves remain owned by the player's pack).
//!
//! Equipment slots and the hero address are stored as [`NonNull`] handles
//! rather than raw pointers: a slot simply holds `None` when empty, so there is
//! no null-pointer sentinel to dereference.

use std::ptr::NonNull;
use std::sync::{OnceLock, RwLock};

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

/// A safe, process-wide owner for the hero actor and its equipment.
///
/// The game is single-threaded, but the player must be reachable from many
/// modules as one stable, process-lifetime object whose address never changes
/// (chase targets and save code keep raw pointers to `t_pos`/`t_stats`). The
/// hero [`Thing`] is boxed and initialized exactly once, so its heap address is
/// stable and [`Player::ptr`] always hands back the same `*mut Thing`. It
/// replaces the legacy `#[no_mangle] static mut player` global and folds in the
/// former `EQUIPMENT` global.
pub struct Player {
    hero: OnceLock<Box<Thing>>,
    equipment: Equipment,
}

// SAFETY: the game runs on a single thread, and the boxed `Thing` is only ever
// reached through the stable raw pointer returned by `ptr`.
unsafe impl Sync for Player {}

impl Player {
    const EMPTY: Self = Self {
        hero: OnceLock::new(),
        equipment: Equipment::EMPTY,
    };

    #[inline]
    fn get(&self) -> &Thing {
        self.hero.get_or_init(|| Box::new(default_player()))
    }

    /// A stable, process-lifetime pointer to the player `Thing`.
    #[inline]
    pub fn ptr(&self) -> *mut Thing {
        self.get() as *const Thing as *mut Thing
    }

    /// The player's current statistics.
    #[inline]
    pub fn stats(&self) -> &Stats {
        unsafe { &(*crate::entity::player::thing_t(self.ptr())).t_stats }
    }

    /// The player's current map position.
    #[inline]
    pub fn pos(&self) -> IVec2 {
        unsafe { (*crate::entity::player::thing_t(self.ptr())).t_pos }
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

    /// Remove `flag` from the player's actor flags.
    #[inline]
    pub fn remove_flag(&self, flag: MonsterFlags) {
        unsafe { (*crate::entity::player::thing_t(self.ptr())).t_flags.remove(flag) }
    }
}

/// Process-wide owner of the player's actor and equipment.
pub static PLAYER: Player = Player::EMPTY;

/// A stable, process-lifetime pointer to the global player [`Thing`].
///
/// This is the safe replacement for the legacy `&raw mut player` accesses: the
/// `Thing` is boxed once inside [`PLAYER`], so the returned address never
/// changes.
#[inline]
pub fn player_ptr() -> *mut Thing {
    PLAYER.ptr()
}

/// Remove `flag` from the global player's actor flags.
#[inline]
pub fn player_remove_flag(flag: MonsterFlags) {
    PLAYER.remove_flag(flag);
}

#[cfg(test)]
mod tests {
    use super::{Equipment, Player};
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
    fn player_pointer_is_stable() {
        let player = Player::EMPTY;
        let first = player.ptr();
        let second = player.ptr();
        assert_eq!(first, second);
    }

    #[test]
    fn equipment_is_a_player_component() {
        let player = Player::EMPTY;
        assert!(player.equipment().weapon().is_null());
        player.equipment().set_weapon(std::ptr::null_mut());
        assert!(player.equipment().armor().is_null());
    }
}