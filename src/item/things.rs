//! Object information tables and object naming/inventory helpers.
//!
//! Ported from `src/c/things.c` to Rust.
//!
//! State pointers must remain valid and writable throughout each call, without
//! exclusive references overlapping reentrant UI/daemon access. Metadata slices
//! must include every indexed kind and must not be mutated during the call.
use crate::command::Command;
use crate::direction::Direction;
use crate::dungeon::Dungeon;
use crate::item::armor::waste_time;
use crate::item::pack::{get_item_id, leave_pack_id};
use crate::rnd::Random;
use crate::ui::output::msg_str;

use crate::entity::player::{chg_str, ObjectFlags, Stats};
use crate::game::globals::ObjInfo;
use crate::item::arena::ThingId;
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::rings::RingType;
use crate::item::sticks::fix_stick_in;
use crate::item::weapons::init_weapon_in;

const MAXSTR: usize = 1024;
const NUMTHINGS: usize = 7;
const MAXARMORS: usize = 8;
const MAXPOTIONS: usize = 14;
const MAXRINGS: usize = RingType::COUNT;
const MAXSCROLLS: usize = 18;
const MAXWEAPONS: usize = 9;
const MAXSTICKS: usize = 14;

#[inline]
fn starts_with_article(name: &str) -> &'static str {
    if name.as_bytes().first().is_some_and(|ch| {
        matches!(
            *ch,
            b'a' | b'A' | b'e' | b'E' | b'i' | b'I' | b'o' | b'O' | b'u' | b'U'
        )
    }) {
        "an "
    } else {
        "a "
    }
}

#[inline]
unsafe fn item_name(
    typ: ItemType,
    which: i32,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) -> &'static str {
    match typ {
        ItemType::Potion(_) => pot_info[which as usize].oi_name,
        ItemType::Scroll(_) => scr_info[which as usize].oi_name,
        ItemType::Ring(_) => ring_info[which as usize].oi_name,
        ItemType::Stick(_) => ws_info[which as usize].oi_name,
        ItemType::Weapon(_) => weap_info[which as usize].oi_name,
        ItemType::Armor(_) => arm_info[which as usize].oi_name,
        ItemType::Food => "food",
        ItemType::Gold => "gold",
        ItemType::Amulet => "the Amulet of Yendor",
        _ => "item",
    }
}

unsafe fn copy_to_prbuf(text: &str) -> String {
    text.to_owned()
}

fn adjust_inventory_case(name: &mut String, drop: u8) {
    if name.is_empty() {
        return;
    }

    let first = name.as_bytes()[0];
    if drop != 0 {
        if first.is_ascii_uppercase() {
            name.replace_range(0..1, &(first as char).to_ascii_lowercase().to_string());
        }
    } else if !first.is_ascii_uppercase() {
        name.replace_range(0..1, &(first as char).to_ascii_uppercase().to_string());
    }
}

#[inline]
fn pick_one(info: &[ObjInfo], nitems: usize, random: &mut Random) -> i32 {
    let idx = random.rnd(100);
    let mut i = 0usize;
    while i < nitems {
        if idx < info[i].oi_prob {
            return i as i32;
        }
        i += 1;
    }
    0
}

/// Pointer-free version of [`inv_name`]; works on an arena [`ThingId`].
pub unsafe fn inv_name_id(
    id: ThingId,
    drop: bool,
    dungeon: &Dungeon,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) -> String {
    let Some((which, typ, count, label, group)) = crate::item::arena::with_object(&dungeon.objects, id, |o| {
        (o.o_which, o.o_type, o.o_count, o.o_label.clone(), o.o_group)
    }) else {
        return String::new();
    };

    let mut name = match typ {
        ItemType::Potion(_) => {
            let item = item_name(typ, which, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
            if count == 1 {
                format!("A {item}")
            } else {
                format!("{count} {item}s")
            }
        }
        ItemType::Ring(_) => {
            let item = item_name(typ, which, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
            if count == 1 {
                format!("A {item} ring")
            } else {
                format!("{count} {item} rings")
            }
        }
        ItemType::Stick(_) => {
            let item = item_name(typ, which, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
            if count == 1 {
                format!("A {item}")
            } else {
                format!("{count} {item}s")
            }
        }
        ItemType::Scroll(_) => {
            let item = item_name(typ, which, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
            if count == 1 {
                format!("A scroll of {item}")
            } else {
                format!("{count} scrolls of {item}")
            }
        }
        ItemType::Food => {
            if count == 1 {
                "Some food".to_owned()
            } else {
                format!("{count} rations of food")
            }
        }
        ItemType::Weapon(_) => {
            let item = item_name(typ, which, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
            let mut text = if count > 1 {
                format!("{count} {item}s")
            } else {
                format!("{}{item}", starts_with_article(item))
            };
            if let Some(label) = label.as_ref() {
                text.push_str(" called ");
                text.push_str(label);
            }
            text
        }
        ItemType::Armor(_) => {
            let mut text = item_name(typ, which, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info).to_owned();
            if let Some(label) = label.as_ref() {
                text.push_str(" called ");
                text.push_str(label);
            }
            text
        }
        ItemType::Amulet => "The Amulet of Yendor".to_owned(),
        ItemType::Gold => format!("{group} Gold pieces"),
        _ => "something".to_owned(),
    };

    if inv_describe != 0 {
        let eq = dungeon.player.equipment();
        if Some(id) == eq.armor_id() {
            name.push_str(" (being worn)");
        }
        if Some(id) == eq.weapon_id() {
            name.push_str(" (weapon in hand)");
        }
        if Some(id) == eq.left_ring_id() {
            name.push_str(" (on left hand)");
        } else if Some(id) == eq.right_ring_id() {
            name.push_str(" (on right hand)");
        }
    }

    adjust_inventory_case(&mut name, drop as u8);
    copy_to_prbuf(&name)
}

/// Pointer-free version of `dropcheck`: works on an arena [`ThingId`].
///
/// # Safety
/// `max_stats` must remain valid and writable across reentrant helpers, with no
/// overlapping references to its storage.
pub unsafe fn dropcheck_id(
    id: ThingId,
    random: &mut Random,
    dungeon: &Dungeon,
    max_stats: *mut Stats,
) -> bool {
    let eq = dungeon.player.equipment();
    let is_weapon = eq.weapon_id() == Some(id);
    let is_armor = eq.armor_id() == Some(id);
    let is_left = eq.left_ring_id() == Some(id);
    let is_right = eq.right_ring_id() == Some(id);

    if !is_weapon && !is_armor && !is_left && !is_right {
        return true;
    }
    let cursed = crate::item::arena::with_object(&dungeon.objects, id, |o| o.o_flags.contains(ObjectFlags::CURSED))
        .unwrap_or(false);
    if cursed {
        msg_str("you can't.  It appears to be cursed");
        return false;
    }
    if is_weapon {
        dungeon.player.set_weapon_id(None);
    } else if is_armor {
        waste_time(random);
        dungeon.player.set_armor_id(None);
    } else {
        if is_left {
            dungeon.player.set_left_ring_id(None);
        } else {
            dungeon.player.set_right_ring_id(None);
        }
        let which = crate::item::arena::with_object(&dungeon.objects, id, |o| o.o_which).unwrap_or(0);
        if which == 0 {
            let arm = crate::item::arena::with_object(&dungeon.objects, id, |o| o.o_arm).unwrap_or(0);
            chg_str(-arm, dungeon, max_stats);
        }
    }
    true
}

/// Allocate and initialize a random object, returning its arena handle.
pub unsafe fn new_thing_id(
    random: &mut Random,
    dungeon: &Dungeon,
    things: &[ObjInfo],
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
    a_class: &[i32],
    group: *mut i32,
) -> ThingId {
    new_thing_in(dungeon, random, things, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info, a_class, group)
}

/// Allocate and initialize a random object in the supplied dungeon.
pub(crate) unsafe fn new_thing_in(
    dungeon: &Dungeon,
    random: &mut Random,
    things: &[ObjInfo],
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
    a_class: &[i32],
    group: *mut i32,
) -> ThingId {
    let cur_id = dungeon.objects.new_item_id();

    // Baseline weapon/armor fields, shared by every generated object.
    dungeon.objects.with_object_mut(cur_id, |o| {
        o.o_hplus = 0;
        o.o_dplus = 0;
        o.o_damage = [b'0', b'x', b'0', 0, 0, 0, 0, 0];
        o.o_hurldmg = [b'0', b'x', b'0', 0, 0, 0, 0, 0];
        o.o_arm = 11;
        o.o_count = 1;
        o.o_group = 0;
        o.o_flags = ObjectFlags::NONE;
    });

    let choice = if dungeon.no_food() > 3 {
        2
    } else {
        pick_one(&things[..], NUMTHINGS, random)
    };
    match choice {
        0 => {
            let which = pick_one(&pot_info[..], MAXPOTIONS, random);
            dungeon.objects.with_object_mut(cur_id, |o| {
                o.o_which = which;
                o.o_type = ItemType::potion(which);
            });
        }
        1 => {
            let which = pick_one(&scr_info[..], MAXSCROLLS, random);
            dungeon.objects.with_object_mut(cur_id, |o| {
                o.o_which = which;
                o.o_type = ItemType::scroll(which);
            });
        }
        2 => {
            dungeon.set_no_food(0);
            let which = if random.rnd(10) != 0
            {
                0
            } else {
                1
            };
            dungeon.objects.with_object_mut(cur_id, |o| {
                o.o_type = ItemType::Food;
                o.o_which = which;
            });
        }
        3 => {
            init_weapon_in(
                &dungeon.objects,
                cur_id,
                pick_one(&weap_info[..], MAXWEAPONS, random),
                random,
                group,
            );
            let r = random.rnd(100);
            dungeon.objects.with_object_mut(cur_id, |o| {
                if r < 10 {
                    o.o_flags.insert(ObjectFlags::CURSED);
                    o.o_hplus -= random.rnd(3) + 1;
                } else if r < 15 {
                    o.o_hplus += random.rnd(3) + 1;
                }
            });
        }
        4 => {
            let which = pick_one(&arm_info[..], MAXARMORS, random);
            let r = random.rnd(100);
            dungeon.objects.with_object_mut(cur_id, |o| {
                o.o_which = which;
                o.o_type = ItemType::Armor(which);
                o.o_arm = a_class[which as usize];
                if r < 20 {
                    o.o_flags.insert(ObjectFlags::CURSED);
                    o.o_arm += random.rnd(3) + 1;
                } else if r < 28 {
                    o.o_arm -= random.rnd(3) + 1;
                }
            });
        }
        5 => {
            let ring_type = RingType::from_raw(pick_one(&ring_info[..], MAXRINGS, random))
                .expect("ring metadata produced an invalid ring type");
            dungeon.objects.with_object_mut(cur_id, |o| {
                o.o_which = ring_type as i32;
                o.o_type = ItemType::Ring(ring_type);
                match ring_type {
                    RingType::Protection
                    | RingType::SustainStrength
                    | RingType::AddHit
                    | RingType::AddDamage => {
                        let mut arm = random.rnd(3);
                        if arm == 0 {
                            arm = -1;
                            o.o_flags.insert(ObjectFlags::CURSED);
                        }
                        o.o_arm = arm;
                    }
                    RingType::Adornment | RingType::Aggravate => {
                        o.o_flags.insert(ObjectFlags::CURSED);
                    }
                    _ => {}
                }
            });
        }
        6 => {
            let which = pick_one(&ws_info[..], MAXSTICKS, random);
            dungeon.objects.with_object_mut(cur_id, |o| {
                o.o_which = which;
                o.o_type = ItemType::stick(which);
            });
            fix_stick_in(&dungeon.objects, cur_id, random);
        }
        _ => {}
    }

    cur_id
}

pub unsafe fn drop(
    random: &mut Random,
    dungeon: &Dungeon,
    after: *mut u8,
    again: u8,
    inpack: *mut i32,
    l_last_comm: Command,
    l_last_dir: Direction,
    l_last_pick: Option<ThingId>,
    last_comm: *mut Command,
    last_dir: *mut Direction,
    last_pick: *mut Option<ThingId>,
    mpos: *mut i32,
    msg_esc: *mut u8,
    n_objs: *mut i32,
    pack_used: *mut [u8; 26],
    terse: u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
    max_stats: *mut Stats,
) {
    let Some(id) = get_item_id("drop", ItemFilter::Any, dungeon, after, again, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick, mpos, msg_esc, n_objs, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info) else {
        return;
    };
    if !dropcheck_id(id, random, dungeon, max_stats) {
        return;
    }
    let all = crate::item::arena::with_object(&dungeon.objects, id, |o| o.o_type.drop_whole_stack_by_default())
        .unwrap_or(false);
    let _ = leave_pack_id(id, true, all, dungeon, inpack, last_pick, pack_used);
}

/// Formats a single `%s` substitution from `fmt` and `arg` and hands the
/// result to `msg_str`, mirroring the legacy `add_line` helper.
pub unsafe fn add_line(fmt: &str, arg: &str) -> u8 {
    let text = match fmt.find("%s") {
        Some(idx) => format!("{}{}{}", &fmt[..idx], arg, &fmt[idx + 2..]),
        None => fmt.to_string(),
    };
    msg_str(&text);
    0
}

unsafe fn nothing(_type: u8) -> String {
    copy_to_prbuf("Nothing found")
}

#[allow(dead_code)]
fn pick_one_ex(info: &[ObjInfo], nitems: usize, random: &mut Random) -> i32 {
    pick_one(info, nitems, random)
}

#[allow(dead_code)]
fn set_order(order: &mut [i32], random: &mut Random) {
    let numthings = order.len() as i32;
    for i in 0..numthings {
        order[i as usize] = i;
    }
    for i in (1..=numthings).rev() {
        let r = random.rnd(i);
        order.swap((i - 1) as usize, r as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::player::{Thing, ThingObject};

    #[test]
    fn equipment_checks_expose_the_strength_dependency() {
        let _: unsafe fn(ThingId, &mut Random, &Dungeon, *mut Stats) -> bool = dropcheck_id;
    }

    #[test]
    fn inventory_name_uses_supplied_tables_and_equipment() {
        let dungeon = Dungeon::empty();
        let other_dungeon = Dungeon::empty();
        let weapon = dungeon.objects.insert(Thing::object(ThingObject {
            o_type: ItemType::Weapon(0),
            o_count: 1,
            ..ThingObject::default()
        }));
        dungeon.player.set_weapon_id(Some(weapon));
        let weap_info = [ObjInfo {
            oi_name: "test blade",
            oi_prob: 100,
            oi_worth: 0,
            oi_guess: None,
            oi_know: true,
        }];

        unsafe {
            assert_eq!(
                inv_name_id(weapon, false, &dungeon, 1, &[], &[], &[], &[], &weap_info, &[]),
                "A test blade (weapon in hand)"
            );
            assert_eq!(
                inv_name_id(weapon, true, &dungeon, 0, &[], &[], &[], &[], &weap_info, &[]),
                "a test blade"
            );
            assert_eq!(
                inv_name_id(weapon, false, &other_dungeon, 1, &[], &[], &[], &[], &weap_info, &[]),
                ""
            );
        }
    }
}
