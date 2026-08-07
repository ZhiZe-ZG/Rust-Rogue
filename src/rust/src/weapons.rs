use std::os::raw::{c_char, c_int, c_uchar};

const NO_WEAPON: c_int = -1;

const FLOOR: c_int = '.' as c_int;
const PASSAGE: c_int = '#' as c_int;
const DOOR: c_int = '+' as c_int;
const WEAPON: c_char = ')' as c_char;
const ARMOR: c_char = ']' as c_char;

const BOW: c_int = 2;
const DAGGER: c_int = 4;
const MAXWEAPONS: usize = 9;

const ISMISL: c_int = 0o000004;
const ISMANY: c_int = 0o000010;

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CCoord {
    pub x: c_int,
    pub y: c_int,
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
    pub t_room: *mut c_char,
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
    pub p_ch: c_char,
    pub p_flags: c_char,
    pub p_monst: *mut CThing,
}

#[repr(C)]
pub struct CObjInfo {
    pub oi_name: *mut c_char,
    pub oi_prob: c_int,
    pub oi_worth: c_int,
    pub oi_guess: *mut c_char,
    pub oi_know: c_uchar,
}

#[derive(Copy, Clone)]
struct InitWeap {
    iw_dam: &'static [u8],
    iw_hrl: &'static [u8],
    iw_launch: c_int,
    iw_flags: c_int,
}

static INIT_DAM: [InitWeap; MAXWEAPONS] = [
    InitWeap { iw_dam: b"2x4\0", iw_hrl: b"1x3\0", iw_launch: NO_WEAPON, iw_flags: 0 },
    InitWeap { iw_dam: b"3x4\0", iw_hrl: b"1x2\0", iw_launch: NO_WEAPON, iw_flags: 0 },
    InitWeap { iw_dam: b"1x1\0", iw_hrl: b"1x1\0", iw_launch: NO_WEAPON, iw_flags: 0 },
    InitWeap { iw_dam: b"1x1\0", iw_hrl: b"2x3\0", iw_launch: BOW, iw_flags: ISMANY | ISMISL },
    InitWeap { iw_dam: b"1x6\0", iw_hrl: b"1x4\0", iw_launch: NO_WEAPON, iw_flags: ISMISL | ISMISL },
    InitWeap { iw_dam: b"4x4\0", iw_hrl: b"1x2\0", iw_launch: NO_WEAPON, iw_flags: 0 },
    InitWeap { iw_dam: b"1x1\0", iw_hrl: b"1x3\0", iw_launch: NO_WEAPON, iw_flags: ISMANY | ISMISL },
    InitWeap { iw_dam: b"1x2\0", iw_hrl: b"2x4\0", iw_launch: NO_WEAPON, iw_flags: ISMANY | ISMISL },
    InitWeap { iw_dam: b"2x3\0", iw_hrl: b"1x6\0", iw_launch: NO_WEAPON, iw_flags: ISMISL },
];

#[no_mangle]
pub static mut group: c_int = 2;

static mut NUMBUF: [c_char; 10] = [0; 10];
static mut FALL_POS: CCoord = CCoord { x: 0, y: 0 };

unsafe extern "C" {
    static mut terse: c_uchar;
    static mut after: c_uchar;
    static mut has_hit: c_uchar;
    static mut places: [CPlace; 32 * 80];
    static mut player: CThing;
    static mut cur_weapon: *mut CThing;
    static mut lvl_obj: *mut CThing;
    static mut weap_info: [CObjInfo; MAXWEAPONS + 1];

    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn dropcheck(obj: *mut CThing) -> c_uchar;
    fn is_current(obj: *mut CThing) -> c_uchar;
    fn leave_pack(obj: *mut CThing, newobj: c_uchar, all: c_uchar) -> *mut CThing;
    fn cansee(y: c_int, x: c_int) -> c_uchar;
    fn show_floor() -> c_uchar;
    fn step_ok(ch: c_int) -> c_int;
    fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int;
    fn refresh() -> c_int;
    fn fight(mp: *mut CCoord, weap: *mut CThing, thrown: c_uchar) -> c_int;
    fn discard(item: *mut CThing);
    fn _attach(list: *mut *mut CThing, item: *mut CThing);
    fn endmsg() -> c_int;
    fn msg(fmt: *const c_char, ...);
    fn inv_name(obj: *mut CThing, drop: c_uchar) -> *mut c_char;
    fn addmsg(fmt: *const c_char, ...);
    fn rnd(range: c_int) -> c_int;
    fn snprintf(s: *mut c_char, n: usize, fmt: *const c_char, ...) -> c_int;
}

use std::os::raw::{c_short, c_uint};

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn hero() -> CCoord {
    (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn place_at(y: c_int, x: c_int) -> *mut CPlace {
    let idx = ((x << 5) + y) as isize;
    (&raw mut places as *mut CPlace).offset(idx)
}

#[inline]
unsafe fn chat(y: c_int, x: c_int) -> c_int {
    (*place_at(y, x)).p_ch as c_uchar as c_int
}

#[inline]
unsafe fn moat(y: c_int, x: c_int) -> *mut CThing {
    (*place_at(y, x)).p_monst
}

#[inline]
unsafe fn winat(y: c_int, x: c_int) -> c_int {
    let m = moat(y, x);
    if m.is_null() {
        chat(y, x)
    } else {
        (*thing_t(m)).t_disguise as c_uchar as c_int
    }
}

#[inline]
unsafe fn copy_c_bytes(dst: &mut [c_char], src: &[u8]) {
    let mut i = 0usize;
    while i + 1 < dst.len() && i < src.len() {
        dst[i] = src[i] as c_char;
        if src[i] == 0 {
            return;
        }
        i += 1;
    }
    dst[dst.len() - 1] = 0;
}

/// Throws a selected weapon in the provided direction and resolves impact/fall behavior.
#[no_mangle]
pub unsafe extern "C" fn missile(ydelta: c_int, xdelta: c_int) {
    let mut obj = get_item(c"throw".as_ptr(), WEAPON as c_int);
    if obj.is_null() {
        return;
    }
    if dropcheck(obj) == 0 || is_current(obj) != 0 {
        return;
    }

    obj = leave_pack(obj, TRUE, FALSE);
    do_motion(obj, ydelta, xdelta);

    let o = thing_o(obj);
    if moat((*o).o_pos.y, (*o).o_pos.x).is_null() || hit_monster((*o).o_pos.y, (*o).o_pos.x, obj) == 0 {
        fall(obj, TRUE);
    }
}

/// Animates projectile movement until it hits blocking terrain or a door.
#[no_mangle]
pub unsafe extern "C" fn do_motion(obj: *mut CThing, ydelta: c_int, xdelta: c_int) {
    let o = thing_o(obj);
    (*o).o_pos = hero();

    loop {
        let h = hero();
        if ((*o).o_pos.x != h.x || (*o).o_pos.y != h.y) && cansee((*o).o_pos.y, (*o).o_pos.x) != 0 && terse == 0 {
            let mut ch = chat((*o).o_pos.y, (*o).o_pos.x);
            if ch == FLOOR && show_floor() == 0 {
                ch = ' ' as c_int;
            }
            mvaddch((*o).o_pos.y, (*o).o_pos.x, ch as c_uint);
        }

        (*o).o_pos.y += ydelta;
        (*o).o_pos.x += xdelta;

        let ch = winat((*o).o_pos.y, (*o).o_pos.x);
        if step_ok(ch) != 0 && ch != DOOR {
            if cansee((*o).o_pos.y, (*o).o_pos.x) != 0 && terse == 0 {
                mvaddch((*o).o_pos.y, (*o).o_pos.x, (*o).o_type as c_uint);
                refresh();
            }
            continue;
        }
        break;
    }
}

/// Drops an item near its current position or discards it if no floor slot is available.
#[no_mangle]
pub unsafe extern "C" fn fall(obj: *mut CThing, pr: c_uchar) {
    if fallpos(&mut (*thing_o(obj)).o_pos, &raw mut FALL_POS) != 0 {
        let pp = place_at(FALL_POS.y, FALL_POS.x);
        (*pp).p_ch = (*thing_o(obj)).o_type as c_char;
        (*thing_o(obj)).o_pos = FALL_POS;

        if cansee(FALL_POS.y, FALL_POS.x) != 0 {
            if !(*pp).p_monst.is_null() {
                (*thing_t((*pp).p_monst)).t_oldch = (*thing_o(obj)).o_type as c_char;
            } else {
                mvaddch(FALL_POS.y, FALL_POS.x, (*thing_o(obj)).o_type as c_uint);
            }
        }

        _attach(&raw mut lvl_obj, obj);
        return;
    }

    if pr != 0 {
        if has_hit != 0 {
            endmsg();
            has_hit = 0;
        }
        msg(
            c"the %s vanishes as it hits the ground".as_ptr(),
            weap_info[(*thing_o(obj)).o_which as usize].oi_name,
        );
    }

    discard(obj);
}

/// Initializes a weapon object with baseline damage, flags, and stack counts.
#[no_mangle]
pub unsafe extern "C" fn init_weapon(weap: *mut CThing, which: c_int) {
    let o = thing_o(weap);
    (*o).o_type = WEAPON as c_int;
    (*o).o_which = which;

    let iwp = INIT_DAM[which as usize];
    copy_c_bytes(&mut (*o).o_damage, iwp.iw_dam);
    copy_c_bytes(&mut (*o).o_hurldmg, iwp.iw_hrl);
    (*o).o_launch = iwp.iw_launch;
    (*o).o_flags = iwp.iw_flags;
    (*o).o_hplus = 0;
    (*o).o_dplus = 0;

    if which == DAGGER {
        (*o).o_count = rnd(4) + 2;
        (*o).o_group = group;
        group += 1;
    } else if ((*o).o_flags & ISMANY) != 0 {
        (*o).o_count = rnd(8) + 8;
        (*o).o_group = group;
        group += 1;
    } else {
        (*o).o_count = 1;
        (*o).o_group = 0;
    }
}

/// Resolves thrown-weapon combat against the target tile.
#[no_mangle]
pub unsafe extern "C" fn hit_monster(y: c_int, x: c_int, obj: *mut CThing) -> c_int {
    let mut mp = CCoord { x, y };
    fight(&mut mp, obj, TRUE)
}

/// Formats signed enchantment numbers for armor and weapons.
#[no_mangle]
pub unsafe extern "C" fn num(n1: c_int, n2: c_int, obj_type: c_char) -> *mut c_char {
    if obj_type == WEAPON {
        let _ = snprintf((&raw mut NUMBUF) as *mut c_char, 10, c"%+d,%+d".as_ptr(), n1, n2);
    } else {
        let _ = snprintf((&raw mut NUMBUF) as *mut c_char, 10, c"%+d".as_ptr(), n1);
    }
    (&raw mut NUMBUF) as *mut c_char
}

/// Equips a selected weapon after validating curses and item type constraints.
#[no_mangle]
pub unsafe extern "C" fn wield() {
    let oweapon = cur_weapon;
    if dropcheck(cur_weapon) == 0 {
        cur_weapon = oweapon;
        return;
    }
    cur_weapon = oweapon;

    let obj = get_item(c"wield".as_ptr(), WEAPON as c_int);
    if obj.is_null() {
        after = 0;
        return;
    }

    if (*thing_o(obj)).o_type == ARMOR as c_int {
        msg(c"you can't wield armor".as_ptr());
        after = 0;
        return;
    }
    if is_current(obj) != 0 {
        after = 0;
        return;
    }

    let sp = inv_name(obj, TRUE);
    cur_weapon = obj;
    if terse == 0 {
        addmsg(c"you are now ".as_ptr());
    }
    msg(c"wielding %s (%c)".as_ptr(), sp, (*thing_o(obj)).o_packch as c_int);
}

/// Chooses a nearby floor/passage location to drop an item and returns whether one was found.
#[no_mangle]
pub unsafe extern "C" fn fallpos(pos: *mut CCoord, newpos: *mut CCoord) -> c_uchar {
    let mut cnt = 0;
    for y in ((*pos).y - 1)..=((*pos).y + 1) {
        for x in ((*pos).x - 1)..=((*pos).x + 1) {
            let h = hero();
            if y == h.y && x == h.x {
                continue;
            }
            let ch = chat(y, x);
            if ch == FLOOR || ch == PASSAGE {
                cnt += 1;
                if rnd(cnt) == 0 {
                    (*newpos).y = y;
                    (*newpos).x = x;
                }
            }
        }
    }
    if cnt != 0 { TRUE } else { FALSE }
}
