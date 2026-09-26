//! Wizard (debug) mode commands.
//!
//! Ported from `src/c/wizard.c` to Rust.
use crate::rnd::rnd;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};
use std::ptr;

use crate::config::GameConfig;
use crate::draw::{self, enter_room, leave_room, look};
use crate::entity::chase::roomin;
use crate::entity::player::{MonsterFlags, ObjectFlags, Thing, ThingMonster, ThingObject};
use crate::globals::{monsters, pot_info, ring_info, scr_info, ws_info, CObjInfo};
use crate::item::pack::{add_pack, floor_at, get_item};
use crate::item::sticks::fix_stick;
use crate::item::arena::new_item;
use crate::item::things::inv_name;
use crate::item::weapons::init_weapon;
use crate::level::find_floor;
use crate::machdep::flush_type;
use crate::ui::input::readchar;
use crate::ui::output::{msg_str, show_win};
use crate::ui::{output, Window};
use glam::IVec2;

const POTION: c_int = b'!' as c_int;
const SCROLL: c_int = b'?' as c_int;
const FOOD: c_int = b':' as c_int;
const R_OR_S: c_int = -2;
const RING: c_int = b'=' as c_int;
const STICK: c_int = b'/' as c_int;
const WEAPON: c_int = b')' as c_int;
const ARMOR: c_int = b']' as c_int;
const GOLD: c_int = b'*' as c_int;

const F_REAL: c_char = 0x10u8 as c_char;

static mut master_mode_enabled: c_uchar = 1;
static mut wizard: c_int = 0;

#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

#[inline]
fn hero() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
fn proom() -> Option<usize> {
    crate::game::PLAYER.room()
}

#[inline]
unsafe fn flat(y: c_int, x: c_int) -> c_char {
    draw::flat_at(y, x)
}

#[inline]
unsafe fn chat(y: c_int, x: c_int) -> c_int {
    draw::cell_glyph(y, x) as c_uchar as c_int
}

#[inline]
unsafe fn get_num(ptr: *mut c_int) {
    let mut value = 0;
    let mut ch = readchar();
    while ch == (b' ' as c_int) || ch == (b'\t' as c_int) {
        ch = readchar();
    }
    while ch >= (b'0' as c_int) && ch <= (b'9' as c_int) {
        value = value * 10 + (ch - b'0' as c_int);
        ch = readchar();
    }
    *ptr = value;
}

#[inline]
unsafe fn master_enabled() -> bool {
    master_mode_enabled != 0
}

use crate::globals::{a_class, count, mpos, n_objs, no_move, running, vf_hit};


pub unsafe fn whatis(insist: c_uchar, item_type: c_int) {
    let pack = crate::game::PLAYER.pack();
    if pack.is_null() {
        msg_str("you don't have anything in your pack to identify");
        return;
    }

    let mut obj: *mut Thing = ptr::null_mut();
    loop {
        obj = get_item("identify", item_type);
        if insist != 0 {
            if n_objs == 0 {
                return;
            } else if obj.is_null() {
                msg_str("you must identify something");
            } else if item_type != 0
                && (*thing_o(obj)).o_type != item_type
                && !(item_type == R_OR_S
                    && ((*thing_o(obj)).o_type == RING || (*thing_o(obj)).o_type == STICK))
            {
                msg_str(&format!(
                    "you must identify a {}",
                    type_name(item_type)
                ));
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if obj.is_null() {
        return;
    }

    match (*thing_o(obj)).o_type {
        SCROLL => set_know(obj, std::ptr::addr_of_mut!(scr_info).cast()),
        POTION => set_know(obj, std::ptr::addr_of_mut!(pot_info).cast()),
        STICK => set_know(obj, std::ptr::addr_of_mut!(ws_info).cast()),
        WEAPON | ARMOR => (*thing_o(obj)).o_flags.insert(ObjectFlags::KNOW),
        RING => set_know(obj, std::ptr::addr_of_mut!(ring_info).cast()),
        _ => {}
    }

    msg_str(&inv_name(obj, false as c_uchar));
}

pub unsafe fn set_know(obj: *mut Thing, info: *mut CObjInfo) {
    if obj.is_null() || info.is_null() {
        return;
    }

    let idx = (*thing_o(obj)).o_which as usize;
    let item = &mut *info.add(idx);
    item.oi_know = true;
    (*thing_o(obj)).o_flags.insert(ObjectFlags::KNOW);
    item.oi_guess = None;
}

pub fn type_name(item_type: c_int) -> &'static str {
    match item_type {
        x if x == POTION => "potion",
        x if x == SCROLL => "scroll",
        x if x == FOOD => "food",
        x if x == R_OR_S => "ring, wand or staff",
        x if x == RING => "ring",
        x if x == STICK => "wand or staff",
        x if x == WEAPON => "weapon",
        x if x == ARMOR => "suit of armor",
        _ => "",
    }
}

pub unsafe fn create_obj() {
    if !master_enabled() {
        return;
    }

    let obj = new_item();
    let mut ch: c_int;

    msg_str("type of item: ");
    (*thing_o(obj)).o_type = readchar();
    mpos = 0;
    msg_str(&format!(
        "which {} do you want? (0-f)",
        (*thing_o(obj)).o_type as u8 as char
    ));
    ch = readchar();
    (*thing_o(obj)).o_which = if (ch as u8).is_ascii_digit() {
        ch - b'0' as c_int
    } else {
        ch - b'a' as c_int + 10
    };

    (*thing_o(obj)).o_group = 0;
    (*thing_o(obj)).o_count = 1;
    mpos = 0;

    if (*thing_o(obj)).o_type == WEAPON || (*thing_o(obj)).o_type == ARMOR {
        msg_str("blessing? (+,-,n)");
        let bless = readchar() as c_char;
        mpos = 0;
        if bless == ('-' as c_char) {
            (*thing_o(obj)).o_flags.insert(ObjectFlags::CURSED);
        }
        if (*thing_o(obj)).o_type == WEAPON {
            init_weapon(obj, (*thing_o(obj)).o_which);
            if bless == ('-' as c_char) {
                (*thing_o(obj)).o_hplus -= rnd(3) + 1;
            }
            if bless == ('+' as c_char) {
                (*thing_o(obj)).o_hplus += rnd(3) + 1;
            }
        } else {
            (*thing_o(obj)).o_arm = a_class[(*thing_o(obj)).o_which as usize];
            if bless == ('-' as c_char) {
                (*thing_o(obj)).o_arm += rnd(3) + 1;
            }
            if bless == ('+' as c_char) {
                (*thing_o(obj)).o_arm -= rnd(3) + 1;
            }
        }
    } else if (*thing_o(obj)).o_type == RING {
        match (*thing_o(obj)).o_which {
            0 | 1 | 2 | 3 | 6 | 7 => {
                msg_str("blessing? (+,-,n)");
                let bless = readchar() as c_char;
                mpos = 0;
                if bless == ('-' as c_char) {
                    (*thing_o(obj)).o_flags.insert(ObjectFlags::CURSED);
                }
                (*thing_o(obj)).o_arm = if bless == ('-' as c_char) {
                    -1
                } else {
                    rnd(2) + 1
                };
            }
            _ => {
                (*thing_o(obj)).o_flags.insert(ObjectFlags::CURSED);
            }
        }
    } else if (*thing_o(obj)).o_type == STICK {
        fix_stick(obj);
    } else if (*thing_o(obj)).o_type == GOLD {
        msg_str("how much?");
        let mut amount = 0;
        get_num(&mut amount);
    }

    add_pack(obj, false as c_uchar);
}

pub unsafe fn teleport() {
    let mut c = find_floor(None, 0, true).unwrap_or(IVec2::ZERO);
    let mut hero = hero();

    output::write_glyph_at(IVec2::new(hero.x, hero.y), (floor_at() as u8) as char);
    if roomin(&mut c) != proom() {
        leave_room(&mut hero);
        hero = c;
        enter_room(&mut hero);
    } else {
        hero = c;
        look(true as c_uchar);
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
    running = false as c_uchar;
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
        assert_eq!(type_name(POTION), "potion");
        assert_eq!(type_name(SCROLL), "scroll");
        assert_eq!(type_name(ARMOR), "suit of armor");
    }

    #[test]
    fn set_know_marks_object_known() {
        unsafe {
            let mut obj = Thing::Object {
                link: crate::entity::player::ThingLink::empty(),
                data: ThingObject {
                    o_type: SCROLL,
                    o_pos: IVec2 { x: 0, y: 0 },
                    o_text: None,
                    o_launch: 0,
                    o_packch: 0,
                    o_damage: [0; 8],
                    o_hurldmg: [0; 8],
                    o_count: 1,
                    o_which: 0,
                    o_hplus: 0,
                    o_dplus: 0,
                    o_arm: 0,
                    o_flags: ObjectFlags::NONE,
                    o_group: 0,
                    o_label: None,
                },
            };

            let mut info = [CObjInfo {
                oi_name: "",
                oi_prob: 0,
                oi_worth: 0,
                oi_guess: None,
                oi_know: false,
            }];

            set_know(&mut obj, info.as_mut_ptr());
            assert_eq!(info[0].oi_know, true);
            assert!((*thing_o(&mut obj)).o_flags.contains(ObjectFlags::KNOW));
        }
    }
}
