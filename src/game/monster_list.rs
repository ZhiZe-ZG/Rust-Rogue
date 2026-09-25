//! Safe, Rust-idiomatic owner for the live monster list.
//!
//! The legacy C engine kept monsters in a raw `mlist` linked list; the earlier
//! Rust port wrapped the raw head pointer in a lock cell. This module replaces
//! that with a fully safe owner: monsters live in a slot vector and callers refer
//! to them through an opaque [`MonsterId`] handle. There are no raw-pointer
//! fields and no `unsafe` code in this module.
//!
//! Access is *scoped*: [`MonsterList::with`] and [`MonsterList::with_mut`] lend a
//! monster for the duration of a closure and release the lock afterwards, so the
//! single-threaded gameplay code can add or remove monsters between calls. To
//! iterate while mutating, snapshot the handles with [`MonsterList::ids`] first.
//!
//! The rest of the port still threads raw `*mut Thing` handles through combat
//! and level code; [`MonsterList::handle`] is the single, explicitly documented
//! bridge that produces such a handle from an owned monster, and
//! [`MonsterList::find`] maps an existing handle back to its [`MonsterId`].

use crate::entity::player::{Thing, ThingMonster};
use std::sync::{Mutex, MutexGuard};

/// Stable, opaque handle to a monster stored in [`MonsterList`].
///
/// The value is a slot index and stays valid until the monster is removed.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct MonsterId(usize);

impl MonsterId {
    /// The raw slot index (used by per-cell maps and the save layer).
    #[inline]
    pub const fn index(self) -> usize {
        self.0
    }

    /// Rebuild a handle from a raw slot index.
    #[inline]
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }
}

/// Owner of the live monsters for the current level.
///
/// Monsters are boxed so their address is stable for as long as the slot holds
/// them, which lets the legacy engine keep raw handles without invalidating
/// them when the backing vector grows.
pub struct MonsterList {
    slots: Mutex<Vec<Option<Box<Thing>>>>,
}

impl MonsterList {
    const fn new() -> Self {
        Self {
            slots: Mutex::new(Vec::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Option<Box<Thing>>>> {
        self.slots.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Store `thing` and return its handle, reusing a freed slot when possible.
    pub fn spawn(&self, thing: Thing) -> MonsterId {
        let mut slots = self.lock();
        if let Some(index) = slots.iter().position(Option::is_none) {
            slots[index] = Some(Box::new(thing));
            MonsterId(index)
        } else {
            slots.push(Some(Box::new(thing)));
            MonsterId(slots.len() - 1)
        }
    }

    /// Store a fresh default actor and return its handle.
    pub fn spawn_actor(&self) -> MonsterId {
        self.spawn(Thing::actor(ThingMonster::default()))
    }

    /// Remove and return the monster behind `id`, if still present.
    pub fn remove(&self, id: MonsterId) -> Option<Thing> {
        let mut slots = self.lock();
        slots
            .get_mut(id.0)
            .and_then(Option::take)
            .map(|boxed| *boxed)
    }

    /// Whether `id` still refers to a live monster.
    pub fn contains(&self, id: MonsterId) -> bool {
        self.lock().get(id.0).is_some_and(Option::is_some)
    }

    /// Drop every monster.
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Number of live monsters.
    pub fn len(&self) -> usize {
        self.lock().iter().filter(|slot| slot.is_some()).count()
    }

    /// Whether there are no monsters.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Live handles in slot order (the canonical traversal order).
    pub fn ids(&self) -> Vec<MonsterId> {
        self.lock()
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.as_ref().map(|_| MonsterId(index)))
            .collect()
    }

    /// Immutably access the monster behind `id`.
    pub fn with<R>(&self, id: MonsterId, operation: impl FnOnce(&Thing) -> R) -> Option<R> {
        let slots = self.lock();
        slots.get(id.0).and_then(Option::as_deref).map(operation)
    }

    /// Mutably access the monster behind `id`.
    pub fn with_mut<R>(&self, id: MonsterId, operation: impl FnOnce(&mut Thing) -> R) -> Option<R> {
        let mut slots = self.lock();
        slots
            .get_mut(id.0)
            .and_then(Option::as_deref_mut)
            .map(operation)
    }

    /// A stable raw handle to the monster behind `id`.
    ///
    /// This is the one bridge to the legacy `*mut Thing` engine boundary. The
    /// monster is boxed, so the address stays valid until the slot is cleared.
    /// Producing the pointer uses only safe casts (no `unsafe` block).
    pub fn handle(&self, id: MonsterId) -> Option<*mut Thing> {
        let slots = self.lock();
        slots
            .get(id.0)
            .and_then(Option::as_deref)
            .map(|thing| thing as *const Thing as *mut Thing)
    }

    /// A stable raw handle to the first live monster (or null when empty).
    ///
    /// The save layer uses this as the head of the monster sequence; the order
    /// matches [`MonsterList::ids`].
    pub fn head(&self) -> *mut Thing {
        self.ids()
            .first()
            .and_then(|&id| self.handle(id))
            .unwrap_or(std::ptr::null_mut())
    }

    /// Map an existing raw handle back to its [`MonsterId`].
    pub fn find(&self, handle: *mut Thing) -> Option<MonsterId> {
        if handle.is_null() {
            return None;
        }
        let slots = self.lock();
        slots
            .iter()
            .enumerate()
            .find_map(|(index, slot)| match slot.as_deref() {
                Some(thing) if thing as *const Thing == handle as *const Thing => {
                    Some(MonsterId(index))
                }
                _ => None,
            })
    }

    /// The traversal index of `id` among live monsters (for the save layer).
    pub fn position(&self, id: MonsterId) -> Option<usize> {
        self.ids().iter().position(|&other| other == id)
    }

    /// The handle at traversal `index`, if any.
    pub fn nth(&self, index: usize) -> Option<MonsterId> {
        self.ids().get(index).copied()
    }
}

/// The monster list for the live level.
pub static MONSTER_LIST: MonsterList = MonsterList::new();