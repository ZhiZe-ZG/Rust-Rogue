//! Rust-owned storage for game things.
//!
//! The game still uses raw pointers as stable handles because they are stored
//! in map cells and equipment slots. The allocations themselves are owned by
//! this vector, so list operations no longer depend on a C allocator or ABI.

use crate::player::{CThing, CThingMonster};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

struct OwnedThing(Box<CThing>);

// The game accesses thing pointers on its single gameplay thread; the mutex
// only protects the arena's ownership when the global is initialized.
unsafe impl Send for OwnedThing {}

static THINGS: OnceLock<Mutex<Vec<OwnedThing>>> = OnceLock::new();
static TOTAL: AtomicI32 = AtomicI32::new(0);

fn things() -> &'static Mutex<Vec<OwnedThing>> {
    THINGS.get_or_init(|| Mutex::new(Vec::new()))
}

#[inline]
unsafe fn thing_t(item: *mut CThing) -> *mut CThingMonster {
    item as *mut CThingMonster
}

pub unsafe fn detach(list: *mut *mut CThing, item: *mut CThing) {
    if *list == item {
        *list = (*thing_t(item)).l_next;
    }
    if !(*thing_t(item)).l_prev.is_null() {
        (*thing_t((*thing_t(item)).l_prev)).l_next = (*thing_t(item)).l_next;
    }
    if !(*thing_t(item)).l_next.is_null() {
        (*thing_t((*thing_t(item)).l_next)).l_prev = (*thing_t(item)).l_prev;
    }
    (*thing_t(item)).l_next = std::ptr::null_mut();
    (*thing_t(item)).l_prev = std::ptr::null_mut();
}

pub unsafe fn attach(list: *mut *mut CThing, item: *mut CThing) {
    (*thing_t(item)).l_next = *list;
    (*thing_t(item)).l_prev = std::ptr::null_mut();
    if !(*list).is_null() {
        (*thing_t(*list)).l_prev = item;
    }
    *list = item;
}

pub unsafe fn free_list(list: *mut *mut CThing) {
    while !(*list).is_null() {
        let item = *list;
        *list = (*thing_t(item)).l_next;
        discard(item);
    }
}

pub unsafe fn discard(item: *mut CThing) {
    let mut things = things().lock().expect("thing store poisoned");
    if let Some(index) = things
        .iter()
        .position(|thing| (&*thing.0 as *const CThing).cast_mut() == item)
    {
        things.swap_remove(index);
        TOTAL.fetch_sub(1, Ordering::Relaxed);
    }
}

pub unsafe fn new_item() -> *mut CThing {
    let mut item = OwnedThing(Box::new(std::mem::zeroed::<CThing>()));
    let pointer = (&mut *item.0) as *mut CThing;
    things().lock().expect("thing store poisoned").push(item);
    TOTAL.fetch_add(1, Ordering::Relaxed);
    pointer
}

pub fn allocated_count() -> i32 {
    TOTAL.load(Ordering::Relaxed)
}
