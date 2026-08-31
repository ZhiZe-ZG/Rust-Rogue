//! Linked-list helpers for `THING` items.
//!
//! Ported from the legacy C module `src/c/list.c` (rogue 5.4.4):
//! `_detach`, `_attach`, `_free_list`, `discard` and `new_item`.
//!
//! The `total` counter (wizard-mode allocation accounting) is exported as a
//! C-visible global so that `state.c` can still save/restore it when the
//! game is compiled with `MASTER` defined.  The Rust port tracks `total`
//! unconditionally; since nothing outside the `MASTER` code paths reads it,
//! gameplay is unaffected when `MASTER` is not defined.

use crate::io::msg_str;
pub use crate::player::{CThing, CThingMonster};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::os::raw::c_int;

/// Total number of `THING` items currently allocated.
///
/// Mirrors `int total` from `list.c` so C code compiled with `MASTER`
/// (e.g. `state.c`) can read and write it directly.
#[no_mangle]
pub static mut total: c_int = 0;

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn next_item(item: *mut CThing) -> *mut CThing {
    (*thing_t(item)).l_next
}

#[inline]
unsafe fn prev_item(item: *mut CThing) -> *mut CThing {
    (*thing_t(item)).l_prev
}

/// Takes an item out of whatever linked list it might be in.
///
/// No globals used directly.
#[no_mangle]
pub unsafe extern "C" fn _detach(list: *mut *mut CThing, item: *mut CThing) {
    if *list == item {
        *list = next_item(item);
    }
    if !prev_item(item).is_null() {
        (*thing_t(prev_item(item))).l_next = next_item(item);
    }
    if !next_item(item).is_null() {
        (*thing_t(next_item(item))).l_prev = prev_item(item);
    }
    (*thing_t(item)).l_next = std::ptr::null_mut();
    (*thing_t(item)).l_prev = std::ptr::null_mut();
}

/// Adds an item to the head of a list.
///
/// No globals used directly.
#[no_mangle]
pub unsafe extern "C" fn _attach(list: *mut *mut CThing, item: *mut CThing) {
    if !(*list).is_null() {
        (*thing_t(item)).l_next = *list;
        (*thing_t(*list)).l_prev = item;
        (*thing_t(item)).l_prev = std::ptr::null_mut();
    } else {
        (*thing_t(item)).l_next = std::ptr::null_mut();
        (*thing_t(item)).l_prev = std::ptr::null_mut();
    }
    *list = item;
}

/// Throws the whole list away.
///
/// Uses globals: total.
#[no_mangle]
pub unsafe extern "C" fn _free_list(ptr: *mut *mut CThing) {
    while !(*ptr).is_null() {
        let item = *ptr;
        *ptr = next_item(item);
        discard(item);
    }
}

/// Frees up an item.
///
/// Uses globals: total.
#[no_mangle]
pub unsafe extern "C" fn discard(item: *mut CThing) {
    total -= 1;
    dealloc(item as *mut u8, Layout::new::<CThing>());
}

/// Gets a new, zeroed item with the next/prev links cleared.
///
/// Uses globals: total.
#[no_mangle]
pub unsafe extern "C" fn new_item() -> *mut CThing {
    let item = alloc_zeroed(Layout::new::<CThing>()) as *mut CThing;
    if item.is_null() {
        msg_str(&format!("ran out of memory after {} items", total));
        return item;
    }
    total += 1;
    (*thing_t(item)).l_next = std::ptr::null_mut();
    (*thing_t(item)).l_prev = std::ptr::null_mut();
    item
}
