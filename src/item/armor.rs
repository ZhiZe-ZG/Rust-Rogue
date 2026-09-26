//! Armor: wearing, removing, and protection.
//!
//! Ported from `src/c/armor.c` to Rust.
use crate::daemon::{do_daemons, do_fuses};
use crate::entity::player::{ObjectFlags, Thing, ThingObject};
use crate::game::PLAYER;
use crate::item::pack::get_item;
use crate::item::rings::RingType;
use crate::item::things::{dropcheck, inv_name};
use crate::misc::spread;
use crate::ui::output::{addmsg_str, endmsg, msg_str};
use std::ffi::CStr;
use std::os::raw::{c_int, c_uchar};

use crate::globals::{after, terse, to_death};

const ARMOR: c_int = ']' as c_int;

#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

#[inline]
unsafe fn ring_is(ring: *mut Thing, ring_type: RingType) -> bool {
    !ring.is_null() && RingType::from_raw((*thing_o(ring)).o_which) == Some(ring_type)
}

/// Equips selected armor if valid and no armor is already worn.
pub unsafe fn wear() {
    let obj = get_item("wear", ARMOR);
    if obj.is_null() {
        return;
    }

    if !PLAYER.armor().is_null() {
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
    (*thing_o(obj)).o_flags.insert(ObjectFlags::KNOW);
    let sp = inv_name(obj, true as c_uchar);
    PLAYER.set_armor(obj);
    if terse == 0 {
        addmsg_str("you are now ");
    }
    msg_str(&format!("wearing {}", sp));
}

/// Removes currently worn armor after curse/drop checks.
pub unsafe fn take_off() {
    let obj = PLAYER.armor();
    if obj.is_null() {
        after = false as c_uchar;
        if terse != 0 {
            msg_str("not wearing armor");
        } else {
            msg_str("you aren't wearing any armor");
        }
        return;
    }

    if dropcheck(PLAYER.armor()) == 0 {
        return;
    }

    PLAYER.set_armor(std::ptr::null_mut());
    if terse != 0 {
        addmsg_str("was");
    } else {
        addmsg_str("you used to be");
    }
    msg_str(&format!(
        " wearing {}) {}",
        (*thing_o(obj)).o_packch as char,
        inv_name(obj, true as c_uchar)
    ));
}

/// Advances daemon and fuse queues as a deliberate no-op turn.
pub unsafe fn waste_time() {
    do_daemons(spread(1));
    do_fuses(spread(1));
    do_daemons(spread(2));
    do_fuses(spread(2));
}

/// rust_armor:
/// Rust the given armor if it is a legal kind to rust.
pub unsafe fn rust_armor(arm: *mut Thing) {
    if arm.is_null()
        || (*thing_o(arm)).o_type != ARMOR
        || (*thing_o(arm)).o_which == 0
        || (*thing_o(arm)).o_arm >= 9
    {
        return;
    }

    if (*thing_o(arm)).o_flags.contains(ObjectFlags::PROT)
        || ring_is(PLAYER.left_ring(), RingType::SustainArmor)
        || ring_is(PLAYER.right_ring(), RingType::SustainArmor)
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
