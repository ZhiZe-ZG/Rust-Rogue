use crate::rnd::rnd;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

use crate::player::{CThing, CThingObject};

const MAXSTR: usize = 1024;
const NUMTHINGS: usize = 7;
const MAXARMORS: usize = 8;
const MAXPOTIONS: usize = 14;
const MAXRINGS: usize = 13;
const MAXSCROLLS: usize = 18;
const MAXWEAPONS: usize = 9;
const MAXSTICKS: usize = 14;
const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

const POTION: c_int = b'!' as c_int;
const SCROLL: c_int = b'?' as c_int;
const FOOD: c_int = b':' as c_int;
const WEAPON: c_int = b')' as c_int;
const ARMOR: c_int = b']' as c_int;
const RING: c_int = b'=' as c_int;
const STICK: c_int = b'/' as c_int;
const GOLD: c_int = b'*' as c_int;
const AMULET: c_int = b',' as c_int;

const LEFT: c_int = 0;
const RIGHT: c_int = 1;
const ISCURSED: c_int = 0o000001;
const ISKNOW: c_int = 0o000200;

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
    static mut after: c_uchar;
    static mut a_class: [c_int; 26];
    static mut amulet: c_uchar;
    static mut arm_info: [CObjInfo; MAXARMORS];
    static mut cur_armor: *mut CThing;
    static mut cur_ring: [*mut CThing; 2];
    static mut cur_weapon: *mut CThing;
    static mut fruit: [c_char; MAXSTR];
    static mut inv_describe: c_uchar;
    static mut lvl_obj: *mut CThing;
    static mut no_food: c_int;
    static mut player: CThing;
    static mut pot_info: [CObjInfo; MAXPOTIONS];
    static mut prbuf: [c_char; MAXSTR];
    static mut ring_info: [CObjInfo; MAXRINGS];
    static mut scr_info: [CObjInfo; MAXSCROLLS];
    static mut terse: c_uchar;
    static mut things: [CObjInfo; NUMTHINGS];
    static mut weap_info: [CObjInfo; MAXWEAPONS + 1];
    static mut ws_info: [CObjInfo; MAXSTICKS];

    fn chg_str(amt: c_int);
    fn extinguish(func: *const c_void);
    fn fix_stick(obj: *mut CThing);
    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn init_weapon(obj: *mut CThing, which: c_int);
    fn isupper(ch: c_int) -> c_int;
    fn leave_pack(obj: *mut CThing, newobj: c_uchar, all: c_uchar) -> *mut CThing;
    fn msg(fmt: *const c_char, ...);
    fn new_item() -> *mut CThing;
    fn sprintf(buf: *mut c_char, fmt: *const c_char, ...) -> c_int;
    fn strcat(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strlen(s: *const c_char) -> usize;
    fn tolower(ch: c_int) -> c_int;
    fn unsee();
    fn waste_time();
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn is_vowel(ch: c_char) -> bool {
    matches!(ch as u8, b'a' | b'A' | b'e' | b'E' | b'i' | b'I' | b'o' | b'O' | b'u' | b'U')
}

#[inline]
unsafe fn starts_with_article(name: *const c_char) -> *mut c_char {
    if !name.is_null() && is_vowel(*name) {
        c"an ".as_ptr() as *mut c_char
    } else {
        c"a ".as_ptr() as *mut c_char
    }
}

#[inline]
unsafe fn item_name(typ: c_int, which: c_int) -> *mut c_char {
    match typ {
        POTION => pot_info[which as usize].oi_name,
        SCROLL => scr_info[which as usize].oi_name,
        RING => ring_info[which as usize].oi_name,
        STICK => ws_info[which as usize].oi_name,
        WEAPON => weap_info[which as usize].oi_name,
        ARMOR => arm_info[which as usize].oi_name,
        FOOD => c"food".as_ptr() as *mut c_char,
        GOLD => c"gold".as_ptr() as *mut c_char,
        AMULET => c"the Amulet of Yendor".as_ptr() as *mut c_char,
        _ => c"item".as_ptr() as *mut c_char,
    }
}

#[inline]
unsafe fn pick_one(info: *mut CObjInfo, nitems: c_int) -> c_int {
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

#[no_mangle]
pub unsafe extern "C" fn inv_name(obj: *mut CThing, drop: c_uchar) -> *mut c_char {
    if obj.is_null() {
        return prbuf.as_mut_ptr();
    }

    let which = (*thing_o(obj)).o_which;
    let typ = (*thing_o(obj)).o_type;
    let count = (*thing_o(obj)).o_count;
    let mut empty = prbuf.as_mut_ptr();
    *empty = 0;

    match typ {
        POTION => {
            if count == 1 {
                sprintf(empty, c"A %s".as_ptr(), pot_info[which as usize].oi_name);
            } else {
                sprintf(empty, c"%d %ss".as_ptr(), count, pot_info[which as usize].oi_name);
            }
        }
        RING => {
            if count == 1 {
                sprintf(empty, c"A %s ring".as_ptr(), ring_info[which as usize].oi_name);
            } else {
                sprintf(empty, c"%d %s rings".as_ptr(), count, ring_info[which as usize].oi_name);
            }
        }
        STICK => {
            if count == 1 {
                sprintf(empty, c"A %s".as_ptr(), ws_info[which as usize].oi_name);
            } else {
                sprintf(empty, c"%d %ss".as_ptr(), count, ws_info[which as usize].oi_name);
            }
        }
        SCROLL => {
            if count == 1 {
                sprintf(empty, c"A scroll of %s".as_ptr(), scr_info[which as usize].oi_name);
            } else {
                sprintf(empty, c"%d scrolls of %s".as_ptr(), count, scr_info[which as usize].oi_name);
            }
        }
        FOOD => {
            if count == 1 {
                sprintf(empty, c"Some food".as_ptr());
            } else {
                sprintf(empty, c"%d rations of food".as_ptr(), count);
            }
        }
        WEAPON => {
            let name = weap_info[which as usize].oi_name;
            if (*thing_o(obj)).o_count > 1 {
                sprintf(empty, c"%d %ss".as_ptr(), count, name);
            } else {
                let article = starts_with_article(name);
                sprintf(empty, c"%s%s".as_ptr(), article, name);
            }
            if !(*thing_o(obj)).o_label.is_null() {
                let label = (*thing_o(obj)).o_label;
                strcat(empty, c" called ".as_ptr());
                strcat(empty, label);
            }
        }
        ARMOR => {
            let name = arm_info[which as usize].oi_name;
            sprintf(empty, c"%s".as_ptr(), name);
            if !(*thing_o(obj)).o_label.is_null() {
                let label = (*thing_o(obj)).o_label;
                strcat(empty, c" called ".as_ptr());
                strcat(empty, label);
            }
        }
        AMULET => {
            strcpy(empty, c"The Amulet of Yendor".as_ptr());
        }
        GOLD => {
            sprintf(empty, c"%d Gold pieces".as_ptr(), (*thing_o(obj)).o_group);
        }
        _ => {
            strcpy(empty, c"something".as_ptr());
        }
    }

    if inv_describe != 0 {
        if obj == cur_armor {
            strcat(empty, c" (being worn)".as_ptr());
        }
        if obj == cur_weapon {
            strcat(empty, c" (weapon in hand)".as_ptr());
        }
        if obj == cur_ring[LEFT as usize] {
            strcat(empty, c" (on left hand)".as_ptr());
        } else if obj == cur_ring[RIGHT as usize] {
            strcat(empty, c" (on right hand)".as_ptr());
        }
    }

    if drop != 0 {
        let first = *empty as c_int;
        if first != 0 && isupper(first) != 0 {
            *empty = tolower(first) as c_char;
        }
    } else if *empty as c_int != 0 && isupper(*empty as c_int) == 0 {
        *empty = toupper(*empty as c_int) as c_char;
    }
    prbuf[MAXSTR - 1] = 0;
    prbuf.as_mut_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn dropcheck(obj: *mut CThing) -> c_uchar {
    if obj.is_null() {
        return TRUE;
    }
    if obj != cur_armor && obj != cur_weapon && obj != cur_ring[LEFT as usize] && obj != cur_ring[RIGHT as usize] {
        return TRUE;
    }
    if ((*thing_o(obj)).o_flags & ISCURSED) != 0 {
        msg(c"you can't.  It appears to be cursed".as_ptr());
        return FALSE;
    }
    if obj == cur_weapon {
        cur_weapon = std::ptr::null_mut();
    } else if obj == cur_armor {
        waste_time();
        cur_armor = std::ptr::null_mut();
    } else {
        let idx = if obj == cur_ring[LEFT as usize] { LEFT } else { RIGHT };
        cur_ring[idx as usize] = std::ptr::null_mut();
        match (*thing_o(obj)).o_which {
            0 => chg_str(-(*thing_o(obj)).o_arm),
            _ => {}
        }
    }
    TRUE
}

#[no_mangle]
pub unsafe extern "C" fn new_thing() -> *mut CThing {
    let cur = new_item();
    (*thing_o(cur)).o_hplus = 0;
    (*thing_o(cur)).o_dplus = 0;
    std::ptr::copy_nonoverlapping(c"0x0".as_ptr(), (*thing_o(cur)).o_damage.as_mut_ptr(), 4);
    std::ptr::copy_nonoverlapping(c"0x0".as_ptr(), (*thing_o(cur)).o_hurldmg.as_mut_ptr(), 4);
    (*thing_o(cur)).o_arm = 11;
    (*thing_o(cur)).o_count = 1;
    (*thing_o(cur)).o_group = 0;
    (*thing_o(cur)).o_flags = 0;

    let choice = if no_food > 3 { 2 } else { pick_one(things.as_ptr() as *mut CObjInfo, NUMTHINGS as c_int) as c_int };
    match choice {
        0 => {
            (*thing_o(cur)).o_type = POTION;
            (*thing_o(cur)).o_which = pick_one(pot_info.as_ptr() as *mut CObjInfo, MAXPOTIONS as c_int);
        }
        1 => {
            (*thing_o(cur)).o_type = SCROLL;
            (*thing_o(cur)).o_which = pick_one(scr_info.as_ptr() as *mut CObjInfo, MAXSCROLLS as c_int);
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
            init_weapon(cur, pick_one(weap_info.as_ptr() as *mut CObjInfo, MAXWEAPONS as c_int));
            let r = rnd(100);
            if r < 10 {
                (*thing_o(cur)).o_flags |= ISCURSED;
                (*thing_o(cur)).o_hplus -= rnd(3) + 1;
            } else if r < 15 {
                (*thing_o(cur)).o_hplus += rnd(3) + 1;
            }
        }
        4 => {
            (*thing_o(cur)).o_type = ARMOR;
            (*thing_o(cur)).o_which = pick_one(arm_info.as_ptr() as *mut CObjInfo, MAXARMORS as c_int);
            (*thing_o(cur)).o_arm = a_class[(*thing_o(cur)).o_which as usize];
            let r = rnd(100);
            if r < 20 {
                (*thing_o(cur)).o_flags |= ISCURSED;
                (*thing_o(cur)).o_arm += rnd(3) + 1;
            } else if r < 28 {
                (*thing_o(cur)).o_arm -= rnd(3) + 1;
            }
        }
        5 => {
            (*thing_o(cur)).o_type = RING;
            (*thing_o(cur)).o_which = pick_one(ring_info.as_ptr() as *mut CObjInfo, MAXRINGS as c_int);
            match (*thing_o(cur)).o_which {
                0 | 2 | 7 | 8 => {
                    let mut arm = rnd(3);
                    if arm == 0 {
                        arm = -1;
                        (*thing_o(cur)).o_flags |= ISCURSED;
                    }
                    (*thing_o(cur)).o_arm = arm;
                }
                5 | 6 => {
                    (*thing_o(cur)).o_flags |= ISCURSED;
                }
                _ => {}
            }
        }
        6 => {
            (*thing_o(cur)).o_type = STICK;
            (*thing_o(cur)).o_which = pick_one(ws_info.as_ptr() as *mut CObjInfo, MAXSTICKS as c_int);
            fix_stick(cur);
        }
        _ => {}
    }

    cur
}

#[no_mangle]
pub unsafe extern "C" fn drop() {
    let obj = get_item(c"drop".as_ptr(), 0);
    if obj.is_null() {
        return;
    }
    if dropcheck(obj) == 0 {
        return;
    }
    let all = if ((*thing_o(obj)).o_type & 0x1) == 0 { TRUE } else { FALSE };
    let _ = leave_pack(obj, TRUE, all);
}

#[no_mangle]
pub unsafe extern "C" fn discovered() {}

#[no_mangle]
pub unsafe extern "C" fn print_disc(_type: c_char) {}

#[no_mangle]
pub unsafe extern "C" fn add_line(fmt: *mut c_char, arg: *mut c_char) -> c_char {
    if fmt.is_null() {
        return 0;
    }
    if !arg.is_null() {
        msg(fmt, arg);
    } else {
        msg(fmt);
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn end_line() {}

#[no_mangle]
pub unsafe extern "C" fn nothing(_type: c_char) -> *mut c_char {
    strcpy(prbuf.as_mut_ptr(), c"Nothing found".as_ptr());
    prbuf.as_mut_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn nameit(obj: *mut CThing, typ: *mut c_char, which: *mut c_char, op: *mut CObjInfo, prfunc: unsafe extern "C" fn(*mut CThing) -> *mut c_char) {
    if op.is_null() || obj.is_null() {
        return;
    }
    if ((*op).oi_know != 0) || !(*op).oi_guess.is_null() {
        let mut buf = prbuf.as_mut_ptr();
        if (*thing_o(obj)).o_count == 1 {
            sprintf(buf, c"A %s ".as_ptr(), typ);
        } else {
            sprintf(buf, c"%d %ss ".as_ptr(), (*thing_o(obj)).o_count, typ);
        }
        let tail = buf.add(strlen(buf));
        if (*op).oi_know != 0 {
            sprintf(tail, c"of %s%s(%s)".as_ptr(), (*op).oi_name, prfunc(obj), which);
        } else if !(*op).oi_guess.is_null() {
            sprintf(tail, c"called %s%s(%s)".as_ptr(), (*op).oi_guess, prfunc(obj), which);
        }
    } else if (*thing_o(obj)).o_count == 1 {
        sprintf(prbuf.as_mut_ptr(), c"A%s %s %s".as_ptr(), which, which, typ);
    } else {
        sprintf(prbuf.as_mut_ptr(), c"%d %s %ss".as_ptr(), (*thing_o(obj)).o_count, which, typ);
    }
}

#[no_mangle]
pub unsafe extern "C" fn nullstr(_: *mut CThing) -> *mut c_char {
    c"".as_ptr() as *mut c_char
}

#[no_mangle]
pub unsafe extern "C" fn pick_one_ex(info: *mut CObjInfo, nitems: c_int) -> c_int {
    pick_one(info, nitems)
}

#[no_mangle]
pub unsafe extern "C" fn set_order(order: *mut c_int, numthings: c_int) {
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

extern "C" {
    fn toupper(ch: c_int) -> c_int;
}
