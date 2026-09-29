//! Wizard (debug) mode commands.
//!
//! Ported from `src/c/wizard.c` to Rust.
use crate::rnd::rnd;

use crate::config::GameConfig;
use crate::draw::{self, enter_room, leave_room, look};
use crate::entity::chase::roomin;
use crate::entity::player::{MonsterFlags, ObjectFlags, Thing};
use crate::game::globals::{monsters, pot_info, ring_info, scr_info, ws_info, ObjInfo};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::{add_pack_id, floor_at, get_item_id};
use crate::item::sticks::fix_stick_id;
use crate::item::arena::{new_item_id, ThingId, OBJECTS};
use crate::item::things::inv_name_id;
use crate::item::weapons::init_weapon;
use crate::level::find_floor;
use crate::machdep::flush_type;
use crate::ui::input::readchar;
use crate::ui::output::{msg_str, show_win};
use crate::ui::{output, Window};
use glam::IVec2;

const FOOD: i32 = b':' as i32;
const GOLD: i32 = b'*' as i32;

const F_REAL: u8 = 0x10u8 as u8;

static mut master_mode_enabled: u8 = 1;
static mut wizard: i32 = 0;

#[inline]
fn hero() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
fn proom() -> Option<usize> {
    crate::game::PLAYER.room()
}

#[inline]
unsafe fn flat(y: i32, x: i32) -> u8 {
    draw::flat_at(y, x)
}

#[inline]
unsafe fn chat(y: i32, x: i32) -> i32 {
    draw::cell_glyph(y, x) as u8 as i32
}

#[inline]
unsafe fn get_num() -> i32 {
    let mut value = 0;
    let mut ch = readchar();
    while ch == (b' ' as i32) || ch == (b'\t' as i32) {
        ch = readchar();
    }
    while ch >= (b'0' as i32) && ch <= (b'9' as i32) {
        value = value * 10 + (ch - b'0' as i32);
        ch = readchar();
    }
    value
}

#[inline]
unsafe fn master_enabled() -> bool {
    master_mode_enabled != 0
}

use crate::game::globals::{a_class, count, mpos, n_objs, no_move, running, vf_hit};


pub unsafe fn whatis(insist: u8, filter: ItemFilter) {
    let pack = crate::game::PLAYER.pack();
    if pack.is_empty() {
        msg_str("you don't have anything in your pack to identify");
        return;
    }

    let mut obj: Option<ThingId> = None;
    loop {
        obj = get_item_id("identify", filter);
        if insist != 0 {
            if n_objs == 0 {
                return;
            } else if obj.is_none() {
                msg_str("you must identify something");
            } else if !filter.matches(
                OBJECTS.with_object(obj.unwrap(), |o| o.o_type).unwrap_or(ItemType::None),
            ) {
                msg_str(&format!("you must identify a {}", type_name(filter)));
            } else {
                break;
            }
        } else {
            break;
        }
    }

    let Some(obj) = obj else {
        return;
    };

    let otype = OBJECTS.with_object(obj, |o| o.o_type).unwrap_or(ItemType::None);
    match otype {
        ItemType::Scroll(_) => set_know_id(obj, &mut scr_info[..]),
        ItemType::Potion(_) => set_know_id(obj, &mut pot_info[..]),
        ItemType::Stick(_) => set_know_id(obj, &mut ws_info[..]),
        ItemType::Weapon(_) | ItemType::Armor(_) => {
            OBJECTS.with_object_mut(obj, |o| o.o_flags.insert(ObjectFlags::KNOW));
        }
        ItemType::Ring(_) => set_know_id(obj, &mut ring_info[..]),
        _ => {}
    }

    msg_str(&inv_name_id(obj, false));
}

pub unsafe fn set_know_id(id: ThingId, info: &mut [ObjInfo]) {
    let idx = OBJECTS.with_object(id, |o| o.o_which).unwrap_or(0) as usize;
    if let Some(item) = info.get_mut(idx) {
        item.oi_know = true;
        item.oi_guess = None;
    }
    OBJECTS.with_object_mut(id, |o| o.o_flags.insert(ObjectFlags::KNOW));
}

pub fn type_name(filter: ItemFilter) -> &'static str {
    match filter {
        ItemFilter::Category(ItemType::Potion(_)) => "potion",
        ItemFilter::Category(ItemType::Scroll(_)) => "scroll",
        ItemFilter::Category(ItemType::Food) => "food",
        ItemFilter::RingOrStick => "ring, wand or staff",
        ItemFilter::Category(ItemType::Ring(_)) => "ring",
        ItemFilter::Category(ItemType::Stick(_)) => "wand or staff",
        ItemFilter::Category(ItemType::Weapon(_)) => "weapon",
        ItemFilter::Category(ItemType::Armor(_)) => "suit of armor",
        _ => "",
    }
}

pub unsafe fn create_obj() {
    if !master_enabled() {
        return;
    }

    let obj = new_item_id();
    let mut ch: i32;

    msg_str("type of item: ");
    let type_ch = readchar();
    mpos = 0;
    msg_str(&format!(
        "which {} do you want? (0-f)",
        (type_ch as u8) as char
    ));
    ch = readchar();
    let which = if (ch as u8).is_ascii_digit() {
        ch - b'0' as i32
    } else {
        ch - b'a' as i32 + 10
    };
    OBJECTS.with_object_mut(obj, |o| {
        o.o_which = which;
        o.o_type = ItemType::from_raw(type_ch, which);
        o.o_group = 0;
        o.o_count = 1;
    });
    mpos = 0;

    let otype = OBJECTS.with_object(obj, |o| o.o_type).unwrap_or(ItemType::None);
    match otype {
        ItemType::Weapon(_) | ItemType::Armor(_) => {
            msg_str("blessing? (+,-,n)");
            let bless = readchar() as u8;
            mpos = 0;
            if bless == ('-' as u8) {
                OBJECTS.with_object_mut(obj, |o| o.o_flags.insert(ObjectFlags::CURSED));
            }
            if matches!(otype, ItemType::Weapon(_)) {
                let w = OBJECTS.with_object(obj, |o| o.o_which).unwrap_or(0);
                init_weapon(obj, w);
                if bless == ('-' as u8) {
                    OBJECTS.with_object_mut(obj, |o| o.o_hplus -= rnd(3) + 1);
                }
                if bless == ('+' as u8) {
                    OBJECTS.with_object_mut(obj, |o| o.o_hplus += rnd(3) + 1);
                }
            } else {
                let w = OBJECTS.with_object(obj, |o| o.o_which).unwrap_or(0);
                OBJECTS.with_object_mut(obj, |o| o.o_arm = a_class[w as usize]);
                if bless == ('-' as u8) {
                    OBJECTS.with_object_mut(obj, |o| o.o_arm += rnd(3) + 1);
                }
                if bless == ('+' as u8) {
                    OBJECTS.with_object_mut(obj, |o| o.o_arm -= rnd(3) + 1);
                }
            }
        }
        ItemType::Ring(_) => {
            let which = OBJECTS.with_object(obj, |o| o.o_which).unwrap_or(0);
            match which {
                0 | 1 | 2 | 3 | 6 | 7 => {
                    msg_str("blessing? (+,-,n)");
                    let bless = readchar() as u8;
                    mpos = 0;
                    if bless == ('-' as u8) {
                        OBJECTS.with_object_mut(obj, |o| o.o_flags.insert(ObjectFlags::CURSED));
                    }
                    let arm = if bless == ('-' as u8) { -1 } else { rnd(2) + 1 };
                    OBJECTS.with_object_mut(obj, |o| o.o_arm = arm);
                }
                _ => {
                    OBJECTS.with_object_mut(obj, |o| o.o_flags.insert(ObjectFlags::CURSED));
                }
            }
        }
        ItemType::Stick(_) => {
            fix_stick_id(obj);
        }
        ItemType::Gold => {
            msg_str("how much?");
            let _amount = get_num();
        }
        _ => {}
    }

    add_pack_id(Some(obj), false);
}

pub unsafe fn teleport() {
    let mut c = find_floor(None, 0, true).unwrap_or(IVec2::ZERO);
    let mut hero = hero();

    output::write_glyph_at(IVec2::new(hero.x, hero.y), (floor_at() as u8) as char);
    if roomin(c) != proom() {
        leave_room(hero);
        hero = c;
        enter_room(hero);
    } else {
        hero = c;
        look(true as u8);
    }
    crate::game::PLAYER.set_pos(hero);
    output::write_glyph_at(IVec2::new(hero.x, hero.y), '@');

    if crate::game::PLAYER.has_flag(MonsterFlags::HELD) {
        crate::game::PLAYER.remove_flag(MonsterFlags::HELD);
        vf_hit = 0;
        let dmg = b"000x0\0";
        let damage = &mut monsters[(b'F' - b'A') as usize].m_stats.damage;
        damage[..dmg.len()].copy_from_slice(dmg);
    }
    no_move = 0;
    count = 0;
    running = false as u8;
    flush_type();
}

pub unsafe fn show_map() {
    if !master_enabled() {
        return;
    }

    let window = Window::Stdscr;
    output::clear_window(window);
    for y in 1..(GameConfig::SCREEN_LINES - 1) {
        for x in 0..GameConfig::SCREEN_COLS {
            let real = flat(y, x);
            if ((real as u8) & (F_REAL as u8)) == 0 {
                output::set_window_standout(window, true);
            }
            output::move_window_cursor(window, IVec2::new(x, y));
            output::write_window_glyph(window, (chat(y, x) as u8) as char);
            if real == 0 {
                output::set_window_standout(window, false);
            }
        }
    }
    show_win("---More (level map)---");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_name_matches_expected_strings() {
        assert_eq!(type_name(ItemFilter::Category(ItemType::POTION)), "potion");
        assert_eq!(type_name(ItemFilter::Category(ItemType::SCROLL)), "scroll");
        assert_eq!(
            type_name(ItemFilter::Category(ItemType::ARMOR)),
            "suit of armor"
        );
    }

    #[test]
    fn set_know_marks_object_known() {
        unsafe {
            let id = crate::item::arena::new_item_id();
            crate::item::arena::OBJECTS.with_object_mut(id, |o| {
                o.o_type = ItemType::SCROLL;
                o.o_which = 0;
                o.o_count = 1;
            });

            let mut info = [ObjInfo {
                oi_name: "",
                oi_prob: 0,
                oi_worth: 0,
                oi_guess: None,
                oi_know: false,
            }];

            set_know_id(id, &mut info[..]);
            assert_eq!(info[0].oi_know, true);
            assert!(crate::item::arena::with_object(id, |o| {
                o.o_flags.contains(ObjectFlags::KNOW)
            })
            .unwrap());
            let _ = crate::item::arena::OBJECTS.remove(id);
        }
    }
}
