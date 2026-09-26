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
use crate::globals::{
    arm_info, pot_info, ring_info, scr_info, things, weap_info, ws_info, CObjInfo,
};
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

const POTION: i32 = b'!' as i32;
const SCROLL: i32 = b'?' as i32;
const FOOD: i32 = b':' as i32;
const WEAPON: i32 = b')' as i32;
const ARMOR: i32 = b']' as i32;
const RING: i32 = b'=' as i32;
const STICK: i32 = b'/' as i32;
const GOLD: i32 = b'*' as i32;
const AMULET: i32 = b',' as i32;

use crate::globals::{a_class, inv_describe, no_food};


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
unsafe fn item_name(typ: i32, which: i32) -> &'static str {
    match typ {
        POTION => pot_info[which as usize].oi_name,
        SCROLL => scr_info[which as usize].oi_name,
        RING => ring_info[which as usize].oi_name,
        STICK => ws_info[which as usize].oi_name,
        WEAPON => weap_info[which as usize].oi_name,
        ARMOR => arm_info[which as usize].oi_name,
        FOOD => "food",
        GOLD => "gold",
        AMULET => "the Amulet of Yendor",
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
unsafe fn pick_one(info: *mut CObjInfo, nitems: i32) -> i32 {
    let mut idx = rnd(100);
    let mut i = 0;
    while i < nitems {
        let prob = (*info.add(i as usize)).oi_prob;
        if idx < prob {
            return i;
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
        POTION => {
            let item = item_name(typ, which);
            if count == 1 {
                format!("A {item}")
            } else {
                format!("{count} {item}s")
            }
        }
        RING => {
            let item = item_name(typ, which);
            if count == 1 {
                format!("A {item} ring")
            } else {
                format!("{count} {item} rings")
            }
        }
        STICK => {
            let item = item_name(typ, which);
            if count == 1 {
                format!("A {item}")
            } else {
                format!("{count} {item}s")
            }
        }
        SCROLL => {
            let item = item_name(typ, which);
            if count == 1 {
                format!("A scroll of {item}")
            } else {
                format!("{count} scrolls of {item}")
            }
        }
        FOOD => {
            if count == 1 {
                "Some food".to_owned()
            } else {
                format!("{count} rations of food")
            }
        }
        WEAPON => {
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
        ARMOR => {
            let mut text = item_name(typ, which).to_owned();
            if let Some(label) = (*thing_o(obj)).o_label.as_ref() {
                text.push_str(" called ");
                text.push_str(label);
            }
            text
        }
        AMULET => "The Amulet of Yendor".to_owned(),
        GOLD => format!("{} Gold pieces", (*thing_o(obj)).o_group),
        _ => "something".to_owned(),
    };

    if inv_describe != 0 {
        if obj == PLAYER.armor() {
            name.push_str(" (being worn)");
        }
        if obj == PLAYER.weapon() {
            name.push_str(" (weapon in hand)");
        }
        if obj == PLAYER.left_ring() {
            name.push_str(" (on left hand)");
        } else if obj == PLAYER.right_ring() {
            name.push_str(" (on right hand)");
        }
    }

    adjust_inventory_case(&mut name, drop);
    copy_to_prbuf(&name)
}

pub unsafe fn dropcheck(obj: *mut Thing) -> u8 {
    if obj.is_null() {
        return true as u8;
    }
    if obj != PLAYER.armor()
        && obj != PLAYER.weapon()
        && obj != PLAYER.left_ring()
        && obj != PLAYER.right_ring()
    {
        return true as u8;
    }
    if (*thing_o(obj)).o_flags.contains(ObjectFlags::CURSED) {
        msg_str("you can't.  It appears to be cursed");
        return false as u8;
    }
    if obj == PLAYER.weapon() {
        PLAYER.set_weapon(std::ptr::null_mut());
    } else if obj == PLAYER.armor() {
        waste_time();
        PLAYER.set_armor(std::ptr::null_mut());
    } else {
        if obj == PLAYER.left_ring() {
            PLAYER.set_left_ring(std::ptr::null_mut());
        } else {
            PLAYER.set_right_ring(std::ptr::null_mut());
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
        pick_one(
            std::ptr::addr_of!(things).cast::<CObjInfo>().cast_mut(),
            NUMTHINGS as i32,
        ) as i32
    };
    match choice {
        0 => {
            (*thing_o(cur)).o_type = POTION;
            (*thing_o(cur)).o_which = pick_one(
                std::ptr::addr_of!(pot_info).cast::<CObjInfo>().cast_mut(),
                MAXPOTIONS as i32,
            );
        }
        1 => {
            (*thing_o(cur)).o_type = SCROLL;
            (*thing_o(cur)).o_which = pick_one(
                std::ptr::addr_of!(scr_info).cast::<CObjInfo>().cast_mut(),
                MAXSCROLLS as i32,
            );
        }
        2 => {
            (*thing_o(cur)).o_type = FOOD;
            no_food = 0;
            if rnd(10) != 0 {
                (*thing_o(cur)).o_which = 0;
            } else {
                (*thing_o(cur)).o_which = 1;
            }
        }
        3 => {
            (*thing_o(cur)).o_type = WEAPON;
            init_weapon(
                cur,
                pick_one(
                    std::ptr::addr_of!(weap_info).cast::<CObjInfo>().cast_mut(),
                    MAXWEAPONS as i32,
                ),
            );
            let r = rnd(100);
            if r < 10 {
                (*thing_o(cur)).o_flags.insert(ObjectFlags::CURSED);
                (*thing_o(cur)).o_hplus -= rnd(3) + 1;
            } else if r < 15 {
                (*thing_o(cur)).o_hplus += rnd(3) + 1;
            }
        }
        4 => {
            (*thing_o(cur)).o_type = ARMOR;
            (*thing_o(cur)).o_which = pick_one(
                std::ptr::addr_of!(arm_info).cast::<CObjInfo>().cast_mut(),
                MAXARMORS as i32,
            );
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
            (*thing_o(cur)).o_type = RING;
            let ring_type = RingType::from_raw(pick_one(
                std::ptr::addr_of!(ring_info).cast::<CObjInfo>().cast_mut(),
                MAXRINGS as i32,
            ))
            .expect("ring metadata produced an invalid ring type");
            (*thing_o(cur)).o_which = ring_type as i32;
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
            (*thing_o(cur)).o_type = STICK;
            (*thing_o(cur)).o_which = pick_one(
                std::ptr::addr_of!(ws_info).cast::<CObjInfo>().cast_mut(),
                MAXSTICKS as i32,
            );
            fix_stick(cur);
        }
        _ => {}
    }

    cur
}

pub unsafe fn drop() {
    let obj = get_item("drop", 0);
    if obj.is_null() {
        return;
    }
    if dropcheck(obj) == 0 {
        return;
    }
    let all = if ((*thing_o(obj)).o_type & 0x1) == 0 {
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
    op: *mut CObjInfo,
    prfunc: unsafe fn(*mut Thing) -> String,
) {
    if op.is_null() || obj.is_null() {
        return;
    }
    let typ = typ;
    let which = which;
    let pr_text = prfunc(obj);
    let count = (*thing_o(obj)).o_count;

    let text = if (*op).oi_know || (*op).oi_guess.is_some() {
        let prefix = if count == 1 {
            format!("A {typ} ")
        } else {
            format!("{count} {typ}s ")
        };
        if (*op).oi_know {
            format!("{prefix}of {}{}({which})", (*op).oi_name, pr_text)
        } else if let Some(guess) = &(*op).oi_guess {
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

unsafe fn pick_one_ex(info: *mut CObjInfo, nitems: i32) -> i32 {
    pick_one(info, nitems)
}

unsafe fn set_order(order: *mut i32, numthings: i32) {
    for i in 0..numthings {
        *order.add(i as usize) = i;
    }
    for i in (1..=numthings).rev() {
        let r = rnd(i);
        let t = *order.add((i - 1) as usize);
        *order.add((i - 1) as usize) = *order.add(r as usize);
        *order.add(r as usize) = t;
    }
}
