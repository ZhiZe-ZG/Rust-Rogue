use crate::daemon::{do_daemons, do_fuses};
use crate::game::EQUIPMENT;
use crate::io::endmsg;
use crate::io::{addmsg_str, msg_str};
use crate::misc::spread;
use crate::pack::get_item;
use crate::player::{CThing, CThingObject};
use crate::things::{dropcheck, inv_name};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};

const ARMOR: c_int = ']' as c_int;
const ISKNOW: c_int = 0o000002;
const ISPROT: c_int = 0o000040;
const LEFT: usize = 0;
const RIGHT: usize = 1;
const R_SUSTARM: c_int = 13;

unsafe extern "C" {
    static mut terse: c_uchar;
    static mut after: c_uchar;
    static mut to_death: c_uchar;

}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn ring_is(which: usize, ring_type: c_int) -> bool {
    let ring = EQUIPMENT.rings[which];
    !ring.is_null() && (*thing_o(ring)).o_which == ring_type
}

/// Equips selected armor if valid and no armor is already worn.
#[no_mangle]
pub unsafe extern "C" fn wear() {
    let obj = get_item(c"wear".as_ptr(), ARMOR);
    if obj.is_null() {
        return;
    }

    if !EQUIPMENT.armor.is_null() {
        addmsg_str("you are already wearing some");
        if terse == 0 {
            addmsg_str(".  You'll have to take it off first");
        }
        endmsg();
        after = false as c_uchar;
        return;
    }

    if (*thing_o(obj)).o_type != ARMOR {
        msg_str("you can't wear that");
        return;
    }

    waste_time();
    (*thing_o(obj)).o_flags |= ISKNOW;
    let sp = inv_name(obj, true as c_uchar);
    EQUIPMENT.armor = obj;
    if terse == 0 {
        addmsg_str("you are now ");
    }
    msg_str(&format!("wearing {}", CStr::from_ptr(sp).to_string_lossy()));
}

/// Removes currently worn armor after curse/drop checks.
#[no_mangle]
pub unsafe extern "C" fn take_off() {
    let obj = EQUIPMENT.armor;
    if obj.is_null() {
        after = false as c_uchar;
        if terse != 0 {
            msg_str("not wearing armor");
        } else {
            msg_str("you aren't wearing any armor");
        }
        return;
    }

    if dropcheck(EQUIPMENT.armor) == 0 {
        return;
    }

    EQUIPMENT.armor = std::ptr::null_mut();
    if terse != 0 {
        addmsg_str("was");
    } else {
        addmsg_str("you used to be");
    }
    msg_str(&format!(
        " wearing {}) {}",
        (*thing_o(obj)).o_packch as u8 as char,
        CStr::from_ptr(inv_name(obj, true as c_uchar)).to_string_lossy()
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
    if arm.is_null()
        || (*thing_o(arm)).o_type != ARMOR
        || (*thing_o(arm)).o_which == 0
        || (*thing_o(arm)).o_arm >= 9
    {
        return;
    }

    if ((*thing_o(arm)).o_flags & ISPROT) != 0
        || ring_is(LEFT, R_SUSTARM)
        || ring_is(RIGHT, R_SUSTARM)
    {
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
