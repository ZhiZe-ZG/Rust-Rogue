//! Miscellaneous game routines.
//!
//! Ported from `src/c/misc.c` to Rust: gold, hunger, the floor map, and other
//! helpers shared across the game loop.
use crate::config::GameConfig;
use crate::globals::CObjInfo;
use crate::daemon::{extinguish, fuse};
use crate::daemons::nohaste;
use crate::entity::chase::runto;
use crate::game::EQUIPMENT;
use crate::item::pack::{get_item, leave_pack, reset_last};
use crate::options::get_str;
use crate::rnd::rnd;
use crate::ui::input::readchar;
use crate::ui::output::{addmsg_str, msg_str};
use glam::IVec2;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

use crate::game::MLIST;
use crate::entity::player::{Thing, ThingMonster, ThingObject, MonsterFlags};
use crate::startup::roll;

const PASSAGE: c_char = b'#' as c_char;
const DOOR: c_char = b'+' as c_char;
const FLOOR: c_char = b'.' as c_char;
const PLAYER: c_char = b'@' as c_char;
const TRAP: c_char = b'^' as c_char;
const STAIRS: c_char = b'%' as c_char;
const GOLD: c_char = b'*' as c_char;
const POTION: c_char = b'!' as c_char;
const SCROLL: c_char = b'?' as c_char;
const MAGIC: c_char = b'$' as c_char;
const FOOD: c_char = b':' as c_char;
const WEAPON: c_char = b')' as c_char;
const ARMOR: c_char = b']' as c_char;
const AMULET: c_char = b',' as c_char;
const RING: c_char = b'=' as c_char;
const STICK: c_char = b'/' as c_char;

const F_PASS: c_char = 0x80u8 as c_char;
const MAXSTR: usize = 1024;
const HUNGERTIME: c_int = 1300;
const STOMACHSIZE: c_int = 2000;
const AFTER: c_int = 2;
const ESCAPE: c_int = 27;
const NORM: c_int = 0;
const F_SEEN: c_uchar = 0x40;

unsafe extern "C" {
    static mut after: c_uchar;
    static mut again: c_uchar;
    static mut amulet: c_uchar;
    static mut delta: IVec2;
    static mut dir_ch: c_char;
    static mut door_stop: c_uchar;
    static mut e_levels: [c_int; 21];
    static mut firstmove: c_uchar;
    static mut food_left: c_int;
    static mut fruit: [c_char; MAXSTR];
    static mut hungry_state: c_int;
    static mut jump: c_uchar;
    static mut last_dir: c_char;
    static mut max_stats: crate::entity::player::Stats;
    static mut mpos: c_int;
    static mut no_command: c_int;
    static mut no_move: c_int;
    static mut oldpos: IVec2;
    static mut passgo: c_uchar;
    static mut prbuf: [c_char; MAXSTR];
    static mut runch: c_char;
    static mut running: c_uchar;
    static mut seenstairs: c_uchar;
    static mut see_floor: bool;
    static mut terse: c_uchar;

    fn isupper(c: c_int) -> c_int;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strlen(s: *const c_char) -> usize;
    fn tolower(c: c_int) -> c_int;
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
unsafe fn on(thing: *mut Thing, flag: MonsterFlags) -> bool {
    (*thing_t(thing)).t_flags.contains(flag)
}

#[inline]
unsafe fn hero_pos() -> IVec2 {
    (*thing_t(crate::game::player_ptr())).t_pos
}

#[inline]
unsafe fn first_is_vowel(s: *const c_char) -> bool {
    let bytes = CStr::from_ptr(s).to_bytes();
    if bytes.is_empty() {
        return false;
    }
    matches!(
        bytes[0],
        b'a' | b'A' | b'e' | b'E' | b'i' | b'I' | b'o' | b'O' | b'u' | b'U'
    )
}

/// show_floor:
/// Returns whether the floor of the player's room should be displayed.
#[no_mangle]
pub unsafe fn show_floor() -> bool {
    let player_room = (*thing_t(crate::game::player_ptr())).t_room;
    if crate::game::room_dark(player_room)
        && !crate::game::room_gone(player_room)
        && !on(crate::game::player_ptr(), MonsterFlags::BLIND)
    {
        return see_floor;
    }
    true
}

#[no_mangle]
pub unsafe extern "C" fn find_obj(y: c_int, x: c_int) -> *mut Thing {
    let mut obj = crate::game::with_current_level(|level| level.items.head());
    while !obj.is_null() {
        if (*thing_o(obj)).o_pos.y == y && (*thing_o(obj)).o_pos.x == x {
            return obj;
        }
        obj = crate::entity::player::thing_next(obj);
    }
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn eat() {
    let obj = get_item(c"eat".as_ptr(), FOOD as c_int);
    if obj.is_null() {
        return;
    }
    if (*thing_o(obj)).o_type != FOOD as c_int {
        if terse == 0 {
            msg_str("ugh, you would get ill if you ate that");
        } else {
            msg_str("that's Inedible!");
        }
        return;
    }

    if food_left < 0 {
        food_left = 0;
    }
    food_left += HUNGERTIME - 200 + rnd(400);
    if food_left > STOMACHSIZE {
        food_left = STOMACHSIZE;
    }
    hungry_state = 0;
    if obj == EQUIPMENT.weapon() {
        EQUIPMENT.set_weapon(std::ptr::null_mut());
    }
    if (*thing_o(obj)).o_which == 1 {
        msg_str(&format!(
            "my, that was a yummy {}",
            CStr::from_ptr(fruit.as_ptr()).to_string_lossy()
        ));
    } else if rnd(100) > 70 {
        (*thing_t(crate::game::player_ptr())).t_stats.experience += 1;
        msg_str("bummer, this food tastes awful");
    } else {
        msg_str("yum, that tasted good");
    }
    leave_pack(obj, false as c_uchar, false as c_uchar);
}

#[no_mangle]
pub unsafe extern "C" fn check_level() {
    let mut i: c_int = 0;
    while e_levels[i as usize] != 0 {
        if e_levels[i as usize] > (*thing_t(crate::game::player_ptr())).t_stats.experience {
            break;
        }
        i += 1;
    }
    i += 1;
    let olevel = (*thing_t(crate::game::player_ptr())).t_stats.level;
    (*thing_t(crate::game::player_ptr())).t_stats.level = i;
    if i > olevel {
        let add = roll(i - olevel, 10);
        (*thing_t(crate::game::player_ptr())).t_stats.max_hit_points += add;
        (*thing_t(crate::game::player_ptr())).t_stats.hit_points += add;
        msg_str(&format!("welcome to level {}", i));
    }
}

#[no_mangle]
pub unsafe extern "C" fn chg_str(amt: c_int) {
    if amt == 0 {
        return;
    }
    let stats = &mut (*thing_t(crate::game::player_ptr())).t_stats;
    let mut new_strength = stats.strength as c_int + amt;
    if new_strength < 3 {
        new_strength = 3;
    } else if new_strength > 31 {
        new_strength = 31;
    }
    stats.strength = new_strength as c_uint;
    let mut comp = stats.strength;

    if !EQUIPMENT.left_ring().is_null() {
        let ring = EQUIPMENT.left_ring();
        let bonus = (*thing_o(ring)).o_arm as c_int;
        let reduced = comp as c_int - bonus;
        comp = if reduced < 3 { 3 } else { reduced as c_uint };
    }
    if !EQUIPMENT.right_ring().is_null() {
        let ring = EQUIPMENT.right_ring();
        let bonus = (*thing_o(ring)).o_arm as c_int;
        let reduced = comp as c_int - bonus;
        comp = if reduced < 3 { 3 } else { reduced as c_uint };
    }
    if comp > max_stats.strength {
        max_stats.strength = comp;
    }
}

#[no_mangle]
pub unsafe extern "C" fn add_str(sp: *mut c_uint, amt: c_int) {
    let newv = (*sp).wrapping_add(amt as c_uint);
    if newv < 3 {
        *sp = 3;
    } else if newv > 31 {
        *sp = 31;
    } else {
        *sp = newv;
    }
}

#[no_mangle]
pub unsafe fn add_haste(potion: bool) -> bool {
    if on(crate::game::player_ptr(), MonsterFlags::HASTE) {
        no_command += rnd(8);
        (*thing_t(crate::game::player_ptr()))
            .t_flags
            .remove(MonsterFlags::RUN | MonsterFlags::HASTE);
        extinguish(nohaste as *const c_void);
        msg_str("you faint from exhaustion");
        return false;
    }

    (*thing_t(crate::game::player_ptr()))
        .t_flags
        .insert(MonsterFlags::HASTE);
    if potion {
        fuse(nohaste as *const c_void, 0, rnd(4) + 4, AFTER);
    }
    true
}

#[no_mangle]
pub unsafe extern "C" fn aggravate() {
    let mut mp = MLIST.head();
    while !mp.is_null() {
        runto(&mut (*thing_t(mp)).t_pos);
        mp = crate::entity::player::thing_next(mp);
    }
}

#[no_mangle]
pub unsafe fn is_current(obj: *mut Thing) -> bool {
    if obj.is_null() {
        return false;
    }
    if obj == EQUIPMENT.armor()
        || obj == EQUIPMENT.weapon()
        || obj == EQUIPMENT.left_ring()
        || obj == EQUIPMENT.right_ring()
    {
        if terse == 0 {
            addmsg_str("That's already ");
        }
        msg_str("in use");
        return true;
    }
    false
}

#[no_mangle]
pub unsafe extern "C" fn get_dir() -> c_uchar {
    let mut gotit: bool;
    let mut last_delt: IVec2 = IVec2 { x: 0, y: 0 };

    if again != 0 && last_dir != 0 {
        delta.y = last_delt.y;
        delta.x = last_delt.x;
        dir_ch = last_dir;
    } else {
        if terse == 0 {
            msg_str("which direction? ");
        }
        loop {
            gotit = true;
            dir_ch = readchar() as c_char;
            match dir_ch as u8 {
                b'h' | b'H' => {
                    delta.y = 0;
                    delta.x = -1;
                }
                b'j' | b'J' => {
                    delta.y = 1;
                    delta.x = 0;
                }
                b'k' | b'K' => {
                    delta.y = -1;
                    delta.x = 0;
                }
                b'l' | b'L' => {
                    delta.y = 0;
                    delta.x = 1;
                }
                b'y' | b'Y' => {
                    delta.y = -1;
                    delta.x = -1;
                }
                b'u' | b'U' => {
                    delta.y = -1;
                    delta.x = 1;
                }
                b'b' | b'B' => {
                    delta.y = 1;
                    delta.x = -1;
                }
                b'n' | b'N' => {
                    delta.y = 1;
                    delta.x = 1;
                }
                c if c as c_int == ESCAPE => {
                    last_dir = 0;
                    reset_last();
                    return false as c_uchar;
                }
                _ => {
                    mpos = 0;
                    msg_str("which direction? ");
                    gotit = false;
                }
            }
            if gotit {
                break;
            }
        }
        if isupper(dir_ch as c_int) != 0 {
            dir_ch = tolower(dir_ch as c_int) as c_char;
        }
        last_dir = dir_ch;
        last_delt.y = delta.y;
        last_delt.x = delta.x;
    }

    if on(crate::game::player_ptr(), MonsterFlags::HUH) && rnd(5) == 0 {
        loop {
            delta.y = rnd(3) - 1;
            delta.x = rnd(3) - 1;
            if !(delta.y == 0 && delta.x == 0) {
                break;
            }
        }
    }
    mpos = 0;
    true as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn sign(nm: c_int) -> c_int {
    if nm < 0 {
        -1
    } else if nm > 0 {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn spread(nm: c_int) -> c_int {
    nm - nm / 20 + rnd(nm / 10)
}

#[no_mangle]
pub unsafe extern "C" fn call_it(info: &mut CObjInfo) {
    if info.oi_know {
        info.oi_guess = None;
    } else if info.oi_guess.is_none() {
        if terse != 0 {
            msg_str("call it: ");
        } else {
            msg_str("what do you want to call it? ");
        }
        if get_str(prbuf.as_mut_ptr().cast(), crate::ui::Window::Stdscr) == NORM {
            let text = CStr::from_ptr(prbuf.as_ptr()).to_string_lossy().into_owned();
            info.oi_guess = Some(text);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rnd_thing() -> c_char {
    let thing_list = [
        POTION, SCROLL, RING, STICK, FOOD, WEAPON, ARMOR, STAIRS, GOLD, AMULET,
    ];
    let idx = if crate::game::current_depth() >= GameConfig::AMULET_LEVEL {
        rnd(thing_list.len() as c_int)
    } else {
        rnd((thing_list.len() - 1) as c_int)
    };
    thing_list[idx as usize]
}

#[no_mangle]
pub unsafe extern "C" fn choose_str(ts: *const c_char, ns: *const c_char) -> *mut c_char {
    if on(crate::game::player_ptr(), MonsterFlags::HALU) {
        ts as *mut c_char
    } else {
        ns as *mut c_char
    }
}

unsafe fn vowelstr(str: *mut c_char) -> *mut c_char {
    if first_is_vowel(str) {
        c"n".as_ptr() as *mut c_char
    } else {
        c"".as_ptr() as *mut c_char
    }
}
