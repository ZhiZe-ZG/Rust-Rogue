use crate::rnd::rnd;
use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::draw::{chat_at as draw_chat, map_cell_reveal, winat as draw_winat};
use crate::game;
use crate::io::{addmsg_str, msg_str};

const NUMCOLS: c_int = 80;
const NUMLINES: c_int = 24;
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

const LEFT: usize = 0;
const RIGHT: usize = 1;

const ISCURSED: c_int = 0o000001;
const ISPROT: c_int = 0o000040;

const CANHUH: c_short = 0o000001;
const ISRUN: c_short = 0o020000;
const ISHELD: c_short = 0o000400;
const SEEMONST: c_short = 0o040000;

const F_PASS: c_char = 0x80u8 as c_char;
const F_SEEN: c_char = 0x40u8 as c_char;
const F_REAL: c_char = 0x10;

const S_CONFUSE: c_int = 0;
const S_MAP: c_int = 1;
const S_HOLD: c_int = 2;
const S_SLEEP: c_int = 3;
const S_ARMOR: c_int = 4;
const S_ID_POTION: c_int = 5;
const S_ID_SCROLL: c_int = 6;
const S_ID_WEAPON: c_int = 7;
const S_ID_ARMOR: c_int = 8;
const S_ID_R_OR_S: c_int = 9;
const S_SCARE: c_int = 10;
const S_FDET: c_int = 11;
const S_TELEP: c_int = 12;
const S_ENCH: c_int = 13;
const S_CREATE: c_int = 14;
const S_REMOVE: c_int = 15;
const S_AGGR: c_int = 16;
const S_PROTECT: c_int = 17;
const MAXSCROLLS: usize = 18;

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CCoord {
    pub x: c_int,
    pub y: c_int,
}

#[repr(C)]
pub struct CRoom {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CStats {
    pub s_str: c_uint,
    pub s_exp: c_int,
    pub s_lvl: c_int,
    pub s_arm: c_int,
    pub s_hpt: c_int,
    pub s_dmg: [c_char; 13],
    pub s_maxhp: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CThingMonster {
    pub l_next: *mut CThing,
    pub l_prev: *mut CThing,
    pub t_pos: CCoord,
    pub t_turn: c_uchar,
    pub t_type: c_char,
    pub t_disguise: c_char,
    pub t_oldch: c_char,
    pub t_dest: *mut CCoord,
    pub t_flags: c_short,
    pub t_stats: CStats,
    pub t_room: *mut CRoom,
    pub t_pack: *mut CThing,
    pub t_reserved: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CThingObject {
    pub l_next: *mut CThing,
    pub l_prev: *mut CThing,
    pub o_type: c_int,
    pub o_pos: CCoord,
    pub o_text: *mut c_char,
    pub o_launch: c_int,
    pub o_packch: c_char,
    pub o_damage: [c_char; 8],
    pub o_hurldmg: [c_char; 8],
    pub o_count: c_int,
    pub o_which: c_int,
    pub o_hplus: c_int,
    pub o_dplus: c_int,
    pub o_arm: c_int,
    pub o_flags: c_int,
    pub o_group: c_int,
    pub o_label: *mut c_char,
}

#[repr(C)]
pub union CThing {
    pub t: CThingMonster,
    pub o: CThingObject,
}

#[repr(C)]
pub struct CPlace {
    pub p_monst: *mut CThing,
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
    static mut cur_weapon: *mut CThing;
    static mut cur_armor: *mut CThing;
    static mut cur_ring: [*mut CThing; 2];
    static mut lvl_obj: *mut CThing;
    static mut hw: *mut c_void;
    static mut scr_info: [CObjInfo; MAXSCROLLS];
    static mut weap_info: [CObjInfo; 10];

    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn leave_pack(obj: *mut CThing, newobj: c_uchar, all: c_uchar) -> *mut CThing;
    fn discard(item: *mut CThing);
    fn pick_color(col: *const c_char) -> *mut c_char;
    fn endmsg() -> c_int;
    fn step_ok(ch: c_int) -> c_int;
    fn find_obj(y: c_int, x: c_int) -> *mut CThing;
    fn new_item() -> *mut CThing;
    fn new_monster(tp: *mut CThing, monster_type: c_char, cp: *mut CCoord);
    fn randmonster(wander: c_uchar) -> c_char;
    fn whatis(insist: c_uchar, item_type: c_int);
    fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int;
    fn wclear(win: *mut c_void) -> c_int;
    fn wmove(win: *mut c_void, y: c_int, x: c_int) -> c_int;
    fn waddch(win: *mut c_void, ch: c_uint) -> c_int;
    fn show_win(message: *const c_char);
    fn teleport();
    fn look(wakeup: c_uchar);
    fn status();
    fn call_it(info: *mut CObjInfo);
    fn choose_str(ts: *const c_char, ns: *const c_char) -> *mut c_char;
    fn aggravate();
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
unsafe fn hero() -> CCoord {
    (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn proom() -> *mut CRoom {
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

    if obj == cur_weapon {
        cur_weapon = std::ptr::null_mut();
    }

    let discardit = (*thing_o(obj)).o_count == 1;
    leave_pack(obj, FALSE, FALSE);
    let orig_obj = obj;

    match (*thing_o(obj)).o_which {
        S_CONFUSE => {
            (*thing_t(&raw mut player)).t_flags |= CANHUH;
            msg_str(&format!(
                "your hands begin to glow {}",
                CStr::from_ptr(pick_color(c"red".as_ptr())).to_string_lossy()
            ));
        }
        S_ARMOR => {
            if !cur_armor.is_null() {
                (*thing_o(cur_armor)).o_arm -= 1;
                (*thing_o(cur_armor)).o_flags &= !ISCURSED;
                msg_str(&format!(
                    "your armor glows {} for a moment",
                    CStr::from_ptr(pick_color(c"silver".as_ptr())).to_string_lossy()
                ));
            }
        }
        S_HOLD => {
            let mut ch: c_char = 0;
            let h = hero();
            for x in (h.x - 2)..=(h.x + 2) {
                if !(0..NUMCOLS).contains(&x) {
                    continue;
                }
                for y in (h.y - 2)..=(h.y + 2) {
                    if y < 0 || y > (NUMLINES - 1) {
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
                scr_info[S_HOLD as usize].oi_know = TRUE;
            } else {
                msg_str("you feel a strange sense of loss");
            }
        }
        S_SLEEP => {
            scr_info[S_SLEEP as usize].oi_know = TRUE;
            no_command += rnd(SLEEPTIME) + 4;
            (*thing_t(&raw mut player)).t_flags &= !ISRUN;
            msg_str("you fall asleep");
        }
        S_CREATE => {
            let mut i = 0;
            let mut mp = CCoord { y: 0, x: 0 };
            let h = hero();
            for y in (h.y - 1)..=(h.y + 1) {
                for x in (h.x - 1)..=(h.x + 1) {
                    if y == h.y && x == h.x {
                        continue;
                    }
                    let ch = winat(y, x);
                    if step_ok(ch) == 0 {
                        continue;
                    }
                    if ch == SCROLL {
                        let found = find_obj(y, x);
                        if !found.is_null() && (*thing_o(found)).o_which == S_SCARE {
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
                new_monster(obj, randmonster(FALSE), &mut mp);
            }
        }
        S_ID_POTION | S_ID_SCROLL | S_ID_WEAPON | S_ID_ARMOR | S_ID_R_OR_S => {
            let id_type: [c_int; (S_ID_R_OR_S as usize) + 1] =
                [0, 0, 0, 0, 0, POTION, SCROLL, WEAPON, ARMOR, R_OR_S];
            scr_info[(*thing_o(obj)).o_which as usize].oi_know = TRUE;
            msg_str(&format!(
                "this scroll is an {} scroll",
                CStr::from_ptr(scr_info[(*thing_o(obj)).o_which as usize].oi_name).to_string_lossy()
            ));
            whatis(TRUE, id_type[(*thing_o(obj)).o_which as usize]);
        }
        S_MAP => {
            scr_info[S_MAP as usize].oi_know = TRUE;
            msg_str("oh, now this scroll has a map on it");

    for y in 1..(NUMLINES - 1) {
        for x in 0..NUMCOLS {
            let ch = map_cell_reveal(y, x);
            if ch != SPACE {
                let tp = moat(y, x);
                if !tp.is_null() {
                    (*thing_t(tp)).t_oldch = ch as c_char;
                }
                if tp.is_null() || !player_has(SEEMONST) {
                    mvaddch(y, x, ch as c_uint);
                }
            }
        }
    }
        }
        S_FDET => {
            let mut found = FALSE;
            wclear(hw);
            let mut it = lvl_obj;
            while !it.is_null() {
                if (*thing_o(it)).o_type == FOOD {
                    found = TRUE;
                    wmove(hw, (*thing_o(it)).o_pos.y, (*thing_o(it)).o_pos.x);
                    waddch(hw, FOOD as c_uint);
                }
                it = (*thing_o(it)).l_next;
            }
            if found != 0 {
                scr_info[S_FDET as usize].oi_know = TRUE;
                show_win(c"Your nose tingles and you smell food.--More--".as_ptr());
            } else {
                msg_str("your nose tingles");
            }
        }
        S_TELEP => {
            let cur_room = proom();
            teleport();
            if cur_room != proom() {
                scr_info[S_TELEP as usize].oi_know = TRUE;
            }
        }
        S_ENCH => {
            if cur_weapon.is_null() || (*thing_o(cur_weapon)).o_type != WEAPON {
                msg_str("you feel a strange sense of loss");
            } else {
                (*thing_o(cur_weapon)).o_flags &= !ISCURSED;
                if rnd(2) == 0 {
                    (*thing_o(cur_weapon)).o_hplus += 1;
                } else {
                    (*thing_o(cur_weapon)).o_dplus += 1;
                }
                msg_str(&format!(
                    "your {} glows {} for a moment",
                    CStr::from_ptr(weap_info[(*thing_o(cur_weapon)).o_which as usize].oi_name).to_string_lossy(),
                    CStr::from_ptr(pick_color(c"blue".as_ptr())).to_string_lossy()
                ));
            }
        }
        S_SCARE => {
            msg_str("you hear maniacal laughter in the distance");
        }
        S_REMOVE => {
            uncurse(cur_armor);
            uncurse(cur_weapon);
            uncurse(cur_ring[LEFT]);
            uncurse(cur_ring[RIGHT]);
            msg_str(&CStr::from_ptr(choose_str(
                c"you feel in touch with the Universal Onenes".as_ptr(),
                c"you feel as if somebody is watching over you".as_ptr(),
            )).to_string_lossy());
        }
        S_AGGR => {
            aggravate();
            msg_str("you hear a high pitched humming noise");
        }
        S_PROTECT => {
            if !cur_armor.is_null() {
                (*thing_o(cur_armor)).o_flags |= ISPROT;
                msg_str(&format!(
                    "your armor is covered by a shimmering {} shield",
                    CStr::from_ptr(pick_color(c"gold".as_ptr())).to_string_lossy()
                ));
            } else {
                msg_str("you feel a strange sense of loss");
            }
        }
        _ => {}
    }

    obj = orig_obj;
    look(TRUE);
    status();

    call_it(&mut scr_info[(*thing_o(obj)).o_which as usize]);
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
