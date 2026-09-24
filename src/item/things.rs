//! Object information tables and object naming/inventory helpers.
//!
//! Ported from `src/c/things.c` to Rust.
use crate::daemon::extinguish;
use crate::daemons::unsee;
use crate::game::EQUIPMENT;
use crate::item::armor::waste_time;
use crate::item::pack::{get_item, leave_pack};
use crate::misc::chg_str;
use crate::rnd::rnd;
use crate::ui::output::msg_str;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

use crate::entity::player::{CThing, CThingObject};
use crate::globals::{
    arm_info, pot_info, ring_info, scr_info, things, weap_info, ws_info, CObjInfo,
};
use crate::item::rings::RingType;
use crate::item::sticks::fix_stick;
use crate::item::thing_list::new_item;
use crate::item::weapons::init_weapon;

const MAXSTR: usize = 1024;
const NUMTHINGS: usize = 7;
const MAXARMORS: usize = 8;
const MAXPOTIONS: usize = 14;
const MAXRINGS: usize = RingType::COUNT;
const MAXSCROLLS: usize = 18;
const MAXWEAPONS: usize = 9;
const MAXSTICKS: usize = 14;

const POTION: c_int = b'!' as c_int;
const SCROLL: c_int = b'?' as c_int;
const FOOD: c_int = b':' as c_int;
const WEAPON: c_int = b')' as c_int;
const ARMOR: c_int = b']' as c_int;
const RING: c_int = b'=' as c_int;
const STICK: c_int = b'/' as c_int;
const GOLD: c_int = b'*' as c_int;
const AMULET: c_int = b',' as c_int;

const ISCURSED: c_int = 0o000001;
const ISKNOW: c_int = 0o000200;

unsafe extern "C" {
    static mut after: c_uchar;
    static mut a_class: [c_int; 26];
    static mut amulet: c_uchar;
    static mut fruit: [c_char; MAXSTR];
    static mut inv_describe: c_uchar;
    static mut no_food: c_int;
    static mut player: CThing;
    static mut prbuf: [c_char; MAXSTR];
    static mut terse: c_uchar;

    fn isupper(ch: c_int) -> c_int;
    fn sprintf(buf: *mut c_char, fmt: *const c_char, ...) -> c_int;
    fn strcat(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strlen(s: *const c_char) -> usize;
    fn tolower(ch: c_int) -> c_int;
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn is_vowel(ch: c_char) -> bool {
    matches!(
        ch as u8,
        b'a' | b'A' | b'e' | b'E' | b'i' | b'I' | b'o' | b'O' | b'u' | b'U'
    )
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
unsafe fn item_name(typ: c_int, which: c_int) -> *const c_char {
    match typ {
        POTION => pot_info[which as usize].oi_name.as_ptr().cast(),
        SCROLL => scr_info[which as usize].oi_name.as_ptr().cast(),
        RING => ring_info[which as usize].oi_name.as_ptr().cast(),
        STICK => ws_info[which as usize].oi_name.as_ptr().cast(),
        WEAPON => weap_info[which as usize].oi_name.as_ptr().cast(),
        ARMOR => arm_info[which as usize].oi_name.as_ptr().cast(),
        FOOD => c"food".as_ptr(),
        GOLD => c"gold".as_ptr(),
        AMULET => c"the Amulet of Yendor".as_ptr(),
        _ => c"item".as_ptr(),
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
                sprintf(empty, c"A %s".as_ptr(), pot_info[which as usize].oi_name.as_ptr().cast::<c_char>());
            } else {
                sprintf(
                    empty,
                    c"%d %ss".as_ptr(),
                    count,
                    pot_info[which as usize].oi_name.as_ptr().cast::<c_char>(),
                );
            }
        }
        RING => {
            if count == 1 {
                sprintf(
                    empty,
                    c"A %s ring".as_ptr(),
                    ring_info[which as usize].oi_name.as_ptr().cast::<c_char>(),
                );
            } else {
                sprintf(
                    empty,
                    c"%d %s rings".as_ptr(),
                    count,
                    ring_info[which as usize].oi_name.as_ptr().cast::<c_char>(),
                );
            }
        }
        STICK => {
            if count == 1 {
                sprintf(empty, c"A %s".as_ptr(), ws_info[which as usize].oi_name.as_ptr().cast::<c_char>());
            } else {
                sprintf(
                    empty,
                    c"%d %ss".as_ptr(),
                    count,
                    ws_info[which as usize].oi_name.as_ptr().cast::<c_char>(),
                );
            }
        }
        SCROLL => {
            if count == 1 {
                sprintf(
                    empty,
                    c"A scroll of %s".as_ptr(),
                    scr_info[which as usize].oi_name.as_ptr().cast::<c_char>(),
                );
            } else {
                sprintf(
                    empty,
                    c"%d scrolls of %s".as_ptr(),
                    count,
                    scr_info[which as usize].oi_name.as_ptr().cast::<c_char>(),
                );
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
            let name = weap_info[which as usize].oi_name.as_ptr().cast::<c_char>();
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
            let name = arm_info[which as usize].oi_name.as_ptr().cast::<c_char>();
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
        if obj == EQUIPMENT.armor() {
            strcat(empty, c" (being worn)".as_ptr());
        }
        if obj == EQUIPMENT.weapon() {
            strcat(empty, c" (weapon in hand)".as_ptr());
        }
        if obj == EQUIPMENT.left_ring() {
            strcat(empty, c" (on left hand)".as_ptr());
        } else if obj == EQUIPMENT.right_ring() {
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
        return true as c_uchar;
    }
    if obj != EQUIPMENT.armor()
        && obj != EQUIPMENT.weapon()
        && obj != EQUIPMENT.left_ring()
        && obj != EQUIPMENT.right_ring()
    {
        return true as c_uchar;
    }
    if ((*thing_o(obj)).o_flags & ISCURSED) != 0 {
        msg_str("you can't.  It appears to be cursed");
        return false as c_uchar;
    }
    if obj == EQUIPMENT.weapon() {
        EQUIPMENT.set_weapon(std::ptr::null_mut());
    } else if obj == EQUIPMENT.armor() {
        waste_time();
        EQUIPMENT.set_armor(std::ptr::null_mut());
    } else {
        if obj == EQUIPMENT.left_ring() {
            EQUIPMENT.set_left_ring(std::ptr::null_mut());
        } else {
            EQUIPMENT.set_right_ring(std::ptr::null_mut());
        }
        match (*thing_o(obj)).o_which {
            0 => chg_str(-(*thing_o(obj)).o_arm),
            _ => {}
        }
    }
    true as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn new_thing() -> *mut CThing {
    let cur = new_item();
    (*thing_o(cur)).o_hplus = 0;
    (*thing_o(cur)).o_dplus = 0;
    std::ptr::copy_nonoverlapping(c"0x0".as_ptr().cast::<u8>(), (*thing_o(cur)).o_damage.as_mut_ptr(), 4);
    std::ptr::copy_nonoverlapping(c"0x0".as_ptr().cast::<u8>(), (*thing_o(cur)).o_hurldmg.as_mut_ptr(), 4);
    (*thing_o(cur)).o_arm = 11;
    (*thing_o(cur)).o_count = 1;
    (*thing_o(cur)).o_group = 0;
    (*thing_o(cur)).o_flags = 0;

    let choice = if no_food > 3 {
        2
    } else {
        pick_one(things.as_ptr() as *mut CObjInfo, NUMTHINGS as c_int) as c_int
    };
    match choice {
        0 => {
            (*thing_o(cur)).o_type = POTION;
            (*thing_o(cur)).o_which =
                pick_one(pot_info.as_ptr() as *mut CObjInfo, MAXPOTIONS as c_int);
        }
        1 => {
            (*thing_o(cur)).o_type = SCROLL;
            (*thing_o(cur)).o_which =
                pick_one(scr_info.as_ptr() as *mut CObjInfo, MAXSCROLLS as c_int);
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
                pick_one(weap_info.as_ptr() as *mut CObjInfo, MAXWEAPONS as c_int),
            );
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
            (*thing_o(cur)).o_which =
                pick_one(arm_info.as_ptr() as *mut CObjInfo, MAXARMORS as c_int);
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
            let ring_type = RingType::from_raw(pick_one(
                ring_info.as_ptr() as *mut CObjInfo,
                MAXRINGS as c_int,
            ))
            .expect("ring metadata produced an invalid ring type");
            (*thing_o(cur)).o_which = ring_type as c_int;
            match ring_type {
                RingType::Protection
                | RingType::SustainStrength
                | RingType::AddHit
                | RingType::AddDamage => {
                    let mut arm = rnd(3);
                    if arm == 0 {
                        arm = -1;
                        (*thing_o(cur)).o_flags |= ISCURSED;
                    }
                    (*thing_o(cur)).o_arm = arm;
                }
                RingType::Adornment | RingType::Aggravate => {
                    (*thing_o(cur)).o_flags |= ISCURSED;
                }
                _ => {}
            }
        }
        6 => {
            (*thing_o(cur)).o_type = STICK;
            (*thing_o(cur)).o_which =
                pick_one(ws_info.as_ptr() as *mut CObjInfo, MAXSTICKS as c_int);
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
    let all = if ((*thing_o(obj)).o_type & 0x1) == 0 {
        true as c_uchar
    } else {
        false as c_uchar
    };
    let _ = leave_pack(obj, true as c_uchar, all);
}

#[no_mangle]
pub unsafe extern "C" fn discovered() {}

unsafe fn print_disc(_type: c_char) {}

#[no_mangle]
pub unsafe extern "C" fn add_line(fmt: *mut c_char, arg: *mut c_char) -> c_char {
    if fmt.is_null() {
        return 0;
    }
    // The caller passes C-style format strings (e.g. `%s` or `a) %s`)
    // together with a single string argument. Substitute the lone string
    // argument for the `%s` conversion, then hand the finished text to
    // `rogue_msg_str` directly, bypassing the legacy variadic `msg()` shim.
    let fmt_str = CStr::from_ptr(fmt).to_string_lossy();
    let text = if arg.is_null() {
        fmt_str.to_string()
    } else {
        let arg_str = CStr::from_ptr(arg).to_string_lossy();
        match fmt_str.find("%s") {
            Some(idx) => format!("{}{}{}", &fmt_str[..idx], arg_str, &fmt_str[idx + 2..]),
            None => fmt_str.to_string(),
        }
    };
    msg_str(&text);
    0
}

unsafe fn end_line() {}

unsafe fn nothing(_type: c_char) -> *mut c_char {
    strcpy(prbuf.as_mut_ptr(), c"Nothing found".as_ptr());
    prbuf.as_mut_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn nameit(
    obj: *mut CThing,
    typ: *mut c_char,
    which: *mut c_char,
    op: *mut CObjInfo,
    prfunc: unsafe extern "C" fn(*mut CThing) -> *mut c_char,
) {
    if op.is_null() || obj.is_null() {
        return;
    }
    if (*op).oi_know || (*op).oi_guess.is_some() {
        let mut buf = prbuf.as_mut_ptr();
        if (*thing_o(obj)).o_count == 1 {
            sprintf(buf, c"A %s ".as_ptr(), typ);
        } else {
            sprintf(buf, c"%d %ss ".as_ptr(), (*thing_o(obj)).o_count, typ);
        }
        let tail = buf.add(strlen(buf));
        if (*op).oi_know {
            sprintf(
                tail,
                c"of %s%s(%s)".as_ptr(),
                (*op).oi_name.as_ptr().cast::<c_char>(),
                prfunc(obj),
                which,
            );
        } else if let Some(guess) = &(*op).oi_guess {
            sprintf(
                tail,
                c"called %s%s(%s)".as_ptr(),
                guess.as_ptr().cast::<c_char>(),
                prfunc(obj),
                which,
            );
        }
    } else if (*thing_o(obj)).o_count == 1 {
        sprintf(prbuf.as_mut_ptr(), c"A%s %s %s".as_ptr(), which, which, typ);
    } else {
        sprintf(
            prbuf.as_mut_ptr(),
            c"%d %s %ss".as_ptr(),
            (*thing_o(obj)).o_count,
            which,
            typ,
        );
    }
}

unsafe fn nullstr(_: *mut CThing) -> *mut c_char {
    c"".as_ptr() as *mut c_char
}

unsafe fn pick_one_ex(info: *mut CObjInfo, nitems: c_int) -> c_int {
    pick_one(info, nitems)
}

unsafe fn set_order(order: *mut c_int, numthings: c_int) {
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
