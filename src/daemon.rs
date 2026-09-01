/*
 * Contains functions for dealing with things that happen in the future
 * (daemons and fuses).
 *
 * Ported from daemon.c to Rust.
 *
 * Rogue: Exploring the Dungeons of Doom
 * Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
 * All rights reserved.
 *
 * See the file LICENSE.TXT for full copyright and licensing information.
 */

use std::os::raw::{c_int, c_void};

const EMPTY: c_int = 0;
const DAEMON: c_int = -1;
const MAXDAEMONS: usize = 20;

/// Function pointer type stored in the delayed-action table.
/// `Option<fn>` with `#[repr(transparent)]` semantics: None is null,
/// which is safe to share with C as a nullable function pointer.
type DFunc = Option<unsafe extern "C" fn(c_int)>;

/// Transmute a raw C function pointer (passed as void*) into our DFunc type.
/// The void* convention is used on both the Rust caller side (potions.rs,
/// misc.rs, etc.) and the C caller side to avoid strict-aliasing issues
/// with mismatched fn-pointer types.
#[inline]
unsafe fn as_dfunc(func: *const c_void) -> DFunc {
    std::mem::transmute::<*const c_void, DFunc>(func)
}

/// The delayed-action table.  Layout must match the C struct declaration in
/// rogue.h:
///   struct delayed_action { int d_type; void (*d_func)(int); int d_arg; int d_time; }
/// repr(C) + field ordering guarantees this.
#[repr(C)]
pub struct CDelayedAction {
    pub d_type: c_int,
    pub d_func: DFunc,
    pub d_arg:  c_int,
    pub d_time: c_int,
}

const EMPTY_SLOT: CDelayedAction = CDelayedAction {
    d_type: EMPTY,
    d_func: None,
    d_arg:  0,
    d_time: 0,
};

/// Global daemon/fuse table.  Exported as `d_list` so C code that
/// declares `extern struct delayed_action d_list[]` resolves against us.
#[no_mangle]
pub static mut d_list: [CDelayedAction; MAXDAEMONS] = [
    EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT,
    EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT,
    EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT,
    EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT, EMPTY_SLOT,
];

/// Find an empty slot in the daemon/fuse list.
/// Returns a raw pointer to the slot, or null if none is available.
unsafe fn d_slot() -> *mut CDelayedAction {
    for i in 0..MAXDAEMONS {
        if d_list[i].d_type == EMPTY {
            return &raw mut d_list[i];
        }
    }
    std::ptr::null_mut()
}

/// Find the slot whose d_func matches `func`.
/// Returns a raw pointer to the slot, or null if not found.
unsafe fn find_slot(func: *const c_void) -> *mut CDelayedAction {
    let target: DFunc = as_dfunc(func);
    for i in 0..MAXDAEMONS {
        if d_list[i].d_type != EMPTY && d_list[i].d_func == target {
            return &raw mut d_list[i];
        }
    }
    std::ptr::null_mut()
}

/// Start a daemon: inserts `func` into the daemon/fuse table as a daemon
/// (d_time == DAEMON, i.e. runs every turn).
#[no_mangle]
pub unsafe extern "C" fn start_daemon(func: *const c_void, arg: c_int, typ: c_int) {
    let dev = d_slot();
    if dev.is_null() {
        return;
    }
    (*dev).d_type = typ;
    (*dev).d_func = as_dfunc(func);
    (*dev).d_arg  = arg;
    (*dev).d_time = DAEMON;
}

/// Remove a daemon/fuse from the table by function pointer.
#[no_mangle]
pub unsafe extern "C" fn kill_daemon(func: *const c_void) {
    let dev = find_slot(func);
    if dev.is_null() {
        return;
    }
    (*dev).d_type = EMPTY;
}

/// Run all active daemons whose d_type matches `flag`.
/// Daemons are entries with d_time == DAEMON.
#[no_mangle]
pub unsafe extern "C" fn do_daemons(flag: c_int) {
    for i in 0..MAXDAEMONS {
        if d_list[i].d_type == flag && d_list[i].d_time == DAEMON {
            // Capture func and arg before the call, which may modify d_list.
            let f   = d_list[i].d_func;
            let arg = d_list[i].d_arg;
            if let Some(f) = f {
                f(arg);
            }
        }
    }
}

/// Light a fuse: inserts `func` with a countdown of `time` turns.
#[no_mangle]
pub unsafe extern "C" fn fuse(func: *const c_void, arg: c_int, time: c_int, typ: c_int) {
    let wire = d_slot();
    if wire.is_null() {
        return;
    }
    (*wire).d_type = typ;
    (*wire).d_func = as_dfunc(func);
    (*wire).d_arg  = arg;
    (*wire).d_time = time;
}

/// Extend the countdown of an existing fuse by `xtime` turns.
#[no_mangle]
pub unsafe extern "C" fn lengthen(func: *const c_void, xtime: c_int) {
    let wire = find_slot(func);
    if wire.is_null() {
        return;
    }
    (*wire).d_time += xtime;
}

/// Extinguish (cancel) a fuse or daemon by function pointer.
#[no_mangle]
pub unsafe extern "C" fn extinguish(func: *const c_void) {
    let wire = find_slot(func);
    if wire.is_null() {
        return;
    }
    (*wire).d_type = EMPTY;
}

/// Decrement all active fuses whose d_type matches `flag`, and fire any
/// that reach zero.
#[no_mangle]
pub unsafe extern "C" fn do_fuses(flag: c_int) {
    for i in 0..MAXDAEMONS {
        if d_list[i].d_type == flag && d_list[i].d_time > 0 {
            d_list[i].d_time -= 1;
            if d_list[i].d_time == 0 {
                d_list[i].d_type = EMPTY;
                // Capture func and arg before the call.
                let f   = d_list[i].d_func;
                let arg = d_list[i].d_arg;
                if let Some(f) = f {
                    f(arg);
                }
            }
        }
    }
}
