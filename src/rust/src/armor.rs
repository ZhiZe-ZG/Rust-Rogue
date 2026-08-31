use crate::io::{addmsg_str, msg_str};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};

const ARMOR: c_int = ']' as c_int;
const ISKNOW: c_int = 0o000002;
const ISPROT: c_int = 0o000040;
const LEFT: usize = 0;
const RIGHT: usize = 1;
const R_SUSTARM: c_int = 13;
const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CCoord {
    pub x: c_int,
    pub y: c_int,
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

#[repr(C)]
pub union CThing {
    pub o: CThingObject,
}

unsafe extern "C" {
    static mut terse: c_uchar;
    static mut after: c_uchar;
    static mut cur_armor: *mut CThing;
    static mut cur_ring: [*mut CThing; 2];
    static mut to_death: c_uchar;

    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn endmsg() -> c_int;
    fn dropcheck(obj: *mut CThing) -> c_uchar;
    fn inv_name(obj: *mut CThing, drop: c_uchar) -> *mut c_char;
    fn do_daemons(flag: c_int);
    fn do_fuses(flag: c_int);
    fn spread(nm: c_int) -> c_int;
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn ring_is(which: usize, ring_type: c_int) -> bool {
    let ring = cur_ring[which];
    !ring.is_null() && (*thing_o(ring)).o_which == ring_type
}

/// Equips selected armor if valid and no armor is already worn.
#[no_mangle]
pub unsafe extern "C" fn wear() {
    let obj = get_item(c"wear".as_ptr(), ARMOR);
    if obj.is_null() {
        return;
    }

    if !cur_armor.is_null() {
        addmsg_str("you are already wearing some");
        if terse == 0 {
            addmsg_str(".  You'll have to take it off first");
        }
        endmsg();
        after = FALSE;
        return;
    }

    if (*thing_o(obj)).o_type != ARMOR {
        msg_str("you can't wear that");
        return;
    }

    waste_time();
    (*thing_o(obj)).o_flags |= ISKNOW;
    let sp = inv_name(obj, TRUE);
    cur_armor = obj;
    if terse == 0 {
        addmsg_str("you are now ");
    }
    msg_str(&format!("wearing {}", CStr::from_ptr(sp).to_string_lossy()));
}

/// Removes currently worn armor after curse/drop checks.
#[no_mangle]
pub unsafe extern "C" fn take_off() {
    let obj = cur_armor;
    if obj.is_null() {
        after = FALSE;
        if terse != 0 {
            msg_str("not wearing armor");
        } else {
            msg_str("you aren't wearing any armor");
        }
        return;
    }

    if dropcheck(cur_armor) == 0 {
        return;
    }

    cur_armor = std::ptr::null_mut();
    if terse != 0 {
        addmsg_str("was");
    } else {
        addmsg_str("you used to be");
    }
    msg_str(&format!(
        " wearing {}) {}",
        (*thing_o(obj)).o_packch as u8 as char,
        CStr::from_ptr(inv_name(obj, TRUE)).to_string_lossy()
    ));
}

/// Advances daemon and fuse queues as a deliberate no-op turn.
#[no_mangle]
pub unsafe extern "C" fn waste_time() {
    do_daemons(spread(1));
    do_fuses(spread(1));
    do_daemons(spread(2));
    do_fuses(spread(2));
}

/// rust_armor:
/// Rust the given armor if it is a legal kind to rust.
#[no_mangle]
pub unsafe extern "C" fn rust_armor(arm: *mut CThing) {
    if arm.is_null() || (*thing_o(arm)).o_type != ARMOR || (*thing_o(arm)).o_which == 0 || (*thing_o(arm)).o_arm >= 9 {
        return;
    }

    if ((*thing_o(arm)).o_flags & ISPROT) != 0 || ring_is(LEFT, R_SUSTARM) || ring_is(RIGHT, R_SUSTARM) {
        if to_death == 0 {
            msg_str("the rust vanishes instantly");
        }
    } else {
        (*thing_o(arm)).o_arm += 1;
        if terse == 0 {
            msg_str("your armor appears to be weaker now. Oh my!");
        } else {
            msg_str("your armor weakens");
        }
    }
}
