//! Object information tables and object naming/inventory helpers.
//!
//! Ported from `src/c/things.c` to Rust.
use crate::game::PLAYER;
use crate::item::armor::waste_time;
use crate::item::pack::{get_item, leave_pack};
use crate::misc::chg_str;
use crate::rnd::rnd;
use crate::ui::output::msg_str;

use crate::entity::player::{ObjectFlags, Thing, ThingObject};
use crate::game::globals::{
    arm_info, pot_info, ring_info, scr_info, things, weap_info, ws_info, ObjInfo,
};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::rings::RingType;
use crate::item::sticks::fix_stick;
use crate::item::arena::new_item;
use crate::item::weapons::init_weapon;

const MAXSTR: usize = 1024;
const NUMTHINGS: usize = 7;
const MAXARMORS: usize = 8;
const MAXPOTIONS: usize = 14;
const MAXRINGS: usize = RingType::COUNT;
const MAXSCROLLS: usize = 18;
const MAXWEAPONS: usize = 9;
const MAXSTICKS: usize = 14;

use crate::game::globals::{a_class, inv_describe, no_food};


#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

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
unsafe fn item_name(typ: ItemType, which: i32) -> &'static str {
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
fn pick_one(info: &[ObjInfo], nitems: usize) -> i32 {
    let idx = rnd(100);
    let mut i = 0usize;
    while i < nitems {
        if idx < info[i].oi_prob {
            return i as i32;
        }
        i += 1;
    }
    0
}

pub unsafe fn inv_name(obj: *mut Thing, drop: u8) -> String {
    if obj.is_null() {
        return String::new();
    }

    let which = (*thing_o(obj)).o_which;
    let typ = (*thing_o(obj)).o_type;
    let count = (*thing_o(obj)).o_count;
    let mut name = match typ {
        ItemType::Potion(_) => {
            let item = item_name(typ, which);
            if count == 1 {
                format!("A {item}")
            } else {
                format!("{count} {item}s")
            }
        }
        ItemType::Ring(_) => {
            let item = item_name(typ, which);
            if count == 1 {
                format!("A {item} ring")
            } else {
                format!("{count} {item} rings")
            }
        }
        ItemType::Stick(_) => {
            let item = item_name(typ, which);
            if count == 1 {
                format!("A {item}")
            } else {
                format!("{count} {item}s")
            }
        }
        ItemType::Scroll(_) => {
            let item = item_name(typ, which);
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
            let item = item_name(typ, which);
            let mut text = if count > 1 {
                format!("{count} {item}s")
            } else {
                format!("{}{item}", starts_with_article(item))
            };
            if let Some(label) = (*thing_o(obj)).o_label.as_ref() {
                text.push_str(" called ");
                text.push_str(label);
            }
            text
        }
        ItemType::Armor(_) => {
            let mut text = item_name(typ, which).to_owned();
            if let Some(label) = (*thing_o(obj)).o_label.as_ref() {
                text.push_str(" called ");
                text.push_str(label);
            }
            text
        }
        ItemType::Amulet => "The Amulet of Yendor".to_owned(),
        ItemType::Gold => format!("{} Gold pieces", (*thing_o(obj)).o_group),
        _ => "something".to_owned(),
    };

    if inv_describe != 0 {
        // Identify the object by arena handle rather than by raw pointer.
        let id = crate::item::arena::id_of(obj);
        let eq = PLAYER.equipment();
        if id.is_some() && id == eq.armor_id() {
            name.push_str(" (being worn)");
        }
        if id.is_some() && id == eq.weapon_id() {
            name.push_str(" (weapon in hand)");
        }
        if id.is_some() && id == eq.left_ring_id() {
            name.push_str(" (on left hand)");
        } else if id.is_some() && id == eq.right_ring_id() {
            name.push_str(" (on right hand)");
        }
    }

    adjust_inventory_case(&mut name, drop);
    copy_to_prbuf(&name)
}

pub unsafe fn dropcheck(obj: *mut Thing) -> u8 {
    let id = match crate::item::arena::id_of(obj) {
        Some(id) => id,
        None => return true as u8,
    };
    let eq = PLAYER.equipment();
    let is_weapon = eq.weapon_id() == Some(id);
    let is_armor = eq.armor_id() == Some(id);
    let is_left = eq.left_ring_id() == Some(id);
    let is_right = eq.right_ring_id() == Some(id);

    if !is_weapon && !is_armor && !is_left && !is_right {
        return true as u8;
    }
    if (*thing_o(obj)).o_flags.contains(ObjectFlags::CURSED) {
        msg_str("you can't.  It appears to be cursed");
        return false as u8;
    }
    if is_weapon {
        PLAYER.set_weapon_id(None);
    } else if is_armor {
        waste_time();
        PLAYER.set_armor_id(None);
    } else {
        if is_left {
            PLAYER.set_left_ring_id(None);
        } else {
            PLAYER.set_right_ring_id(None);
        }
        match (*thing_o(obj)).o_which {
            0 => chg_str(-(*thing_o(obj)).o_arm),
            _ => {}
        }
    }
    true as u8
}

pub unsafe fn new_thing() -> *mut Thing {
    let cur = new_item();
    (*thing_o(cur)).o_hplus = 0;
    (*thing_o(cur)).o_dplus = 0;
    std::ptr::copy_nonoverlapping(
        b"0x0\0".as_ptr(),
        (*thing_o(cur)).o_damage.as_mut_ptr(),
        4,
    );
    std::ptr::copy_nonoverlapping(
        b"0x0\0".as_ptr(),
        (*thing_o(cur)).o_hurldmg.as_mut_ptr(),
        4,
    );
    (*thing_o(cur)).o_arm = 11;
    (*thing_o(cur)).o_count = 1;
    (*thing_o(cur)).o_group = 0;
    (*thing_o(cur)).o_flags = ObjectFlags::NONE;

    let choice = if no_food > 3 {
        2
    } else {
        pick_one(&things[..], NUMTHINGS)
    };
    match choice {
        0 => {
            let which = pick_one(&pot_info[..], MAXPOTIONS);
            (*thing_o(cur)).o_which = which;
            (*thing_o(cur)).o_type = ItemType::potion(which);
        }
        1 => {
            let which = pick_one(&scr_info[..], MAXSCROLLS);
            (*thing_o(cur)).o_which = which;
            (*thing_o(cur)).o_type = ItemType::scroll(which);
        }
        2 => {
            (*thing_o(cur)).o_type = ItemType::Food;
            no_food = 0;
            if rnd(10) != 0 {
                (*thing_o(cur)).o_which = 0;
            } else {
                (*thing_o(cur)).o_which = 1;
            }
        }
        3 => {
            init_weapon(cur, pick_one(&weap_info[..], MAXWEAPONS));
            let r = rnd(100);
            if r < 10 {
                (*thing_o(cur)).o_flags.insert(ObjectFlags::CURSED);
                (*thing_o(cur)).o_hplus -= rnd(3) + 1;
            } else if r < 15 {
                (*thing_o(cur)).o_hplus += rnd(3) + 1;
            }
        }
        4 => {
            let which = pick_one(&arm_info[..], MAXARMORS);
            (*thing_o(cur)).o_which = which;
            (*thing_o(cur)).o_type = ItemType::Armor(which);
            (*thing_o(cur)).o_arm = a_class[(*thing_o(cur)).o_which as usize];
            let r = rnd(100);
            if r < 20 {
                (*thing_o(cur)).o_flags.insert(ObjectFlags::CURSED);
                (*thing_o(cur)).o_arm += rnd(3) + 1;
            } else if r < 28 {
                (*thing_o(cur)).o_arm -= rnd(3) + 1;
            }
        }
        5 => {
            let ring_type = RingType::from_raw(pick_one(&ring_info[..], MAXRINGS))
                .expect("ring metadata produced an invalid ring type");
            (*thing_o(cur)).o_which = ring_type as i32;
            (*thing_o(cur)).o_type = ItemType::Ring(ring_type);
            match ring_type {
                RingType::Protection
                | RingType::SustainStrength
                | RingType::AddHit
                | RingType::AddDamage => {
                    let mut arm = rnd(3);
                    if arm == 0 {
                        arm = -1;
                        (*thing_o(cur)).o_flags.insert(ObjectFlags::CURSED);
                    }
                    (*thing_o(cur)).o_arm = arm;
                }
                RingType::Adornment | RingType::Aggravate => {
                    (*thing_o(cur)).o_flags.insert(ObjectFlags::CURSED);
                }
                _ => {}
            }
        }
        6 => {
            let which = pick_one(&ws_info[..], MAXSTICKS);
            (*thing_o(cur)).o_which = which;
            (*thing_o(cur)).o_type = ItemType::stick(which);
            fix_stick(cur);
        }
        _ => {}
    }

    cur
}

pub unsafe fn drop() {
    let obj = get_item("drop", ItemFilter::Any);
    if obj.is_null() {
        return;
    }
    if dropcheck(obj) == 0 {
        return;
    }
    let all = if (*thing_o(obj)).o_type.drop_whole_stack_by_default() {
        true as u8
    } else {
        false as u8
    };
    let _ = leave_pack(obj, true as u8, all);
}

pub unsafe fn discovered() {}

unsafe fn print_disc(_type: u8) {}

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

unsafe fn end_line() {}

unsafe fn nothing(_type: u8) -> String {
    copy_to_prbuf("Nothing found")
}

pub unsafe fn nameit(
    obj: *mut Thing,
    typ: &str,
    which: &str,
    op: &ObjInfo,
    prfunc: unsafe fn(*mut Thing) -> String,
) {
    if obj.is_null() {
        return;
    }
    let typ = typ;
    let which = which;
    let pr_text = prfunc(obj);
    let count = (*thing_o(obj)).o_count;

    let text = if op.oi_know || op.oi_guess.is_some() {
        let prefix = if count == 1 {
            format!("A {typ} ")
        } else {
            format!("{count} {typ}s ")
        };
        if op.oi_know {
            format!("{prefix}of {}{}({which})", op.oi_name, pr_text)
        } else if let Some(guess) = &op.oi_guess {
            format!("{prefix}called {guess}{pr_text}({which})")
        } else {
            prefix
        }
    } else if count == 1 {
        format!("A{which} {which} {typ}")
    } else {
        format!("{count} {which} {typ}s")
    };

    copy_to_prbuf(&text);
}

unsafe fn nullstr(_: *mut Thing) -> String {
    String::new()
}

#[allow(dead_code)]
fn pick_one_ex(info: &[ObjInfo], nitems: usize) -> i32 {
    pick_one(info, nitems)
}

#[allow(dead_code)]
fn set_order(order: &mut [i32]) {
    let numthings = order.len() as i32;
    for i in 0..numthings {
        order[i as usize] = i;
    }
    for i in (1..=numthings).rev() {
        let r = rnd(i);
        order.swap((i - 1) as usize, r as usize);
    }
}
