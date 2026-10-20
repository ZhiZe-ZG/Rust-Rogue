//! Armor: wearing, removing, and protection.
//!
//! Ported from `src/c/armor.c` to Rust.
use crate::daemon::{do_daemons, do_fuses};
use crate::entity::player::ObjectFlags;
use crate::game::PLAYER;
use crate::item::arena::{ThingId, OBJECTS};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::get_item_id;
use crate::item::rings::RingType;
use crate::item::things::{dropcheck_id, inv_name_id};
use crate::misc::spread;
use crate::ui::output::{addmsg_str, endmsg, msg_str};

use crate::game::globals::{after, terse, to_death};

const ARMOR: i32 = ']' as i32;

/// Equips selected armor if valid and no armor is already worn.
pub unsafe fn wear() {
    let Some(obj) = get_item_id("wear", ItemFilter::Category(ItemType::ARMOR)) else {
        return;
    };

    if PLAYER.equipment().armor_id().is_some() {
        addmsg_str("you are already wearing some");
        if terse == 0 {
            addmsg_str(".  You'll have to take it off first");
        }
        endmsg();
        after = false as u8;
        return;
    }

    if !matches!(
        OBJECTS.with_object(obj, |o| o.o_type),
        Some(ItemType::Armor(_))
    ) {
        msg_str("you can't wear that");
        return;
    }

    waste_time();
    OBJECTS.with_object_mut(obj, |o| o.o_flags.insert(ObjectFlags::KNOW));
    let sp = inv_name_id(obj, true);
    PLAYER.set_armor_id(Some(obj));
    if terse == 0 {
        addmsg_str("you are now ");
    }
    msg_str(&format!("wearing {}", sp));
}

/// Removes currently worn armor after curse/drop checks.
pub unsafe fn take_off() {
    let Some(obj) = PLAYER.equipment().armor_id() else {
        after = false as u8;
        if terse != 0 {
            msg_str("not wearing armor");
        } else {
            msg_str("you aren't wearing any armor");
        }
        return;
    };

    if !dropcheck_id(obj) {
        return;
    }

    PLAYER.set_armor_id(None);
    if terse != 0 {
        addmsg_str("was");
    } else {
        addmsg_str("you used to be");
    }
    let packch = OBJECTS.with_object(obj, |o| o.o_packch).unwrap_or(0);
    msg_str(&format!(
        " wearing {}) {}",
        packch as char,
        inv_name_id(obj, true)
    ));
}

/// Advances daemon and fuse queues as a deliberate no-op turn.
pub unsafe fn waste_time() {
    do_daemons(spread(1));
    do_fuses(spread(1));
    do_daemons(spread(2));
    do_fuses(spread(2));
}

/// Rust the given armor if it is a legal kind to rust.
pub unsafe fn rust_armor_id(arm: Option<ThingId>) {
    let Some(id) = arm else {
        return;
    };
    let Some((typ, which, o_arm, prot)) = OBJECTS.with_object(id, |o| {
        (
            o.o_type,
            o.o_which,
            o.o_arm,
            o.o_flags.contains(ObjectFlags::PROT),
        )
    }) else {
        return;
    };
    if !matches!(typ, ItemType::Armor(_)) || which == 0 || o_arm >= 9 {
        return;
    }

    if prot || PLAYER.wearing_ring(RingType::SustainArmor) {
        if to_death == 0 {
            msg_str("the rust vanishes instantly");
        }
    } else {
        OBJECTS.with_object_mut(id, |o| o.o_arm += 1);
        if terse == 0 {
            msg_str("your armor appears to be weaker now. Oh my!");
        } else {
            msg_str("your armor weakens");
        }
    }
}
