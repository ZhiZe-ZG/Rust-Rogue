//! Rust-owned storage for game things.
//!
//! The game still uses raw pointers as stable handles because they are stored
//! in map cells and equipment slots. The allocations themselves are owned by
//! this vector, so list operations no longer depend on a C allocator or ABI.

use crate::entity::player::{Thing, ThingObject};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

/// Arena slot: a boxed, stable-address thing.
///
/// `Thing` already opts into `Send` (see `entity::player`), so `Box<Thing>` is
/// `Send` automatically and no explicit `unsafe impl` is needed here. The game
/// accesses the pointers from its single gameplay thread; the mutex only
/// protects the arena's ownership when the global is initialised.
struct OwnedThing(Box<Thing>);

static THINGS: OnceLock<Mutex<Vec<OwnedThing>>> = OnceLock::new();
static TOTAL: AtomicI32 = AtomicI32::new(0);

fn things() -> &'static Mutex<Vec<OwnedThing>> {
    THINGS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Prepend `item` to the actor `owner`'s pack list.
pub unsafe fn attach_pack(owner: *mut Thing, item: *mut Thing) {
    let head = crate::entity::player::thing_pack(owner);
    crate::entity::player::set_thing_next(item, head);
    crate::entity::player::set_thing_prev(item, std::ptr::null_mut());
    if !head.is_null() {
        crate::entity::player::set_thing_prev(head, item);
    }
    crate::entity::player::set_thing_pack(owner, item);
}

/// Unlink `item` from the global player's pack list.
///
/// The player is not reachable as a `*mut Thing` anymore, so this mirrors
/// [`detach_pack`] against the safe [`crate::game::PLAYER`] pack accessors.
pub unsafe fn detach_pack_from_player(item: *mut Thing) {
    let prev = crate::entity::player::thing_prev(item);
    let next = crate::entity::player::thing_next(item);

    if crate::game::PLAYER.pack() == item {
        crate::game::PLAYER.set_pack(next);
    }
    if !prev.is_null() {
        crate::entity::player::set_thing_next(prev, next);
    }
    if !next.is_null() {
        crate::entity::player::set_thing_prev(next, prev);
    }
    crate::entity::player::set_thing_next(item, std::ptr::null_mut());
    crate::entity::player::set_thing_prev(item, std::ptr::null_mut());
}

/// Unlink `item` from the actor `owner`'s pack list.
pub unsafe fn detach_pack(owner: *mut Thing, item: *mut Thing) {
    let prev = crate::entity::player::thing_prev(item);
    let next = crate::entity::player::thing_next(item);

    if crate::entity::player::thing_pack(owner) == item {
        crate::entity::player::set_thing_pack(owner, next);
    }
    if !prev.is_null() {
        crate::entity::player::set_thing_next(prev, next);
    }
    if !next.is_null() {
        crate::entity::player::set_thing_prev(next, prev);
    }
    crate::entity::player::set_thing_next(item, std::ptr::null_mut());
    crate::entity::player::set_thing_prev(item, std::ptr::null_mut());
}

/// Drop every item in the actor `owner`'s pack list.
pub unsafe fn free_pack(owner: *mut Thing) {
    let mut item = crate::entity::player::thing_pack(owner);
    while !item.is_null() {
        let next = crate::entity::player::thing_next(item);
        discard(item);
        item = next;
    }
    crate::entity::player::set_thing_pack(owner, std::ptr::null_mut());
}

/// Unlink `item` from a doubly-linked list, patching its neighbours and
/// clearing its own header.
pub unsafe fn detach(list: *mut *mut Thing, item: *mut Thing) {
    let prev = crate::entity::player::thing_prev(item);
    let next = crate::entity::player::thing_next(item);

    if *list == item {
        *list = next;
    }
    if !prev.is_null() {
        crate::entity::player::set_thing_next(prev, next);
    }
    if !next.is_null() {
        crate::entity::player::set_thing_prev(next, prev);
    }
    crate::entity::player::set_thing_next(item, std::ptr::null_mut());
    crate::entity::player::set_thing_prev(item, std::ptr::null_mut());
}

/// Prepend `item` to a doubly-linked list.
pub unsafe fn attach(list: *mut *mut Thing, item: *mut Thing) {
    crate::entity::player::set_thing_next(item, *list);
    crate::entity::player::set_thing_prev(item, std::ptr::null_mut());
    if !(*list).is_null() {
        crate::entity::player::set_thing_prev(*list, item);
    }
    *list = item;
}

pub unsafe fn free_list(list: *mut *mut Thing) {
    while !(*list).is_null() {
        let item = *list;
        *list = crate::entity::player::thing_next(item);
        discard(item);
    }
}

pub unsafe fn discard(item: *mut Thing) {
    // Monsters are owned by the pointer-free `MonsterList`; objects by the arena.
    if let Some(id) = crate::game::MONSTER_LIST.find(item) {
        crate::game::MONSTER_LIST.remove(id);
        return;
    }
    let mut things = things().lock().expect("thing store poisoned");
    if let Some(index) = things
        .iter()
        .position(|thing| (&*thing.0 as *const Thing).cast_mut() == item)
    {
        things.swap_remove(index);
        TOTAL.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Allocate an object (item) thing in the arena.
pub unsafe fn new_object() -> *mut Thing {
    let mut item = OwnedThing(Box::new(Thing::object(ThingObject::default())));
    let pointer = (&mut *item.0) as *mut Thing;
    things().lock().expect("thing store poisoned").push(item);
    TOTAL.fetch_add(1, Ordering::Relaxed);
    pointer
}

/// Allocate an actor (monster) thing.
///
/// Monsters are owned by the safe [`crate::game::MonsterList`] rather than this
/// arena; the returned raw handle is the monster's stable address and stays
/// valid until the monster is discarded.
pub unsafe fn new_actor() -> *mut Thing {
    let id = crate::game::MONSTER_LIST.spawn_actor();
    crate::game::MONSTER_LIST
        .handle(id)
        .unwrap_or(std::ptr::null_mut())
}

/// Allocate an object thing; the historical item-allocation entry point.
pub unsafe fn new_item() -> *mut Thing {
    new_object()
}

pub fn allocated_count() -> i32 {
    TOTAL.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::player::{thing_next, thing_prev};

    /// Serialise the arena assertions; the arena and its counter are process
    /// globals, so the tests must not interleave allocation/free from several
    /// threads.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Prepend/remove across head, middle, tail, and only-element cases, and
    /// confirm neighbour links and the head pointer are patched correctly.
    #[test]
    fn attach_detach_preserves_order_and_links() {
        let _guard = serial();
        let a = unsafe { new_object() };
        let b = unsafe { new_object() };
        let c = unsafe { new_object() };

        let mut head: *mut Thing = std::ptr::null_mut();
        unsafe {
            attach(&mut head, a);
            attach(&mut head, b);
            attach(&mut head, c);
        }

        // Head is the most recently attached; links read c, b, a.
        assert_eq!(head, c);
        unsafe {
            assert_eq!(thing_next(c), b);
            assert_eq!(thing_next(b), a);
            assert!(thing_next(a).is_null());
            assert!(thing_prev(c).is_null());
            assert_eq!(thing_prev(b), c);
            assert_eq!(thing_prev(a), b);
        }

        // Remove the middle element.
        unsafe {
            detach(&mut head, b);
        }
        assert_eq!(head, c);
        unsafe {
            assert_eq!(thing_next(c), a);
            assert_eq!(thing_prev(a), c);
            // The removed node's own header is cleared.
            assert!(thing_next(b).is_null());
            assert!(thing_prev(b).is_null());
        }

        // Remove the head element.
        unsafe {
            detach(&mut head, c);
        }
        assert_eq!(head, a);
        unsafe {
            assert!(thing_prev(a).is_null());
        }

        // Remove the only remaining element.
        unsafe {
            detach(&mut head, a);
        }
        assert!(head.is_null());

        unsafe {
            discard(a);
            discard(b);
            discard(c);
        }
    }

    /// Draining a list while traversing must capture each node's successor
    /// before the node is freed, and leave the list empty.
    #[test]
    fn removal_during_traversal_drains_list() {
        let _guard = serial();
        let a = unsafe { new_object() };
        let b = unsafe { new_object() };
        let c = unsafe { new_object() };

        let mut head: *mut Thing = std::ptr::null_mut();
        unsafe {
            attach(&mut head, a);
            attach(&mut head, b);
            attach(&mut head, c);
        }

        let before = allocated_count();
        let mut node = head;
        let mut visited = 0;
        unsafe {
            while !node.is_null() {
                // Capture the successor before the node is freed.
                let next = thing_next(node);
                visited += 1;
                discard(node);
                node = next;
            }
        }
        assert_eq!(visited, 3);
        // All three arena entries were released during the traversal.
        assert_eq!(allocated_count(), before - 3);
    }

    /// Allocating and discarding objects keeps the tracked count balanced.
    #[test]
    fn allocation_count_is_balanced() {
        let _guard = serial();
        let before = allocated_count();
        let a = unsafe { new_object() };
        let b = unsafe { new_object() };
        assert_eq!(allocated_count(), before + 2);
        unsafe {
            discard(a);
            discard(b);
        }
        assert_eq!(allocated_count(), before);
    }
}
