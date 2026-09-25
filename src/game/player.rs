//! Process-wide player actor and equipment state.
//!
//! The legacy C engine kept the hero in a single global `player` variable and
//! the equipped armor/rings/weapon in individual globals. This module groups
//! those into stable, process-lifetime owners:
//!
//! * [`PLAYER`] — the boxed hero [`Thing`] whose address never changes, reached
//!   through [`player_ptr`];
//! * [`EQUIPMENT`] — non-owning handles to the objects currently equipped by
//!   the player (the objects themselves remain owned by the player's pack).

use std::sync::{OnceLock, RwLock};

use crate::entity::player::{MonsterFlags, Stats, Thing, ThingMonster};
use glam::IVec2;

/// A non-owning, interior-mutable cell for a raw [`Thing`] pointer.
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

/// A safe, process-wide holder for the player's actor [`Thing`].
///
/// The game is single-threaded, but the player must be reachable from many
/// modules as one stable, process-lifetime object whose address never changes
/// (chase targets and save code keep raw pointers to `t_pos`/`t_stats`). The
/// `Thing` is boxed and initialized exactly once, so its heap address is
/// stable and [`Player::ptr`] always hands back the same `*mut Thing`. It
/// replaces the legacy `#[no_mangle] static mut player` global.
pub struct Player {
    thing: OnceLock<Box<Thing>>,
}

// SAFETY: the game runs on a single thread, and the boxed `Thing` is only ever
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

    /// A stable, process-lifetime pointer to the player `Thing`.
    #[inline]
    pub fn ptr(&self) -> *mut Thing {
        self.get() as *const Thing as *mut Thing
    }
}

/// Process-wide owner of the player's actor [`Thing`].
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
    unsafe { (*crate::entity::player::thing_t(player_ptr())).t_flags.remove(flag) }
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
}