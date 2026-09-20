//! Rust-owned storage for game things.
//!
//! The game still uses raw pointers as stable handles because they are stored
//! in map cells and equipment slots. The allocations themselves are owned by
//! this vector, so list operations no longer depend on a C allocator or ABI.

use crate::io::msg_str;
use crate::player::{CThing, CThingMonster};
use std::os::raw::c_int;

static mut THINGS: Vec<Box<CThing>> = Vec::new();
static mut TOTAL: c_int = 0;

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
    if let Some(index) = THINGS.iter().position(|thing| {
        (&**thing as *const CThing).cast_mut() == item
    }) {
        THINGS.swap_remove(index);
        TOTAL -= 1;
    }
}

pub unsafe fn new_item() -> *mut CThing {
    let mut item = Box::new(std::mem::zeroed::<CThing>());
    let pointer = (&mut *item) as *mut CThing;
    THINGS.push(item);
    TOTAL += 1;
    if TOTAL < 0 {
        msg_str("ran out of memory");
        return std::ptr::null_mut();
    }
    pointer
}

pub unsafe fn allocated_count() -> c_int {
    TOTAL
}

pub unsafe fn _detach(list: *mut *mut CThing, item: *mut CThing) {
    detach(list, item);
}

pub unsafe fn _attach(list: *mut *mut CThing, item: *mut CThing) {
    attach(list, item);
}

pub unsafe fn _free_list(list: *mut *mut CThing) {
    free_list(list);
}