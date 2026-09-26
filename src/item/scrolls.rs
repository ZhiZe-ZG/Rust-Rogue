//! Scrolls and reading them.
//!
//! Ported from `src/c/scrolls.c` to Rust.
use crate::rnd::rnd;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};

use crate::config::GameConfig;
use crate::draw::{look, map_cell_reveal};
use crate::entity::monsters::{new_monster, randmonster};
use crate::entity::player::{MonsterFlags, ObjectFlags, Thing, ThingMonster, ThingObject};
use crate::game;
use crate::game::PLAYER;
use crate::globals::{scr_info, weap_info};
use crate::init::pick_color;
use crate::item::pack::{get_item, leave_pack};
use crate::item::thing_list::{discard, new_actor};
use crate::misc::{aggravate, call_it, choose_str, find_obj};
use crate::ui::output::{addmsg_str, endmsg, msg_str, show_win, status};
use crate::ui::{output, Window};
use crate::wizard::{teleport, whatis};
use glam::IVec2;

const SLEEPTIME: c_int = 5;

const DOOR: c_int = '+' as c_int;
const FLOOR: c_int = '.' as c_int;
const PASSAGE: c_int = '#' as c_int;
const TRAP: c_int = '^' as c_int;
const STAIRS: c_int = '%' as c_int;
const H_WALL: c_int = '-' as c_int;
const V_WALL: c_int = '|' as c_int;
const SPACE: c_int = ' ' as c_int;
const FOOD: c_int = ':' as c_int;
const POTION: c_int = '!' as c_int;
const SCROLL: c_int = '?' as c_int;
const WEAPON: c_int = ')' as c_int;
const ARMOR: c_int = ']' as c_int;
const R_OR_S: c_int = -2;

const F_PASS: c_char = 0x80u8 as c_char;
const F_SEEN: c_char = 0x40u8 as c_char;
const F_REAL: c_char = 0x10;

const MAXSCROLLS: usize = 18;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScrollType {
    Confuse = 0,
    Map = 1,
    Hold = 2,
    Sleep = 3,
    Armor = 4,
    IdentifyPotion = 5,
    IdentifyScroll = 6,
    IdentifyWeapon = 7,
    IdentifyArmor = 8,
    IdentifyRingOrStick = 9,
    Scare = 10,
    FindFood = 11,
    Teleport = 12,
    Enchant = 13,
    CreateMonster = 14,
    RemoveCurse = 15,
    Aggravate = 16,
    Protect = 17,
}

impl ScrollType {
    #[inline]
    fn from_raw(value: c_int) -> Self {
        match value {
            0 => Self::Confuse,
            1 => Self::Map,
            2 => Self::Hold,
            3 => Self::Sleep,
            4 => Self::Armor,
            5 => Self::IdentifyPotion,
            6 => Self::IdentifyScroll,
            7 => Self::IdentifyWeapon,
            8 => Self::IdentifyArmor,
            9 => Self::IdentifyRingOrStick,
            10 => Self::Scare,
            11 => Self::FindFood,
            12 => Self::Teleport,
            13 => Self::Enchant,
            14 => Self::CreateMonster,
            15 => Self::RemoveCurse,
            16 => Self::Aggravate,
            17 => Self::Protect,
            _ => panic!("invalid scroll type: {value}"),
        }
    }

    #[inline]
    const fn index(self) -> usize {
        self as usize
    }
}

unsafe extern "C" {
    static mut terse: c_uchar;
    static mut no_command: c_int;

}

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
unsafe fn moat(y: c_int, x: c_int) -> *mut Thing {
    game::monster_at(y, x) as *mut Thing
}

#[inline]
unsafe fn on_flag(tp: *mut Thing, flag: MonsterFlags) -> bool {
    (*thing_t(tp)).t_flags.contains(flag)
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    crate::game::PLAYER.has_flag(flag)
}

// Map reveal now lives in `crate::draw::map_cell_reveal`, operating directly
// on the `CURRENT_LEVEL` tile map and flag grids.

/// read_scroll:
/// Read a scroll from the pack and apply its effect.
#[no_mangle]
pub unsafe extern "C" fn read_scroll() {
    let mut obj = get_item(c"read".as_ptr(), SCROLL);
    if obj.is_null() {
        return;
    }

    if (*thing_o(obj)).o_type != SCROLL {
        if terse == 0 {
            msg_str("there is nothing on it to read");
        } else {
            msg_str("nothing to read");
        }
        return;
    }

    if obj == PLAYER.weapon() {
        PLAYER.set_weapon(std::ptr::null_mut());
    }

    let discardit = (*thing_o(obj)).o_count == 1;
    leave_pack(obj, false as c_uchar, false as c_uchar);
    let orig_obj = obj;

    let scroll_type = ScrollType::from_raw((*thing_o(obj)).o_which);
    match scroll_type {
        ScrollType::Confuse => {
            crate::game::PLAYER.add_flag(MonsterFlags::CANHUH);
            msg_str(&format!("your hands begin to glow {}", pick_color("red")));
        }
        ScrollType::Armor => {
            if !PLAYER.armor().is_null() {
                (*thing_o(PLAYER.armor())).o_arm -= 1;
                (*thing_o(PLAYER.armor()))
                    .o_flags
                    .remove(ObjectFlags::CURSED);
                msg_str(&format!(
                    "your armor glows {} for a moment",
                    pick_color("silver")
                ));
            }
        }
        ScrollType::Hold => {
            let mut ch: c_char = 0;
            let h = hero();
            for x in (h.x - 2)..=(h.x + 2) {
                if !(0..GameConfig::SCREEN_COLS).contains(&x) {
                    continue;
                }
                for y in (h.y - 2)..=(h.y + 2) {
                    if y < 0 || y > (GameConfig::SCREEN_LINES - 1) {
                        continue;
                    }
                    let tp = moat(y, x);
                    if !tp.is_null() && on_flag(tp, MonsterFlags::RUN) {
                        (*thing_t(tp)).t_flags.remove(MonsterFlags::RUN);
                        (*thing_t(tp)).t_flags.insert(MonsterFlags::HELD);
                        ch += 1;
                    }
                }
            }

            if ch != 0 {
                addmsg_str("the monster");
                if ch > 1 {
                    addmsg_str("s around you");
                }
                addmsg_str(" freeze");
                if ch == 1 {
                    addmsg_str("s");
                }
                endmsg();
                scr_info[ScrollType::Hold.index()].oi_know = true;
            } else {
                msg_str("you feel a strange sense of loss");
            }
        }
        ScrollType::Sleep => {
            scr_info[ScrollType::Sleep.index()].oi_know = true;
            no_command += rnd(SLEEPTIME) + 4;
            crate::game::PLAYER.remove_flag(MonsterFlags::RUN);
            msg_str("you fall asleep");
        }
        ScrollType::CreateMonster => {
            let mut i = 0;
            let mut mp = IVec2 { y: 0, x: 0 };
            let h = hero();
            for y in (h.y - 1)..=(h.y + 1) {
                for x in (h.x - 1)..=(h.x + 1) {
                    if y == h.y && x == h.x {
                        continue;
                    }
                    if !crate::game::cell_is_walkable(y, x) {
                        continue;
                    }
                    let found = find_obj(y, x);
                    if !found.is_null()
                        && (*thing_o(found)).o_type == SCROLL as c_int
                        && (*thing_o(found)).o_which == ScrollType::Scare as c_int
                    {
                        continue;
                    }
                    i += 1;
                    if rnd(i) == 0 {
                        mp.y = y;
                        mp.x = x;
                    }
                }
            }

            if i == 0 {
                msg_str("you hear a faint cry of anguish in the distance");
            } else {
                obj = new_actor();
                new_monster(obj, randmonster(false), &mut mp);
            }
        }
        ScrollType::IdentifyPotion
        | ScrollType::IdentifyScroll
        | ScrollType::IdentifyWeapon
        | ScrollType::IdentifyArmor
        | ScrollType::IdentifyRingOrStick => {
            let id_type: [c_int; ScrollType::IdentifyRingOrStick.index() + 1] =
                [0, 0, 0, 0, 0, POTION, SCROLL, WEAPON, ARMOR, R_OR_S];
            scr_info[(*thing_o(obj)).o_which as usize].oi_know = true;
            msg_str(&format!(
                "this scroll is an {} scroll",
                scr_info[(*thing_o(obj)).o_which as usize].oi_name
            ));
            whatis(true as c_uchar, id_type[(*thing_o(obj)).o_which as usize]);
        }
        ScrollType::Map => {
            scr_info[ScrollType::Map.index()].oi_know = true;
            msg_str("oh, now this scroll has a map on it");

            for y in 1..(GameConfig::SCREEN_LINES - 1) {
                for x in 0..GameConfig::SCREEN_COLS {
                    let ch = map_cell_reveal(y, x);
                    if ch != SPACE {
                        let tp = moat(y, x);
                        if !tp.is_null() {
                            (*thing_t(tp)).t_oldch = ch as u8;
                        }
                        if tp.is_null() || !player_has(MonsterFlags::SEEMONST) {
                            output::write_glyph_at(IVec2::new(x, y), (ch as u8) as char);
                        }
                    }
                }
            }
        }
        ScrollType::FindFood => {
            let mut found = false as c_uchar;
            let window = Window::Stdscr;
            output::clear_window(window);
            let mut it = crate::game::with_current_level(|level| level.items.head());
            while !it.is_null() {
                if (*thing_o(it)).o_type == FOOD {
                    found = true as c_uchar;
                    output::move_window_cursor(
                        window,
                        IVec2::new((*thing_o(it)).o_pos.x, (*thing_o(it)).o_pos.y),
                    );
                    output::write_window_glyph(window, (FOOD as u8) as char);
                }
                it = crate::entity::player::thing_next(it);
            }
            if found != 0 {
                scr_info[ScrollType::FindFood.index()].oi_know = true;
                show_win("Your nose tingles and you smell food.--More--");
            } else {
                msg_str("your nose tingles");
            }
        }
        ScrollType::Teleport => {
            let cur_room = proom();
            teleport();
            if cur_room != proom() {
                scr_info[ScrollType::Teleport.index()].oi_know = true;
            }
        }
        ScrollType::Enchant => {
            if PLAYER.weapon().is_null() || (*thing_o(PLAYER.weapon())).o_type != WEAPON {
                msg_str("you feel a strange sense of loss");
            } else {
                (*thing_o(PLAYER.weapon()))
                    .o_flags
                    .remove(ObjectFlags::CURSED);
                if rnd(2) == 0 {
                    (*thing_o(PLAYER.weapon())).o_hplus += 1;
                } else {
                    (*thing_o(PLAYER.weapon())).o_dplus += 1;
                }
                msg_str(&format!(
                    "your {} glows {} for a moment",
                    weap_info[(*thing_o(PLAYER.weapon())).o_which as usize].oi_name,
                    pick_color("blue")
                ));
            }
        }
        ScrollType::Scare => {
            msg_str("you hear maniacal laughter in the distance");
        }
        ScrollType::RemoveCurse => {
            uncurse(PLAYER.armor());
            uncurse(PLAYER.weapon());
            uncurse(PLAYER.left_ring());
            uncurse(PLAYER.right_ring());
            msg_str(
                &CStr::from_ptr(choose_str(
                    c"you feel in touch with the Universal Onenes".as_ptr(),
                    c"you feel as if somebody is watching over you".as_ptr(),
                ))
                .to_string_lossy(),
            );
        }
        ScrollType::Aggravate => {
            aggravate();
            msg_str("you hear a high pitched humming noise");
        }
        ScrollType::Protect => {
            if !PLAYER.armor().is_null() {
                (*thing_o(PLAYER.armor())).o_flags.insert(ObjectFlags::PROT);
                msg_str(&format!(
                    "your armor is covered by a shimmering {} shield",
                    pick_color("gold")
                ));
            } else {
                msg_str("you feel a strange sense of loss");
            }
        }
        _ => {}
    }

    obj = orig_obj;
    look(true as c_uchar);
    status();

    call_it(&mut scr_info[(*thing_o(obj)).o_which as usize]);
    if discardit {
        discard(obj);
    }
}

/// uncurse:
/// Uncurse an item.
#[no_mangle]
pub unsafe extern "C" fn uncurse(obj: *mut Thing) {
    if !obj.is_null() {
        (*thing_o(obj)).o_flags.remove(ObjectFlags::CURSED);
    }
}
