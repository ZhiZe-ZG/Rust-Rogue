//! Generational arena owning game objects (items).
//!
//! This is the Rust-native replacement for the legacy `Box` arena whose
//! allocations were addressed by raw `*mut Thing` pointers. Callers refer to a
//! stored object through an opaque, generational [`ThingId`] handle; a handle
//! to a slot that has since been freed and reused is rejected, so stale handles
//! cannot silently alias a new object.
//!
//! Access is *scoped*: [`ThingArena::with`] and [`ThingArena::with_mut`] lend a
//! thing for the duration of a closure and release the lock afterwards, which
//! lets the single-threaded gameplay code add or remove objects between calls
//! (mirroring [`crate::game::MonsterList`]).
//!
//! The rest of the port still threads raw `*mut Thing` handles through combat
//! and level code. [`ThingArena::ptr`] and [`ThingArena::id_for_ptr`] are the
//! single, explicitly documented bridge that maps between that legacy form and
//! an owned [`ThingId`]; they exist only while the raw-pointer callers migrate.
//! Objects are boxed, so an address produced by [`ThingArena::ptr`] stays valid
//! until the object is removed from the arena.

use crate::entity::player::{Thing, ThingObject};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, MutexGuard};

/// Stable, opaque handle to a thing stored in [`ThingArena`].
///
/// The `index` locates the slot and the `generation` distinguishes the current
/// occupant from any object previously stored in the same slot, so a handle to
/// a freed object never validates against a reused slot.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ThingId {
    index: usize,
    generation: u32,
}

impl ThingId {
    /// The raw slot index (used by tests and adapters).
    #[inline]
    pub const fn index(self) -> usize {
        self.index
    }

    /// The slot generation that this handle was issued for.
    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// One arena slot: a generation counter plus the current boxed object.
struct Slot {
    generation: u32,
    thing: Option<Box<Thing>>,
}

/// Mutable arena state guarded by [`ThingArena::inner`].
struct ArenaInner {
    slots: Vec<Slot>,
    /// Indices of empty slots available for reuse, newest first.
    free: Vec<usize>,
    live: usize,
}

/// Owner of a set of things addressed by [`ThingId`] handles.
pub struct ThingArena {
    inner: Mutex<ArenaInner>,
    /// Cheap lock-free mirror of `ArenaInner::live` for counters.
    live_count: AtomicI32,
}

impl ThingArena {
    /// An empty arena. Usable in `const`/`static` initialisers.
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(ArenaInner {
                slots: Vec::new(),
                free: Vec::new(),
                live: 0,
            }),
            live_count: AtomicI32::new(0),
        }
    }

    fn lock(&self) -> MutexGuard<'_, ArenaInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Store `thing` and return a fresh handle, reusing a freed slot if any.
    pub fn insert(&self, thing: Thing) -> ThingId {
        let mut arena = self.lock();
        let id = if let Some(index) = arena.free.pop() {
            let slot = &mut arena.slots[index];
            // `generation` was already advanced when the slot was freed, so the
            // reused slot is distinguishable from every prior occupant.
            slot.thing = Some(Box::new(thing));
            ThingId {
                index,
                generation: slot.generation,
            }
        } else {
            let index = arena.slots.len();
            arena.slots.push(Slot {
                generation: 0,
                thing: Some(Box::new(thing)),
            });
            ThingId {
                index,
                generation: 0,
            }
        };
        arena.live += 1;
        self.live_count.store(arena.live as i32, Ordering::Relaxed);
        id
    }

    /// Remove and return the thing behind `id`, if the handle is still valid.
    pub fn remove(&self, id: ThingId) -> Option<Thing> {
        let mut arena = self.lock();
        let slot = arena.slots.get_mut(id.index)?;
        if slot.generation != id.generation {
            return None;
        }
        let thing = slot.thing.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        arena.free.push(id.index);
        arena.live -= 1;
        self.live_count.store(arena.live as i32, Ordering::Relaxed);
        Some(*thing)
    }

    /// Whether `id` still refers to a live object.
    pub fn contains(&self, id: ThingId) -> bool {
        let arena = self.lock();
        arena
            .slots
            .get(id.index)
            .is_some_and(|slot| slot.generation == id.generation && slot.thing.is_some())
    }

    /// Immutably access the object behind `id`.
    pub fn with<R>(&self, id: ThingId, operation: impl FnOnce(&Thing) -> R) -> Option<R> {
        let arena = self.lock();
        arena
            .slots
            .get(id.index)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.thing.as_deref())
            .map(operation)
    }

    /// Mutably access the object behind `id`.
    pub fn with_mut<R>(&self, id: ThingId, operation: impl FnOnce(&mut Thing) -> R) -> Option<R> {
        let mut arena = self.lock();
        arena
            .slots
            .get_mut(id.index)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.thing.as_deref_mut())
            .map(operation)
    }

    /// Number of live objects.
    pub fn len(&self) -> usize {
        self.live_count.load(Ordering::Relaxed).max(0) as usize
    }

    /// Whether the arena holds no live objects.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Live handles in slot order (the canonical traversal order).
    pub fn ids(&self) -> Vec<ThingId> {
        let arena = self.lock();
        arena
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.thing.as_ref().map(|_| ThingId {
                    index,
                    generation: slot.generation,
                })
            })
            .collect()
    }

    /// A stable raw handle to the object behind `id` (null when stale).
    ///
    /// This is the one bridge to the legacy `*mut Thing` engine boundary. The
    /// object is boxed, so the address stays valid until it is removed.
    /// Producing the pointer uses only safe casts (no `unsafe` block).
    pub fn ptr(&self, id: ThingId) -> *mut Thing {
        let arena = self.lock();
        arena
            .slots
            .get(id.index)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.thing.as_deref())
            .map_or(std::ptr::null_mut(), |thing| {
                thing as *const Thing as *mut Thing
            })
    }

    /// Map an existing raw handle back to its [`ThingId`].
    pub fn id_for_ptr(&self, ptr: *mut Thing) -> Option<ThingId> {
        if ptr.is_null() {
            return None;
        }
        let arena = self.lock();
        arena
            .slots
            .iter()
            .enumerate()
            .find_map(|(index, slot)| match slot.thing.as_deref() {
                Some(thing) if thing as *const Thing == ptr as *const Thing => Some(ThingId {
                    index,
                    generation: slot.generation,
                }),
                _ => None,
            })
    }

    /// Remove the object at raw handle `ptr`, if still present.
    pub fn remove_by_ptr(&self, ptr: *mut Thing) -> Option<Thing> {
        let id = self.id_for_ptr(ptr)?;
        self.remove(id)
    }

    /// Store a fresh default object and return its stable raw handle.
    ///
    /// The handle is the boxed object's address, valid until it is removed.
    /// Producing it uses only safe casts (no `unsafe` block).
    pub fn new_object(&self) -> *mut Thing {
        let id = self.insert(Thing::object(ThingObject::default()));
        self.ptr(id)
    }

    /// Store a fresh default object; the historical item-allocation entry point.
    pub fn new_item(&self) -> *mut Thing {
        self.new_object()
    }

    /// Remove and drop the object at raw handle `ptr`, reporting whether it was
    /// present. A no-op for a null or already-freed handle.
    pub fn discard(&self, ptr: *mut Thing) -> bool {
        self.remove_by_ptr(ptr).is_some()
    }

    /// Number of live objects (the historical allocation counter).
    pub fn allocated_count(&self) -> i32 {
        self.len() as i32
    }
}

impl Default for ThingArena {
    fn default() -> Self {
        Self::new()
    }
}

/// Generational owner of every live object (item) thing.
///
/// Callers that have migrated keep a [`ThingId`]; this global is reached through
/// the safe allocation/discard helpers below, which return the stable address of
/// the boxed object for the pointer-based engine boundary.
pub static OBJECTS: ThingArena = ThingArena::new();

/// Allocate an object (item) thing in the arena and return its stable handle.
pub fn new_object() -> *mut Thing {
    OBJECTS.new_object()
}

/// Allocate an object thing; the historical item-allocation entry point.
pub fn new_item() -> *mut Thing {
    OBJECTS.new_item()
}

/// Number of live objects tracked by the global arena.
pub fn allocated_count() -> i32 {
    OBJECTS.allocated_count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::player::ThingObject;

    fn object() -> Thing {
        Thing::object(ThingObject::default())
    }

    /// Insert/get round-trips and the length tracks live entries.
    #[test]
    fn insert_and_access_round_trips() {
        let arena = ThingArena::new();
        let a = arena.insert(object());
        assert_eq!(arena.len(), 1);
        assert!(arena.contains(a));
        assert_eq!(arena.with(a, |_| ()), Some(()));
        assert_eq!(arena.ids(), vec![a]);
        arena.remove(a);
        assert!(arena.is_empty());
    }

    /// A handle to a removed object is rejected, and the freed slot's next
    /// occupant gets a distinct generation.
    #[test]
    fn stale_handle_is_rejected_after_reuse() {
        let arena = ThingArena::new();
        let stale = arena.insert(object());
        assert!(arena.remove(stale).is_some());

        // Reusing the freed slot yields a different generation.
        let fresh = arena.insert(object());
        assert_eq!(fresh.index(), stale.index());
        assert_ne!(fresh.generation(), stale.generation());

        // The old handle no longer resolves.
        assert!(!arena.contains(stale));
        assert_eq!(arena.with(stale, |_| ()), None);
        assert_eq!(arena.with_mut(stale, |_| ()), None);
        assert!(arena.remove(stale).is_none());

        // The new handle resolves.
        assert!(arena.contains(fresh));
    }

    /// The raw-pointer bridge round-trips and refuses stale addresses.
    #[test]
    fn pointer_bridge_round_trips() {
        let arena = ThingArena::new();
        let id = arena.insert(object());
        let raw = arena.ptr(id);
        assert!(!raw.is_null());
        assert_eq!(arena.id_for_ptr(raw), Some(id));
        assert!(arena.remove_by_ptr(raw).is_some());
        // After removal the same address no longer maps to a live object.
        assert!(arena.id_for_ptr(raw).is_none());
        assert!(arena.remove_by_ptr(raw).is_none());
        assert_eq!(arena.id_for_ptr(std::ptr::null_mut()), None);
    }

    /// Scoped mutable access can mutate and is released afterwards.
    #[test]
    fn scoped_mutation_is_visible() {
        let arena = ThingArena::new();
        let id = arena.insert(object());
        arena.with_mut(id, |thing| {
            if let Thing::Object { data, .. } = thing {
                data.o_count = 7;
            }
        });
        let count = arena.with(id, |thing| match thing {
            Thing::Object { data, .. } => data.o_count,
            _ => -1,
        });
        assert_eq!(count, Some(7));
    }
}
