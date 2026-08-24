use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint, c_void};

use crate::draw::place_at;
use crate::player::{CCoord, CPlace, CRoom, CThing, CThingMonster, CThingObject};

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

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

const ISDARK: c_short = 0o0000001;
const ISGONE: c_short = 0o0000002;
const ISHALU: c_short = 0o0004000;
const ISBLIND: c_short = 0o0000004;
const ISHASTE: c_short = 0o0000100;
const ISHUH: c_short = 0o0001000;
const ISINVIS: c_short = 0o0002000;
const ISRUN: c_short = 0o020000;
const SEEMONST: c_short = 0o040000;
const F_PASS: c_char = 0x80;
const MAXSTR: usize = 1024;
const MAXLINES: c_int = 24;
const MAXCOLS: c_int = 80;
const AMULETLEVEL: c_int = 26;
const HUNGERTIME: c_int = 1300;
const STOMACHSIZE: c_int = 2000;
const AFTER: c_int = 2;
const ESCAPE: c_int = 27;
const LEFT: c_int = 0;
const RIGHT: c_int = 1;
const NORM: c_int = 0;
const F_SEEN: c_uchar = 0x40;

#[repr(C)]
pub struct CObjInfo {
    pub oi_name: *mut c_char,
    pub oi_prob: c_int,
    pub oi_worth: c_int,
    pub oi_guess: *mut c_char,
    pub oi_know: c_uchar,
}

unsafe extern "C" {
    static mut after: c_uchar;
    static mut again: c_uchar;
    static mut amulet: c_uchar;
    static mut delta: CCoord;
    static mut dir_ch: c_char;
    static mut door_stop: c_uchar;
    static mut e_levels: [c_int; 21];
    static mut firstmove: c_uchar;
    static mut food_left: c_int;
    static mut fruit: [c_char; MAXSTR];
    static mut hungry_state: c_int;
    static mut jump: c_uchar;
    static mut last_dir: c_char;
    static mut level: c_int;
    static mut max_stats: crate::player::CStats;
    static mut mpos: c_int;
    static mut no_command: c_int;
    static mut no_move: c_int;
    static mut oldpos: CCoord;
    static mut oldrp: *mut CRoom;
    static mut passgo: c_uchar;
    static mut player: CThing;
    static mut prbuf: [c_char; MAXSTR];
    static mut runch: c_char;
    static mut running: c_uchar;
    static mut seenstairs: c_uchar;
    static mut see_floor: c_uchar;
    static mut stairs: CCoord;
    static mut stdscr: *mut c_void;
    static mut terse: c_uchar;
    static mut places: [CPlace; 32 * 80];
    static mut cur_armor: *mut CThing;
    static mut cur_ring: [*mut CThing; 2];
    static mut cur_weapon: *mut CThing;
    static mut lvl_obj: *mut CThing;
    static mut mlist: *mut CThing;

    fn addch(ch: c_uint) -> c_int;
    fn addmsg(fmt: *const c_char, ...);
    fn extinguish(func: *const c_void);
    fn free(ptr: *mut c_void);
    fn fuse(func: *const c_void, arg: c_int, time: c_int, typ: c_int);
    fn nohaste();
    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn get_str(s: *mut c_char, win: *mut c_void) -> c_int;
    fn inch() -> c_uint;
    fn isupper(c: c_int) -> c_int;
    fn leave_pack(obj: *mut CThing, newobj: c_uchar, all: c_uchar) -> *mut CThing;
    fn malloc(size: usize) -> *mut c_void;
    fn msg(fmt: *const c_char, ...);
    fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int;
    #[link_name = "move"]
    fn move_(y: c_int, x: c_int);
    fn readchar() -> c_int;
    fn reset_last();
    fn rnd(range: c_int) -> c_int;
    fn roll(num: c_int, sides: c_int) -> c_int;
    fn runto(cp: *mut CCoord);
    fn see_monst(mp: *mut CThing) -> c_uchar;
    fn step_ok(ch: c_int) -> c_int;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strlen(s: *const c_char) -> usize;
    fn tolower(c: c_int) -> c_int;
    fn wake_monster(y: c_int, x: c_int);
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
unsafe fn on(thing: *mut CThing, flag: c_short) -> bool {
    ((*thing_t(thing)).t_flags & flag) != 0
}

#[inline]
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_ch
}

#[inline]
unsafe fn room_flags(rp: *mut CRoom) -> c_short {
    if rp.is_null() { 0 } else { (*rp).r_flags }
}

#[inline]
unsafe fn hero_pos() -> CCoord {
    (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn first_is_vowel(s: *const c_char) -> bool {
    let bytes = CStr::from_ptr(s).to_bytes();
    if bytes.is_empty() {
        return false;
    }
    matches!(bytes[0], b'a' | b'A' | b'e' | b'E' | b'i' | b'I' | b'o' | b'O' | b'u' | b'U')
}

#[no_mangle]
pub unsafe extern "C" fn look(wakeup: c_uchar) {
    let mut x: c_int;
    let mut y: c_int;
    let mut ch: c_int;
    let mut tp: *mut CThing;
    let mut pp: *mut CPlace;
    let mut ey: c_int;
    let mut ex: c_int;
    let mut passcount: c_int = 0;
    let mut pfl: c_char;
    let mut pch: c_char;
    let mut sy: c_int;
    let mut sx: c_int;
    let mut sumhero: c_int = 0;
    let mut diffhero: c_int = 0;
    let hero = hero_pos();

    if !(oldpos.x == hero.x && oldpos.y == hero.y) {
        erase_lamp(&raw mut oldpos, oldrp);
        oldpos = hero;
        oldrp = (*thing_t(&raw mut player)).t_room;
    }

    ey = hero.y + 1;
    ex = hero.x + 1;
    sx = hero.x - 1;
    sy = hero.y - 1;
    if door_stop != 0 && firstmove == 0 && running != 0 {
        sumhero = hero.y + hero.x;
        diffhero = hero.y - hero.x;
    }

    pp = place_at((&raw mut places) as *mut CPlace, hero.y, hero.x);
    pch = (*pp).p_ch;
    pfl = (*pp).p_flags;

    for y in sy..=ey {
        if y <= 0 || y >= MAXLINES - 1 {
            continue;
        }
        for x in sx..=ex {
            if x < 0 || x >= MAXCOLS {
                continue;
            }
            if !on(&raw mut player, ISBLIND) && y == hero.y && x == hero.x {
                continue;
            }

            pp = place_at((&raw mut places) as *mut CPlace, y, x);
            ch = (*pp).p_ch as c_int;
            if ch == ' ' as c_int {
                continue;
            }

            let fp = &mut (*pp).p_flags;
            if pch != DOOR && ch != DOOR as c_int && (pfl as u8 & F_PASS as u8) != ((*fp as u8) & F_PASS as u8) {
                continue;
            }
            if (((*fp as u8) & F_PASS as u8) != 0 || ch == DOOR as c_int)
                && (((pfl as u8) & F_PASS as u8) != 0 || pch == DOOR)
            {
                if hero.x != x && hero.y != y
                    && step_ok(chat_at(y, hero.x) as c_int) == 0
                    && step_ok(chat_at(hero.y, x) as c_int) == 0
                {
                    continue;
                }
            }

            tp = (*pp).p_monst;
            if tp.is_null() {
                ch = trip_ch(y, x, ch);
            } else {
                if on(tp, SEEMONST) && on(tp, ISINVIS) {
                    if door_stop != 0 && firstmove == 0 {
                        running = FALSE;
                    }
                    continue;
                }
                if wakeup != 0 {
                    wake_monster(y, x);
                }
                if see_monst(tp) != 0 {
                    if on(&raw mut player, ISHALU) {
                        ch = rnd(26) + 'A' as c_int;
                    } else {
                        ch = (*thing_t(tp)).t_disguise as c_int;
                    }
                }
            }

            if on(&raw mut player, ISBLIND) && (y != hero.y || x != hero.x) {
                continue;
            }

            move_(y, x);
            let player_room = (*thing_t(&raw mut player)).t_room;
            if (room_flags(player_room) & (ISGONE as c_short | ISDARK as c_short)) == ISDARK && see_floor == 0 && ch == FLOOR as c_int {
                ch = ' ' as c_int;
            }

            let screen_ch = inch() as c_int & 0xFF;
            if tp.is_null() || ch != screen_ch {
                addch(ch as c_uint);
            }

            if door_stop != 0 && firstmove == 0 && running != 0 {
                if runch == b'h' && x == ex { continue; }
                if runch == b'j' && y == sy { continue; }
                if runch == b'k' && y == ey { continue; }
                if runch == b'l' && x == sx { continue; }
                if runch == b'y' && (y + x) - sumhero >= 1 { continue; }
                if runch == b'u' && (y - x) - diffhero >= 1 { continue; }
                if runch == b'n' && (y + x) - sumhero <= -1 { continue; }
                if runch == b'b' && (y - x) - diffhero <= -1 { continue; }

                if ch == DOOR as c_int {
                    if x == hero.x || y == hero.y {
                        running = FALSE;
                    }
                } else if ch == PASSAGE as c_int {
                    if x == hero.x || y == hero.y {
                        passcount += 1;
                    }
                } else if ch == FLOOR as c_int || ch == b'|' as c_int || ch == b'-' as c_int || ch == b' ' as c_int {
                } else {
                    running = FALSE;
                }
            }
        }
    }

    if door_stop != 0 && firstmove == 0 && passcount > 1 {
        running = FALSE;
    }
    if running == 0 || jump == 0 {
        mvaddch(hero.y, hero.x, PLAYER as c_uint);
    }
}

#[no_mangle]
pub unsafe extern "C" fn trip_ch(y: c_int, x: c_int, ch: c_int) -> c_int {
    if on(&raw mut player, ISHALU) && after != 0 {
        match ch as c_char {
            FLOOR | b' ' | PASSAGE | b'-' | b'|' | DOOR | TRAP => {}
            _ => {
                if !(y == stairs.y && x == stairs.x && seenstairs != 0) {
                    return rnd(26) as c_char as c_int;
                }
            }
        }
    }
    ch
}

#[no_mangle]
pub unsafe extern "C" fn erase_lamp(pos: *mut CCoord, rp: *mut CRoom) {
    let mut y: c_int;
    let mut x: c_int;
    let mut ey: c_int;
    let mut sy: c_int;
    let mut ex: c_int;

    if !((see_floor != 0) && ((room_flags(rp) & (ISGONE as c_short | ISDARK as c_short)) == ISDARK) && !on(&raw mut player, ISBLIND)) {
        return;
    }

    ey = (*pos).y + 1;
    ex = (*pos).x + 1;
    sy = (*pos).y - 1;
    for x in (*pos).x - 1..=ex {
        for y in sy..=ey {
            if y == hero_pos().y && x == hero_pos().x {
                continue;
            }
            move_(y, x);
            if inch() as c_char == FLOOR {
                addch(b' ' as c_uint);
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn show_floor() -> c_uchar {
    let player_room = (*thing_t(&raw mut player)).t_room;
    if (room_flags(player_room) & (ISGONE as c_short | ISDARK as c_short)) == ISDARK && !on(&raw mut player, ISBLIND) {
        return see_floor;
    }
    TRUE
}

#[no_mangle]
pub unsafe extern "C" fn find_obj(y: c_int, x: c_int) -> *mut CThing {
    let mut obj = lvl_obj;
    while !obj.is_null() {
        if (*thing_t(obj)).t_pos.y == y && (*thing_t(obj)).t_pos.x == x {
            return obj;
        }
        obj = (*thing_t(obj)).l_next;
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
            msg(c"ugh, you would get ill if you ate that".as_ptr());
        } else {
            msg(c"that's Inedible!".as_ptr());
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
    if obj == cur_weapon {
        cur_weapon = std::ptr::null_mut();
    }
    if (*thing_o(obj)).o_which == 1 {
        msg(c"my, that was a yummy %s".as_ptr(), fruit.as_ptr());
    } else if rnd(100) > 70 {
        (*thing_t(&raw mut player)).t_stats.s_exp += 1;
        msg(c"%s, this food tastes awful".as_ptr(), c"bummer".as_ptr());
    } else {
        msg(c"%s, that tasted good".as_ptr(), c"yum".as_ptr());
    }
    leave_pack(obj, FALSE, FALSE);
}

#[no_mangle]
pub unsafe extern "C" fn check_level() {
    let mut i: c_int = 0;
    let mut add: c_int;
    let mut olevel: c_int;

    while e_levels[i as usize] != 0 {
        if e_levels[i as usize] > (*thing_t(&raw mut player)).t_stats.s_exp {
            break;
        }
        i += 1;
    }
    i += 1;
    olevel = (*thing_t(&raw mut player)).t_stats.s_lvl;
    (*thing_t(&raw mut player)).t_stats.s_lvl = i;
    if i > olevel {
        add = roll(i - olevel, 10);
        (*thing_t(&raw mut player)).t_stats.s_maxhp += add;
        (*thing_t(&raw mut player)).t_stats.s_hpt += add;
        msg(c"welcome to level %d".as_ptr(), i);
    }
}

#[no_mangle]
pub unsafe extern "C" fn chg_str(amt: c_int) {
    let mut comp: c_uint;
    if amt == 0 {
        return;
    }
    let stats = &mut (*thing_t(&raw mut player)).t_stats;
    let mut new_strength = stats.s_str as c_int + amt;
    if new_strength < 3 {
        new_strength = 3;
    } else if new_strength > 31 {
        new_strength = 31;
    }
    stats.s_str = new_strength as c_uint;
    comp = stats.s_str;

    if cur_ring[LEFT as usize] != std::ptr::null_mut() {
        let ring = cur_ring[LEFT as usize];
        let bonus = (*thing_o(ring)).o_arm as c_int;
        let reduced = comp as c_int - bonus;
        comp = if reduced < 3 { 3 } else { reduced as c_uint };
    }
    if cur_ring[RIGHT as usize] != std::ptr::null_mut() {
        let ring = cur_ring[RIGHT as usize];
        let bonus = (*thing_o(ring)).o_arm as c_int;
        let reduced = comp as c_int - bonus;
        comp = if reduced < 3 { 3 } else { reduced as c_uint };
    }
    if comp > max_stats.s_str {
        max_stats.s_str = comp;
    }
}

#[no_mangle]
pub unsafe extern "C" fn add_str(sp: *mut c_uint, amt: c_int) {
    let newv = (*sp).wrapping_add(amt as c_uint);
    if newv < 3 { *sp = 3; }
    else if newv > 31 { *sp = 31; }
    else { *sp = newv; }
}

#[no_mangle]
pub unsafe extern "C" fn add_haste(potion: c_uchar) -> c_uchar {
    if on(&raw mut player, ISHASTE) {
        no_command += rnd(8);
        (*thing_t(&raw mut player)).t_flags &= !(ISRUN as c_short | ISHASTE as c_short) as c_short;
        extinguish(nohaste as *const c_void);
        msg(c"you faint from exhaustion".as_ptr());
        return FALSE;
    }

    (*thing_t(&raw mut player)).t_flags |= ISHASTE as c_short;
    if potion != 0 {
        fuse(nohaste as *const c_void, 0, rnd(4) + 4, AFTER);
    }
    TRUE
}

#[no_mangle]
pub unsafe extern "C" fn aggravate() {
    let mut mp = mlist;
    while !mp.is_null() {
        runto(&mut (*thing_t(mp)).t_pos);
        mp = (*thing_t(mp)).l_next;
    }
}

#[no_mangle]
pub unsafe extern "C" fn is_current(obj: *mut CThing) -> c_uchar {
    if obj.is_null() {
        return FALSE;
    }
    if obj == cur_armor || obj == cur_weapon || obj == cur_ring[LEFT as usize] || obj == cur_ring[RIGHT as usize] {
        if terse == 0 {
            addmsg(c"That's already ".as_ptr());
        }
        msg(c"in use".as_ptr());
        return TRUE;
    }
    FALSE
}

#[no_mangle]
pub unsafe extern "C" fn get_dir() -> c_uchar {
    let mut gotit: bool;
    let mut last_delt: CCoord = CCoord { x: 0, y: 0 };
    let prompt: *const c_char;

    if again != 0 && last_dir != 0 {
        delta.y = last_delt.y;
        delta.x = last_delt.x;
        dir_ch = last_dir;
    } else {
        if terse == 0 {
            msg(c"which direction? ".as_ptr());
        } else {
            // prompt is intentionally unused in the terse form.
        }
        loop {
            gotit = true;
            dir_ch = readchar() as c_char;
            match dir_ch {
                b'h' | b'H' => { delta.y = 0; delta.x = -1; }
                b'j' | b'J' => { delta.y = 1; delta.x = 0; }
                b'k' | b'K' => { delta.y = -1; delta.x = 0; }
                b'l' | b'L' => { delta.y = 0; delta.x = 1; }
                b'y' | b'Y' => { delta.y = -1; delta.x = -1; }
                b'u' | b'U' => { delta.y = -1; delta.x = 1; }
                b'b' | b'B' => { delta.y = 1; delta.x = -1; }
                b'n' | b'N' => { delta.y = 1; delta.x = 1; }
                c if c as c_int == ESCAPE => { last_dir = 0; reset_last(); return FALSE; }
                _ => {
                    mpos = 0;
                    msg(c"which direction? ".as_ptr());
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

    if on(&raw mut player, ISHUH) && rnd(5) == 0 {
        loop {
            delta.y = rnd(3) - 1;
            delta.x = rnd(3) - 1;
            if !(delta.y == 0 && delta.x == 0) {
                break;
            }
        }
    }
    mpos = 0;
    TRUE
}

#[no_mangle]
pub unsafe extern "C" fn sign(nm: c_int) -> c_int {
    if nm < 0 { -1 } else if nm > 0 { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "C" fn spread(nm: c_int) -> c_int {
    nm - nm / 20 + rnd(nm / 10)
}

#[no_mangle]
pub unsafe extern "C" fn call_it(info: *mut CObjInfo) {
    if (*info).oi_know != 0 {
        if !(*info).oi_guess.is_null() {
            free((*info).oi_guess as *mut c_void);
            (*info).oi_guess = std::ptr::null_mut();
        }
    } else if (*info).oi_guess.is_null() {
        if terse != 0 {
            msg(c"call it: ".as_ptr());
        } else {
            msg(c"what do you want to call it? ".as_ptr());
        }
        if get_str(prbuf.as_mut_ptr(), stdscr) == NORM {
            if !(*info).oi_guess.is_null() {
                free((*info).oi_guess as *mut c_void);
            }
            let len = strlen(prbuf.as_ptr()) + 1;
            let buf = malloc(len) as *mut c_char;
            if !buf.is_null() {
                strcpy(buf, prbuf.as_ptr());
                (*info).oi_guess = buf;
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rnd_thing() -> c_char {
    let mut thing_list = [POTION, SCROLL, RING, STICK, FOOD, WEAPON, ARMOR, STAIRS, GOLD, AMULET];
    let idx = if level >= AMULETLEVEL { rnd(thing_list.len() as c_int) } else { rnd((thing_list.len() - 1) as c_int) };
    thing_list[idx as usize]
}

#[no_mangle]
pub unsafe extern "C" fn choose_str(ts: *const c_char, ns: *const c_char) -> *mut c_char {
    if on(&raw mut player, ISHALU) { ts as *mut c_char } else { ns as *mut c_char }
}

#[no_mangle]
pub unsafe extern "C" fn vowelstr(str: *mut c_char) -> *mut c_char {
    if first_is_vowel(str) {
        c"n".as_ptr() as *mut c_char
    } else {
        c"".as_ptr() as *mut c_char
    }
}
