//! Armor: wearing, removing, and protection.
//!
//! Ported from `src/c/armor.c` to Rust.
use crate::command::Command;
use crate::daemon::{do_daemons, do_fuses};
use crate::direction::Direction;
use crate::dungeon::Dungeon;
use crate::entity::player::ObjectFlags;
use crate::game::globals::ObjInfo;
use crate::item::arena::ThingId;
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::get_item_id;
use crate::item::rings::RingType;
use crate::item::things::{dropcheck_id, inv_name_id};
use crate::misc::spread;
use crate::rnd::Random;
use crate::ui::output::{addmsg_str, endmsg, msg_str};

/// Equips selected armor if valid and no armor is already worn.
///
/// # Safety
/// `after` must remain writable for the call; no exclusive reference to its
/// storage may be held across callbacks that can access the same flag.
pub unsafe fn wear(
    random: &mut Random,
    dungeon: &Dungeon,
    terse: u8,
    after: *mut u8,
    again: u8,
    l_last_comm: Command,
    l_last_dir: Direction,
    l_last_pick: Option<ThingId>,
    last_comm: *mut Command,
    last_dir: *mut Direction,
    last_pick: *mut Option<ThingId>,
    mpos: *mut i32,
    msg_esc: *mut u8,
    n_objs: *mut i32,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) {
    let Some(obj) = get_item_id("wear", ItemFilter::Category(ItemType::ARMOR), dungeon, after, again, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick, mpos, msg_esc, n_objs, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info) else {
        return;
    };

    if dungeon.player.equipment().armor_id().is_some() {
        addmsg_str("you are already wearing some");
        if terse == 0 {
            addmsg_str(".  You'll have to take it off first");
        }
        endmsg();
        *after = false as u8;
        return;
    }

    if !matches!(
        dungeon.objects.with_object(obj, |o| o.o_type),
        Some(ItemType::Armor(_))
    ) {
        msg_str("you can't wear that");
        return;
    }

    waste_time(random);
    dungeon.objects.with_object_mut(obj, |o| o.o_flags.insert(ObjectFlags::KNOW));
    let sp = inv_name_id(obj, true, dungeon, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
    dungeon.player.set_armor_id(Some(obj));
    if terse == 0 {
        addmsg_str("you are now ");
    }
    msg_str(&format!("wearing {}", sp));
}

/// Removes currently worn armor after curse/drop checks.
///
/// # Safety
/// `after` must remain writable for the call without an overlapping exclusive
/// reference across reentrant item/UI calls.
pub unsafe fn take_off(
    random: &mut Random,
    dungeon: &Dungeon,
    terse: u8,
    after: *mut u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) {
    let Some(obj) = dungeon.player.equipment().armor_id() else {
        *after = false as u8;
        if terse != 0 {
            msg_str("not wearing armor");
        } else {
            msg_str("you aren't wearing any armor");
        }
        return;
    };

    if !dropcheck_id(obj, random, dungeon) {
        return;
    }

    dungeon.player.set_armor_id(None);
    if terse != 0 {
        addmsg_str("was");
    } else {
        addmsg_str("you used to be");
    }
    let packch = dungeon.objects.with_object(obj, |o| o.o_packch).unwrap_or(0);
    msg_str(&format!(
        " wearing {}) {}",
        packch as char,
        inv_name_id(obj, true, dungeon, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info)
    ));
}

/// Advances daemon and fuse queues as a deliberate no-op turn.
pub unsafe fn waste_time(random: &mut Random) {
    do_daemons(spread(1, random), random);
    do_fuses(spread(1, random), random);
    do_daemons(spread(2, random), random);
    do_fuses(spread(2, random), random);
}

/// Rust the given armor if it is a legal kind to rust.
pub unsafe fn rust_armor_id(arm: Option<ThingId>, dungeon: &Dungeon, to_death: u8, terse: u8) {
    let Some(id) = arm else {
        return;
    };
    let Some((typ, which, o_arm, prot)) = dungeon.objects.with_object(id, |o| {
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

    if prot || dungeon.player.wearing_ring(RingType::SustainArmor) {
        if to_death == 0 {
            msg_str("the rust vanishes instantly");
        }
    } else {
        dungeon.objects.with_object_mut(id, |o| o.o_arm += 1);
        if terse == 0 {
            msg_str("your armor appears to be weaker now. Oh my!");
        } else {
            msg_str("your armor weakens");
        }
    }
}
