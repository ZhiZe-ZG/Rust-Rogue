use crate::rnd::rnd;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};
use std::ptr;

use crate::curses as cur;
use crate::draw;
use crate::io::msg_str;
use crate::player::{CCoord, CRoom, CThing, CThingMonster, CThingObject};

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

const POTION: c_int = b'!' as c_int;
const SCROLL: c_int = b'?' as c_int;
const FOOD: c_int = b':' as c_int;
const R_OR_S: c_int = -2;
const RING: c_int = b'=' as c_int;
const STICK: c_int = b'/' as c_int;
const WEAPON: c_int = b')' as c_int;
const ARMOR: c_int = b']' as c_int;
const GOLD: c_int = b'*' as c_int;

const ISCURSED: c_int = 0o000001;
const ISKNOW: c_int = 0o000200;
const ISHELD: c_short = 0o000400;
const F_REAL: c_char = 0x10u8 as c_char;
const NUMCOLS: c_int = 80;
const NUMLINES: c_int = 24;

static mut master_mode_enabled: c_uchar = 1;
static mut wizard: c_int = 0;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CObjInfo {
    pub oi_name: *mut c_char,
    pub oi_prob: c_int,
    pub oi_worth: c_int,
    pub oi_guess: *mut c_char,
    pub oi_know: c_uchar,
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
unsafe fn flat(y: c_int, x: c_int) -> c_char {
    draw::flat_at(y, x)
}

#[inline]
unsafe fn chat(y: c_int, x: c_int) -> c_int {
    draw::chat_at(y, x) as c_uchar as c_int
}

#[inline]
unsafe fn get_num(ptr: *mut c_int, _win: *mut std::ffi::c_void) {
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

unsafe extern "C" {
    static mut n_objs: c_int;
    static mut mpos: c_int;
    static mut a_class: [c_int; 26];
    static mut no_move: c_int;
    static mut count: c_int;
    static mut running: c_uchar;
    static mut vf_hit: c_int;
    static mut hw: *mut std::ffi::c_void;
    static mut stdscr: *mut std::ffi::c_void;
    static mut monsters: [crate::monsters::CMonster; 26];
    static mut player: CThing;
    static mut scr_info: [CObjInfo; 18];
    static mut pot_info: [CObjInfo; 14];
    static mut ws_info: [CObjInfo; 14];
    static mut ring_info: [CObjInfo; 16];

    fn inv_name(obj: *mut CThing, is_weapon: c_uchar) -> *mut c_char;
    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn new_item() -> *mut CThing;
    fn add_pack(obj: *mut CThing, all: c_uchar);
    fn init_weapon(obj: *mut CThing, which: c_int);
    fn fix_stick(obj: *mut CThing);
    fn readchar() -> c_int;
    fn isdigit(ch: c_int) -> c_int;
    fn free(ptr: *mut std::ffi::c_void);
    fn floor_at() -> c_char;
    fn find_floor(rp: *mut CRoom, cp: *mut CCoord, limit: c_uchar, monst: c_uchar);
    fn roomin(cp: *mut CCoord) -> *mut CRoom;
    fn leave_room(cp: *mut CCoord);
    fn enter_room(cp: *mut CCoord);
    fn look(wakeup: c_uchar);
    fn flush_type();
    fn show_win(message: *const c_char);
}

#[no_mangle]
pub unsafe extern "C" fn whatis(insist: c_uchar, item_type: c_int) {
    let pack = (*thing_t(&mut player)).t_pack;
    if pack.is_null() {
        msg_str("you don't have anything in your pack to identify");
        return;
    }

    let mut obj: *mut CThing = ptr::null_mut();
    loop {
        obj = get_item(c"identify".as_ptr(), item_type);
        if insist != 0 {
            if n_objs == 0 {
                return;
            } else if obj.is_null() {
                msg_str("you must identify something");
            } else if item_type != 0 && (*thing_o(obj)).o_type != item_type
                && !(item_type == R_OR_S && ((*thing_o(obj)).o_type == RING || (*thing_o(obj)).o_type == STICK))
            {
                msg_str(&format!(
                    "you must identify a {}",
                    CStr::from_ptr(type_name(item_type)).to_string_lossy()
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
        SCROLL => set_know(obj, scr_info.as_mut_ptr()),
        POTION => set_know(obj, pot_info.as_mut_ptr()),
        STICK => set_know(obj, ws_info.as_mut_ptr()),
        WEAPON | ARMOR => (*thing_o(obj)).o_flags |= ISKNOW,
        RING => set_know(obj, ring_info.as_mut_ptr()),
        _ => {}
    }

    msg_str(&CStr::from_ptr(inv_name(obj, FALSE)).to_string_lossy());
}

#[no_mangle]
pub unsafe extern "C" fn set_know(obj: *mut CThing, info: *mut CObjInfo) {
    if obj.is_null() || info.is_null() {
        return;
    }

    let idx = (*thing_o(obj)).o_which as usize;
    let item = &mut *info.add(idx);
    item.oi_know = TRUE;
    (*thing_o(obj)).o_flags |= ISKNOW;
    let guess = &mut item.oi_guess;
    if !guess.is_null() {
        free(*guess as *mut std::ffi::c_void);
        *guess = ptr::null_mut();
    }
}

#[no_mangle]
pub unsafe extern "C" fn type_name(item_type: c_int) -> *mut c_char {
    match item_type {
        x if x == POTION => c"potion".as_ptr() as *mut c_char,
        x if x == SCROLL => c"scroll".as_ptr() as *mut c_char,
        x if x == FOOD => c"food".as_ptr() as *mut c_char,
        x if x == R_OR_S => c"ring, wand or staff".as_ptr() as *mut c_char,
        x if x == RING => c"ring".as_ptr() as *mut c_char,
        x if x == STICK => c"wand or staff".as_ptr() as *mut c_char,
        x if x == WEAPON => c"weapon".as_ptr() as *mut c_char,
        x if x == ARMOR => c"suit of armor".as_ptr() as *mut c_char,
        _ => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn create_obj() {
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
    (*thing_o(obj)).o_which = if isdigit(ch) != 0 {
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
            (*thing_o(obj)).o_flags |= ISCURSED;
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
                    (*thing_o(obj)).o_flags |= ISCURSED;
                }
                (*thing_o(obj)).o_arm = if bless == ('-' as c_char) { -1 } else { rnd(2) + 1 };
            }
            _ => {
                (*thing_o(obj)).o_flags |= ISCURSED;
            }
        }
    } else if (*thing_o(obj)).o_type == STICK {
        fix_stick(obj);
    } else if (*thing_o(obj)).o_type == GOLD {
        msg_str("how much?");
        let mut amount = 0;
        get_num(&mut amount, stdscr);
    }

    add_pack(obj, FALSE);
}

#[no_mangle]
pub unsafe extern "C" fn teleport() {
    let mut c = CCoord { x: 0, y: 0 };
    let mut hero = hero();

    cur::mvaddch(hero.y, hero.x, floor_at() as c_uint);
    find_floor(ptr::null_mut(), &mut c, FALSE, TRUE);
    if roomin(&mut c) != proom() {
        leave_room(&mut hero);
        hero = c;
        enter_room(&mut hero);
    } else {
        hero = c;
        look(TRUE);
    }
    (*thing_t(&raw mut player)).t_pos = hero;
    cur::mvaddch(hero.y, hero.x, b'@' as c_uint);

    if ((*thing_t(&raw mut player)).t_flags & ISHELD) != 0 {
        (*thing_t(&raw mut player)).t_flags &= !ISHELD;
        vf_hit = 0;
        let dmg = b"000x0\0";
        std::ptr::copy_nonoverlapping(dmg.as_ptr() as *const c_char, (&mut monsters[('F' as u8 - 'A' as u8) as usize].m_stats.s_dmg[0]) as *mut c_char, dmg.len());
    }
    no_move = 0;
    count = 0;
    running = FALSE;
    flush_type();
}

#[no_mangle]
pub unsafe extern "C" fn show_map() {
    if !master_enabled() {
        return;
    }

    cur::wclear(hw);
    for y in 1..(NUMLINES - 1) {
        for x in 0..NUMCOLS {
            let real = flat(y, x);
            if ((real as u8) & (F_REAL as u8)) == 0 {
                cur::wstandout(hw);
            }
            cur::wmove(hw, y, x);
            cur::waddch(hw, chat(y, x) as c_uint);
            if real == 0 {
                cur::wstandend(hw);
            }
        }
    }
    show_win(c"---More (level map)---".as_ptr());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_name_matches_expected_strings() {
        unsafe {
            let potion = CStr::from_ptr(type_name(POTION)).to_string_lossy().into_owned();
            let scroll = CStr::from_ptr(type_name(SCROLL)).to_string_lossy().into_owned();
            let armor = CStr::from_ptr(type_name(ARMOR)).to_string_lossy().into_owned();

            assert_eq!(potion, "potion");
            assert_eq!(scroll, "scroll");
            assert_eq!(armor, "suit of armor");
        }
    }

    #[test]
    fn set_know_marks_object_known() {
        unsafe {
            let mut obj = CThing {
                o: CThingObject {
                    l_next: ptr::null_mut(),
                    l_prev: ptr::null_mut(),
                    o_type: SCROLL,
                    o_pos: CCoord { x: 0, y: 0 },
                    o_text: ptr::null_mut(),
                    o_launch: 0,
                    o_packch: 0,
                    o_damage: [0; 8],
                    o_hurldmg: [0; 8],
                    o_count: 1,
                    o_which: 0,
                    o_hplus: 0,
                    o_dplus: 0,
                    o_arm: 0,
                    o_flags: 0,
                    o_group: 0,
                    o_label: ptr::null_mut(),
                },
            };

            let mut info = [CObjInfo {
                oi_name: ptr::null_mut(),
                oi_prob: 0,
                oi_worth: 0,
                oi_guess: ptr::null_mut(),
                oi_know: FALSE,
            }];

            set_know(&mut obj, info.as_mut_ptr());
            assert_eq!(info[0].oi_know, TRUE);
            assert!(((*thing_o(&mut obj)).o_flags & ISKNOW) != 0);
        }
    }
}
