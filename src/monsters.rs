use crate::rnd::rnd;
use crate::curses as cur;
use crate::io::{addmsg_str, msg_str};
use crate::player::{CCoord, CRoom, CStats, CThing, CThingMonster, CThingObject};
use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

const AMULETLEVEL: c_int = 26;
const LAMPDIST: c_int = 3;
const HUHDURATION: c_int = 20;
const AFTER: c_int = 2;
const VS_MAGIC: c_int = 0o03;

const ISDARK: c_short = 0o000001;
const ISBLIND: c_short = 0o000004;
const ISCANC: c_short = 0o000010;
const ISLEVIT: c_short = 0o000010;
const ISFOUND: c_short = 0o000020;
const ISGREED: c_short = 0o000040;
const ISHASTE: c_short = 0o000100;
const ISHELD: c_short = 0o000400;
const ISHUH: c_short = 0o001000;
const ISMEAN: c_short = 0o004000;
const ISHALU: c_short = 0o004000;
const ISRUN: c_short = 0o020000;
const SEEMONST: c_short = 0o040000;

const LEFT: usize = 0;
const RIGHT: usize = 1;
const R_AGGR: c_int = 6;
const R_STEALTH: c_int = 12;
const R_PROTECT: c_int = 0;

const TRUE: c_uchar = 1;

/// Layout mirror of the C `struct monster` stat table, tied to the `monsters[]`
/// global the C engine exposes.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CMonster {
    pub m_name: *mut c_char,
    pub m_carry: c_int,
    pub m_flags: c_short,
    pub m_stats: CStats,
}

static LVL_MONS: [c_char; 26] = [
    b'K' as c_char,
    b'E' as c_char,
    b'B' as c_char,
    b'S' as c_char,
    b'H' as c_char,
    b'I' as c_char,
    b'R' as c_char,
    b'O' as c_char,
    b'Z' as c_char,
    b'L' as c_char,
    b'C' as c_char,
    b'Q' as c_char,
    b'A' as c_char,
    b'N' as c_char,
    b'Y' as c_char,
    b'F' as c_char,
    b'T' as c_char,
    b'W' as c_char,
    b'P' as c_char,
    b'X' as c_char,
    b'U' as c_char,
    b'M' as c_char,
    b'V' as c_char,
    b'G' as c_char,
    b'J' as c_char,
    b'D' as c_char,
];

static WAND_MONS: [c_char; 26] = [
    b'K' as c_char,
    b'E' as c_char,
    b'B' as c_char,
    b'S' as c_char,
    b'H' as c_char,
    0,
    b'R' as c_char,
    b'O' as c_char,
    b'Z' as c_char,
    0,
    b'C' as c_char,
    b'Q' as c_char,
    b'A' as c_char,
    0,
    b'Y' as c_char,
    0,
    b'T' as c_char,
    b'W' as c_char,
    b'P' as c_char,
    0,
    b'U' as c_char,
    b'M' as c_char,
    b'V' as c_char,
    b'G' as c_char,
    b'J' as c_char,
    0,
];

unsafe extern "C" {
    static mut level: c_int;
    static mut max_level: c_int;
    static mut mlist: *mut CThing;
    static mut monsters: [CMonster; 26];
    static mut player: CThing;
    static mut cur_ring: [*mut CThing; 2];
    static mut wizard: c_int;

    fn _attach(list: *mut *mut CThing, item: *mut CThing);
    fn roomin(cp: *mut CCoord) -> *mut CRoom;
    fn roll(number: c_int, sides: c_int) -> c_int;
    fn runto(cp: *mut CCoord);
    fn rnd_thing() -> c_char;
    fn new_item() -> *mut CThing;
    fn new_thing() -> *mut CThing;
    fn find_floor(rp: *mut CRoom, cp: *mut CCoord, limit: c_uchar, monst: c_uchar) -> c_uchar;
    fn dist(y1: c_int, x1: c_int, y2: c_int, x2: c_int) -> c_int;
    fn lengthen(func: *const c_void, xtime: c_int);
    fn fuse(func: *const c_void, arg: c_int, time: c_int, typ: c_int);
    fn unconfuse();
    fn spread(nm: c_int) -> c_int;
    fn set_mname(tp: *mut CThing) -> *mut c_char;
    fn strcmp(a: *const c_char, b: *const c_char) -> c_int;
    fn abort() -> !;
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
unsafe fn player_t() -> *mut CThingMonster {
    (&raw mut player) as *mut CThing as *mut CThingMonster
}

#[inline]
unsafe fn has_flag(tp: *mut CThing, flag: c_short) -> bool {
    ((*thing_t(tp)).t_flags & flag) != 0
}

#[inline]
unsafe fn player_has(flag: c_short) -> bool {
    ((*player_t()).t_flags & flag) != 0
}

#[inline]
unsafe fn iswearing(which: c_int) -> bool {
    (!cur_ring[LEFT].is_null() && (*thing_o(cur_ring[LEFT])).o_which == which)
        || (!cur_ring[RIGHT].is_null() && (*thing_o(cur_ring[RIGHT])).o_which == which)
}

/// Picks an appropriate monster glyph for the current depth.
#[no_mangle]
pub unsafe extern "C" fn randmonster(wander: c_uchar) -> c_char {
    let mons = if wander != 0 { &WAND_MONS } else { &LVL_MONS };
    loop {
        let mut d = level + (rnd(10) - 6);
        if d < 0 {
            d = rnd(5);
        }
        if d > 25 {
            d = rnd(5) + 21;
        }
        let m = mons[d as usize];
        if m != 0 {
            return m;
        }
    }
}

/// Initializes a freshly allocated monster thing and places it on the map.
#[no_mangle]
pub unsafe extern "C" fn new_monster(tp: *mut CThing, monster_type: c_char, cp: *mut CCoord) {
    let mut lev_add = level - AMULETLEVEL;
    if lev_add < 0 {
        lev_add = 0;
    }

    _attach(&raw mut mlist, tp);

    (*thing_t(tp)).t_type = monster_type;
    (*thing_t(tp)).t_disguise = monster_type;
    (*thing_t(tp)).t_pos = *cp;

    (*thing_t(tp)).t_oldch = crate::draw::chat_at((*cp).y, (*cp).x);
    (*thing_t(tp)).t_room = roomin(cp);
    // Keep both occupancy structures in sync (`MONSTERS` and `places`).
    crate::game::set_monster((*cp).y, (*cp).x, tp);

    let mp = &monsters[(monster_type as i32 - 'A' as i32) as usize];
    (*thing_t(tp)).t_stats.s_lvl = mp.m_stats.s_lvl + lev_add;
    (*thing_t(tp)).t_stats.s_maxhp = roll((*thing_t(tp)).t_stats.s_lvl, 8);
    (*thing_t(tp)).t_stats.s_hpt = (*thing_t(tp)).t_stats.s_maxhp;
    (*thing_t(tp)).t_stats.s_arm = mp.m_stats.s_arm - lev_add;
    (*thing_t(tp)).t_stats.s_dmg = mp.m_stats.s_dmg;
    (*thing_t(tp)).t_stats.s_str = mp.m_stats.s_str;
    (*thing_t(tp)).t_stats.s_exp = mp.m_stats.s_exp + lev_add * 10 + exp_add(tp);
    (*thing_t(tp)).t_flags = mp.m_flags;
    if level > 29 {
        (*thing_t(tp)).t_flags |= ISHASTE;
    }
    (*thing_t(tp)).t_turn = TRUE;
    (*thing_t(tp)).t_pack = std::ptr::null_mut();

    if iswearing(R_AGGR) {
        runto(cp);
    }
    if monster_type == 'X' as c_char {
        (*thing_t(tp)).t_disguise = rnd_thing();
    }
}

/// Computes bonus experience from a monster's level and max HP.
#[no_mangle]
pub unsafe extern "C" fn exp_add(tp: *mut CThing) -> c_int {
    let mut modu = if (*thing_t(tp)).t_stats.s_lvl == 1 {
        (*thing_t(tp)).t_stats.s_maxhp / 8
    } else {
        (*thing_t(tp)).t_stats.s_maxhp / 6
    };

    if (*thing_t(tp)).t_stats.s_lvl > 9 {
        modu *= 20;
    } else if (*thing_t(tp)).t_stats.s_lvl > 6 {
        modu *= 4;
    }
    modu
}

/// Spawns a wandering monster in a different room and sets it running toward the hero.
#[no_mangle]
pub unsafe extern "C" fn wanderer() {
    let tp = new_item();
    let mut cp = CCoord { x: 0, y: 0 };

    loop {
        let _ = find_floor(std::ptr::null_mut(), &mut cp, 0, 1);
        if roomin(&mut cp) != (*player_t()).t_room {
            break;
        }
    }

    new_monster(tp, randmonster(1), &mut cp);

    if player_has(SEEMONST) {
        cur::standout();
        if !player_has(ISHALU) {
            cur::addch((*thing_t(tp)).t_type as c_uint);
        } else {
            cur::addch((rnd(26) + 'A' as c_int) as c_uint);
        }
        cur::standend();
    }

    runto(&mut (*thing_t(tp)).t_pos);

    if wizard != 0 {
        msg_str(&format!(
            "started a wandering {}",
            CStr::from_ptr(monsters[((*thing_t(tp)).t_type as i32 - 'A' as i32) as usize].m_name).to_string_lossy()
        ));
    }
}

/// Wakes and updates an adjacent monster's pursuit behavior and special gaze logic.
#[no_mangle]
pub unsafe extern "C" fn wake_monster(y: c_int, x: c_int) -> *mut CThing {
    let tp = crate::game::monster_at(y, x);
    if tp.is_null() {
        cur::endwin();
        abort();
    }

    let ch = (*thing_t(tp)).t_type;

    if !has_flag(tp, ISRUN)
        && rnd(3) != 0
        && has_flag(tp, ISMEAN)
        && !has_flag(tp, ISHELD)
        && !iswearing(R_STEALTH)
        && !player_has(ISLEVIT)
    {
        (*thing_t(tp)).t_dest = &mut (*player_t()).t_pos;
        (*thing_t(tp)).t_flags |= ISRUN;
    }

    if ch == 'M' as c_char
        && !player_has(ISBLIND)
        && !player_has(ISHALU)
        && !has_flag(tp, ISFOUND)
        && !has_flag(tp, ISCANC)
        && has_flag(tp, ISRUN)
    {
        let rp = (*player_t()).t_room;
        if (!rp.is_null() && ((*rp).r_flags & ISDARK) == 0)
            || dist(y, x, (*player_t()).t_pos.y, (*player_t()).t_pos.x) < LAMPDIST
        {
            (*thing_t(tp)).t_flags |= ISFOUND;
            if save(VS_MAGIC) == 0 {
                if player_has(ISHUH) {
                    lengthen(unconfuse as *const c_void, spread(HUHDURATION));
                } else {
                    fuse(unconfuse as *const c_void, 0, spread(HUHDURATION), AFTER);
                }
                (*player_t()).t_flags |= ISHUH;
                let mname = set_mname(tp);
                addmsg_str(&CStr::from_ptr(mname).to_string_lossy());
                if strcmp(mname, c"it".as_ptr()) != 0 {
                    addmsg_str("'");
                }
                msg_str("s gaze has confused you");
            }
        }
    }

    if has_flag(tp, ISGREED) && !has_flag(tp, ISRUN) {
        (*thing_t(tp)).t_flags |= ISRUN;
        let pr = (*player_t()).t_room;
        if !pr.is_null() && (*pr).r_goldval != 0 {
            (*thing_t(tp)).t_dest = &mut (*pr).r_gold;
        } else {
            (*thing_t(tp)).t_dest = &mut (*player_t()).t_pos;
        }
    }

    tp
}

/// Potentially gives a monster a carried item based on depth and monster carry chance.
#[no_mangle]
pub unsafe extern "C" fn give_pack(tp: *mut CThing) {
    if level >= max_level && rnd(100) < monsters[((*thing_t(tp)).t_type as i32 - 'A' as i32) as usize].m_carry {
        _attach(&mut (*thing_t(tp)).t_pack, new_thing());
    }
}

/// Rolls a saving throw for any creature against an effect category.
#[no_mangle]
pub unsafe extern "C" fn save_throw(which: c_int, tp: *mut CThing) -> c_int {
    let need = 14 + which - (*thing_t(tp)).t_stats.s_lvl / 2;
    if roll(1, 20) >= need {
        1
    } else {
        0
    }
}

/// Rolls the hero's saving throw, applying ring of protection magic adjustment.
#[no_mangle]
pub unsafe extern "C" fn save(which: c_int) -> c_int {
    let mut adj = which;
    if which == VS_MAGIC {
        if !cur_ring[LEFT].is_null() && (*thing_o(cur_ring[LEFT])).o_which == R_PROTECT {
            adj -= (*thing_o(cur_ring[LEFT])).o_arm;
        }
        if !cur_ring[RIGHT].is_null() && (*thing_o(cur_ring[RIGHT])).o_which == R_PROTECT {
            adj -= (*thing_o(cur_ring[RIGHT])).o_arm;
        }
    }
    save_throw(adj, &raw mut player)
}
