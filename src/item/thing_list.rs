//! Rust-owned storage for game things.
//!
//! The game still uses raw pointers as stable handles because they are stored
//! in map cells and equipment slots. The allocations themselves are owned by
//! this vector, so list operations no longer depend on a C allocator or ABI.

use crate::entity::player::{Thing, ThingMonster, ThingObject};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

struct OwnedThing(Box<Thing>);

// The game accesses thing pointers on its single gameplay thread; the mutex
// only protects the arena's ownership when the global is initialized.
unsafe impl Send for OwnedThing {}

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
    if let Some(id) = crate::game::MLIST.find(item) {
        crate::game::MLIST.remove(id);
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
    let id = crate::game::MLIST.spawn_actor();
    crate::game::MLIST
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
