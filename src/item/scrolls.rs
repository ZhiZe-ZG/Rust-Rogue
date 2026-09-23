//! Scrolls and reading them.
//!
//! Ported from `src/c/scrolls.c` to Rust.
use crate::rnd::rnd;
use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::config::GameConfig;
use crate::draw::{chat_at as draw_chat, look, map_cell_reveal, winat as draw_winat};
use crate::entity::monsters::{new_monster, randmonster};
use crate::entity::player::{CPlace, CThing, CThingMonster, CThingObject};
use crate::game;
use crate::game::EQUIPMENT;
use crate::init::pick_color;
use crate::item::pack::{get_item, leave_pack};
use crate::item::thing_list::{discard, new_item};
use crate::level::tile_is_walkable;
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

const ISCURSED: c_int = 0o000001;
const ISPROT: c_int = 0o000040;

const CANHUH: c_short = 0o000001;
const ISRUN: c_short = 0o020000;
const ISHELD: c_short = 0o000400;
const SEEMONST: c_short = 0o040000;

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

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CObjInfo {
    pub oi_name: *mut c_char,
    pub oi_prob: c_int,
    pub oi_worth: c_int,
    pub oi_guess: *mut c_char,
    pub oi_know: c_uchar,
}

unsafe extern "C" {
    static mut terse: c_uchar;
    static mut no_command: c_int;
    static mut places: [CPlace; 32 * 80];
    static mut player: CThing;
    static mut scr_info: [CObjInfo; MAXSCROLLS];
    static mut weap_info: [CObjInfo; 10];

}

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn hero() -> IVec2 {
    (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn proom() -> Option<usize> {
    (*thing_t(&raw mut player)).t_room
}

#[inline]
unsafe fn chat(y: c_int, x: c_int) -> c_int {
    draw_chat(y, x) as c_uchar as c_int
}

#[inline]
unsafe fn moat(y: c_int, x: c_int) -> *mut CThing {
    game::monster_at(y, x) as *mut CThing
}

#[inline]
unsafe fn winat(y: c_int, x: c_int) -> c_int {
    draw_winat(y, x) as c_uchar as c_int
}

#[inline]
unsafe fn on_flag(tp: *mut CThing, flag: c_short) -> bool {
    ((*thing_t(tp)).t_flags & flag) != 0
}

#[inline]
unsafe fn player_has(flag: c_short) -> bool {
    ((*thing_t(&raw mut player)).t_flags & flag) != 0
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

    if obj == EQUIPMENT.weapon() {
        EQUIPMENT.set_weapon(std::ptr::null_mut());
    }

    let discardit = (*thing_o(obj)).o_count == 1;
    leave_pack(obj, false as c_uchar, false as c_uchar);
    let orig_obj = obj;

    let scroll_type = ScrollType::from_raw((*thing_o(obj)).o_which);
    match scroll_type {
        ScrollType::Confuse => {
            (*thing_t(&raw mut player)).t_flags |= CANHUH;
            msg_str(&format!(
                "your hands begin to glow {}",
                CStr::from_ptr(pick_color(c"red".as_ptr().cast_mut())).to_string_lossy()
            ));
        }
        ScrollType::Armor => {
            if !EQUIPMENT.armor().is_null() {
                (*thing_o(EQUIPMENT.armor())).o_arm -= 1;
                (*thing_o(EQUIPMENT.armor())).o_flags &= !ISCURSED;
                msg_str(&format!(
                    "your armor glows {} for a moment",
                    CStr::from_ptr(pick_color(c"silver".as_ptr().cast_mut())).to_string_lossy()
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
                    if !tp.is_null() && on_flag(tp, ISRUN) {
                        (*thing_t(tp)).t_flags &= !ISRUN;
                        (*thing_t(tp)).t_flags |= ISHELD;
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
                scr_info[ScrollType::Hold.index()].oi_know = true as c_uchar;
            } else {
                msg_str("you feel a strange sense of loss");
            }
        }
        ScrollType::Sleep => {
            scr_info[ScrollType::Sleep.index()].oi_know = true as c_uchar;
            no_command += rnd(SLEEPTIME) + 4;
            (*thing_t(&raw mut player)).t_flags &= !ISRUN;
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
                    let ch = winat(y, x);
                    if !tile_is_walkable(ch as u8) {
                        continue;
                    }
                    if ch == SCROLL {
                        let found = find_obj(y, x);
                        if !found.is_null()
                            && (*thing_o(found)).o_which == ScrollType::Scare as c_int
                        {
                            continue;
                        }
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
                obj = new_item();
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
            scr_info[(*thing_o(obj)).o_which as usize].oi_know = true as c_uchar;
            msg_str(&format!(
                "this scroll is an {} scroll",
                CStr::from_ptr(scr_info[(*thing_o(obj)).o_which as usize].oi_name)
                    .to_string_lossy()
            ));
            whatis(true as c_uchar, id_type[(*thing_o(obj)).o_which as usize]);
        }
        ScrollType::Map => {
            scr_info[ScrollType::Map.index()].oi_know = true as c_uchar;
            msg_str("oh, now this scroll has a map on it");

            for y in 1..(GameConfig::SCREEN_LINES - 1) {
                for x in 0..GameConfig::SCREEN_COLS {
                    let ch = map_cell_reveal(y, x);
                    if ch != SPACE {
                        let tp = moat(y, x);
                        if !tp.is_null() {
                            (*thing_t(tp)).t_oldch = ch as c_char;
                        }
                        if tp.is_null() || !player_has(SEEMONST) {
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
                it = (*thing_o(it)).l_next;
            }
            if found != 0 {
                scr_info[ScrollType::FindFood.index()].oi_know = true as c_uchar;
                show_win("Your nose tingles and you smell food.--More--");
            } else {
                msg_str("your nose tingles");
            }
        }
        ScrollType::Teleport => {
            let cur_room = proom();
            teleport();
            if cur_room != proom() {
                scr_info[ScrollType::Teleport.index()].oi_know = true as c_uchar;
            }
        }
        ScrollType::Enchant => {
            if EQUIPMENT.weapon().is_null() || (*thing_o(EQUIPMENT.weapon())).o_type != WEAPON {
                msg_str("you feel a strange sense of loss");
            } else {
                (*thing_o(EQUIPMENT.weapon())).o_flags &= !ISCURSED;
                if rnd(2) == 0 {
                    (*thing_o(EQUIPMENT.weapon())).o_hplus += 1;
                } else {
                    (*thing_o(EQUIPMENT.weapon())).o_dplus += 1;
                }
                msg_str(&format!(
                    "your {} glows {} for a moment",
                    CStr::from_ptr(
                        weap_info[(*thing_o(EQUIPMENT.weapon())).o_which as usize].oi_name,
                    )
                    .to_string_lossy(),
                    CStr::from_ptr(pick_color(c"blue".as_ptr().cast_mut())).to_string_lossy()
                ));
            }
        }
        ScrollType::Scare => {
            msg_str("you hear maniacal laughter in the distance");
        }
        ScrollType::RemoveCurse => {
            uncurse(EQUIPMENT.armor());
            uncurse(EQUIPMENT.weapon());
            uncurse(EQUIPMENT.left_ring());
            uncurse(EQUIPMENT.right_ring());
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
            if !EQUIPMENT.armor().is_null() {
                (*thing_o(EQUIPMENT.armor())).o_flags |= ISPROT;
                msg_str(&format!(
                    "your armor is covered by a shimmering {} shield",
                    CStr::from_ptr(pick_color(c"gold".as_ptr().cast_mut())).to_string_lossy()
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

    call_it((&mut scr_info[(*thing_o(obj)).o_which as usize] as *mut CObjInfo).cast());
    if discardit {
        discard(obj);
    }
}

/// uncurse:
/// Uncurse an item.
#[no_mangle]
pub unsafe extern "C" fn uncurse(obj: *mut CThing) {
    if !obj.is_null() {
        (*thing_o(obj)).o_flags &= !ISCURSED;
    }
}
