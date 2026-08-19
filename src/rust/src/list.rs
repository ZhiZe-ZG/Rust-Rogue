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

use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

/// Total number of `THING` items currently allocated.
///
/// Mirrors `int total` from `list.c` so C code compiled with `MASTER`
/// (e.g. `state.c`) can read and write it directly.
#[no_mangle]
pub static mut total: c_int = 0;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CCoord {
    pub x: c_int,
    pub y: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CStats {
    pub s_str: c_uint,
    pub s_exp: c_int,
    pub s_lvl: c_int,
    pub s_arm: c_int,
    pub s_hpt: c_int,
    pub s_dmg: [c_char; 13],
    pub s_maxhp: c_int,
}

#[repr(C)]
pub struct CRoom {
    pub r_pos: CCoord,
    pub r_max: CCoord,
    pub r_gold: CCoord,
    pub r_goldval: c_int,
    pub r_flags: c_short,
    pub r_nexits: c_int,
    pub r_exit: [CCoord; 12],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CThingMonster {
    pub l_next: *mut CThing,
    pub l_prev: *mut CThing,
    pub t_pos: CCoord,
    pub t_turn: c_uchar,
    pub t_type: c_char,
    pub t_disguise: c_char,
    pub t_oldch: c_char,
    pub t_dest: *mut CCoord,
    pub t_flags: c_short,
    pub t_stats: CStats,
    pub t_room: *mut CRoom,
    pub t_pack: *mut CThing,
    pub t_reserved: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CThingObject {
    pub l_next: *mut CThing,
    pub l_prev: *mut CThing,
    pub o_type: c_int,
    pub o_pos: CCoord,
    pub o_text: *mut c_char,
    pub o_launch: c_int,
    pub o_packch: c_char,
    pub o_damage: [c_char; 8],
    pub o_hurldmg: [c_char; 8],
    pub o_count: c_int,
    pub o_which: c_int,
    pub o_hplus: c_int,
    pub o_dplus: c_int,
    pub o_arm: c_int,
    pub o_flags: c_int,
    pub o_group: c_int,
    pub o_label: *mut c_char,
}

/// A `THING` from `rogue.h`: a monster or an object.
#[repr(C)]
pub union CThing {
    pub t: CThingMonster,
    pub o: CThingObject,
}

unsafe extern "C" {
    fn msg(fmt: *const c_char, ...);
}

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
        msg(c"ran out of memory after %d items".as_ptr(), total);
        return item;
    }
    total += 1;
    (*thing_t(item)).l_next = std::ptr::null_mut();
    (*thing_t(item)).l_prev = std::ptr::null_mut();
    item
}
