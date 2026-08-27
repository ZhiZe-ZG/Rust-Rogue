use crate::rnd::rnd;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};
use crate::draw::{set_tile_char, place_at};
use crate::level::{be_trapped, door_open, T_DOOR, T_TELEP};
use crate::rndmove::rndmove;

const NUMCOLS: c_int = 80;
const NUMLINES: c_int = 24;

const DOOR: c_char = b'+' as c_char;
const FLOOR: c_char = b'.' as c_char;
const PASSAGE: c_char = b'#' as c_char;
const TRAP: c_char = b'^' as c_char;
const STAIRS: c_char = b'%' as c_char;
const SPACE: c_char = b' ' as c_char;
const H_WALL: c_char = b'-' as c_char;
const V_WALL: c_char = b'|' as c_char;

const ISBLIND: c_short = 0o0000004;
const ISHELD: c_short = 0o0000400;
const ISHUH: c_short = 0o0001000;
const ISLEVIT: c_short = 0o0000010;
const ISRUN: c_short = 0o020000;
const SEEMONST: c_short = 0o040000;

const ISDARK: c_short = 0o0000001;
const ISGONE: c_short = 0o0000002;
const ISMAZE: c_short = 0o0000004;

const F_PASS: c_char = 0x80u8 as c_char;
const F_REAL: c_char = 0x10u8 as c_char;
const F_SEEN: c_char = 0x40u8 as c_char;
const F_PNUM: c_char = 0x0fu8 as c_char;


const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;
const MAXPASS: usize = 13;

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
pub struct CRoom {
    pub r_pos: CCoord,
    pub r_max: CCoord,
    pub r_gold: CCoord,
    pub r_goldval: c_int,
    pub r_flags: c_short,
    pub r_nexits: c_int,
    pub r_exit: [CCoord; 12],
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
#[derive(Copy, Clone)]
pub struct CPlace {
    pub p_ch: c_char,
    pub p_flags: c_char,
    pub p_monst: *mut CThing,
}

#[repr(C)]
pub struct CWindow {
    _private: [u8; 0],
}

unsafe extern "C" {
    static mut after: c_uchar;
    static mut count: c_int;
    static mut door_stop: c_uchar;
    static mut firstmove: c_uchar;
    static mut jump: c_uchar;
    static mut move_on: c_uchar;
    static mut no_move: c_int;
    static mut passgo: c_uchar;
    static mut running: c_uchar;
    static mut seenstairs: c_uchar;
    static mut take: c_char;
    static mut to_death: c_uchar;
    static mut oldpos: CCoord;
    static mut delta: CCoord;
    static mut cur_weapon: *mut CThing;
    static mut player: CThing;
    static mut passages: [CRoom; MAXPASS];
    static mut runch: c_char;
    static mut places: [CPlace; 32 * 80];
    static mut stdscr: *mut CWindow;

    fn msg(fmt: *const c_char, ...);
    fn diag_ok(sp: *mut CCoord, ep: *mut CCoord) -> c_uchar;
    fn see_monst(mp: *mut CThing) -> c_uchar;
    fn fight(mp: *mut CCoord, weap: *mut CThing, thrown: c_uchar) -> c_int;
    fn roomin(cp: *mut CCoord) -> *mut CRoom;
    fn floor_at() -> c_char;
    fn r#move(y: c_int, x: c_int) -> c_int;
    fn inch() -> c_uint;
    fn addch(ch: c_uint) -> c_int;
    fn standout() -> c_int;
    fn standend() -> c_int;
    fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int;
    fn leaveok(win: *mut CWindow, flag: c_int) -> c_int;
    fn refresh() -> c_int;
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
unsafe fn hero_ptr() -> *mut CCoord {
    &mut (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn hero_pos() -> CCoord {
    (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn player_has(flag: c_short) -> bool {
    ((*thing_t(&raw mut player)).t_flags & flag) != 0
}

#[inline]
unsafe fn coord_eq(a: CCoord, b: CCoord) -> bool {
    a.x == b.x && a.y == b.y
}

#[inline]
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_ch
}

#[inline]
unsafe fn flat_at(y: c_int, x: c_int) -> c_char {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_flags
}

#[inline]
unsafe fn add_flat_flag(y: c_int, x: c_int, flag: c_char) {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_flags = (((*place_at((&raw mut places) as *mut CPlace, y, x)).p_flags as u8) | (flag as u8)) as c_char;
}

#[inline]
unsafe fn winat(y: c_int, x: c_int) -> c_char {
    let tp = (*place_at((&raw mut places) as *mut CPlace, y, x)).p_monst;
    if tp.is_null() {
        chat_at(y, x)
    } else {
        (*thing_o(tp)).o_packch
    }
}

#[inline]
unsafe fn is_upper(ch: c_char) -> bool {
    (ch as u8).is_ascii_uppercase()
}

#[inline]
unsafe fn cchar_at_cursor() -> c_char {
    inch() as u8 as c_char
}

/// enter_room:
/// Code that is executed whenever the hero appears in a room.
#[no_mangle]
pub unsafe extern "C" fn enter_room(cp: *mut CCoord) {
    if cp.is_null() {
        return;
    }

    let rp = roomin(cp);
    if rp.is_null() {
        return;
    }

    (*thing_t(&raw mut player)).t_room = rp;
    door_open(rp);

    if ((*rp).r_flags & ISDARK) != 0 || player_has(ISBLIND) {
        return;
    }

    let y0 = (*rp).r_pos.y;
    let x0 = (*rp).r_pos.x;
    let y_end = y0 + (*rp).r_max.y;
    let x_end = x0 + (*rp).r_max.x;
    let mut y = y0;
    while y < y_end {
        r#move(y, x0);
        let mut x = x0;
        while x < x_end {
            let pp = place_at((&raw mut places) as *mut CPlace, y, x);
            let tp = (*pp).p_monst;
            let ch = (*pp).p_ch;

            if tp.is_null() {
                if cchar_at_cursor() != ch {
                    addch(ch as c_uint);
                } else {
                    r#move(y, x + 1);
                }
            } else {
                (*thing_t(tp)).t_oldch = ch;
                if see_monst(tp) == 0 {
                    if player_has(SEEMONST) {
                        standout();
                        addch((*thing_t(tp)).t_disguise as c_uint);
                        standend();
                    } else {
                        addch(ch as c_uint);
                    }
                } else {
                    addch((*thing_t(tp)).t_disguise as c_uint);
                }
            }
            x += 1;
        }
        y += 1;
    }
}

/// leave_room:
/// Code for when the hero exits a room.
#[no_mangle]
pub unsafe extern "C" fn leave_room(cp: *mut CCoord) {
    if cp.is_null() {
        return;
    }

    let rp = (*thing_t(&raw mut player)).t_room;
    if rp.is_null() {
        return;
    }

    if ((*rp).r_flags & ISMAZE) != 0 {
        return;
    }

    let floor = if ((*rp).r_flags & ISGONE) != 0 {
        PASSAGE
    } else if ((*rp).r_flags & ISDARK) == 0 || player_has(ISBLIND) {
        FLOOR
    } else {
        SPACE
    };

    let pnum = (flat_at((*cp).y, (*cp).x) as u8 & F_PNUM as u8) as usize;
    if pnum < MAXPASS {
        (*thing_t(&raw mut player)).t_room = (&raw mut passages[pnum]) as *mut CRoom;
    }

    let y0 = (*rp).r_pos.y;
    let x0 = (*rp).r_pos.x;
    let y_end = y0 + (*rp).r_max.y;
    let x_end = x0 + (*rp).r_max.x;
    let mut y = y0;
    while y < y_end {
        let mut x = x0;
        while x < x_end {
            r#move(y, x);
            let ch = cchar_at_cursor();
            if ch == FLOOR {
                if floor == SPACE && ch != SPACE {
                    addch(SPACE as c_uint);
                }
            } else if is_upper(ch) {
                if player_has(SEEMONST) {
                    standout();
                    addch(ch as c_uint);
                    standend();
                } else {
                    let pp = place_at((&raw mut places) as *mut CPlace, y, x);
                    let out = if (*pp).p_ch == DOOR { DOOR } else { floor };
                    addch(out as c_uint);
                }
            }
            x += 1;
        }
        y += 1;
    }

    door_open(rp);
}

/// turnref:
/// Decide whether to refresh at a passage turning or not.
#[no_mangle]
pub unsafe extern "C" fn turnref() {
    let hero = hero_pos();
    let place = place_at((&raw mut places) as *mut CPlace, hero.y, hero.x);
    if ((*place).p_flags as u8 & F_SEEN as u8) == 0 {
        if jump != 0 {
            leaveok(stdscr, TRUE as c_int);
            refresh();
            leaveok(stdscr, FALSE as c_int);
        }
        (*place).p_flags = (((*place).p_flags as u8) | (F_SEEN as u8)) as c_char;
    }
}

/// turn_ok:
/// Decide whether it is legal to turn onto the given space.
#[no_mangle]
pub unsafe extern "C" fn turn_ok(y: c_int, x: c_int) -> c_uchar {
    let place = place_at((&raw mut places) as *mut CPlace, y, x);
    let flags = (*place).p_flags as u8;
    if (*place).p_ch == DOOR || (flags & (F_REAL as u8 | F_PASS as u8)) == (F_REAL as u8 | F_PASS as u8) {
        TRUE
    } else {
        0
    }
}

#[inline]
unsafe fn move_stuff(next_pos: &mut CCoord, fl: c_char) {
    let hero = hero_pos();
    mvaddch(hero.y, hero.x, floor_at() as c_uint);
    if (fl as u8 & F_PASS as u8) != 0 && chat_at(oldpos.y, oldpos.x) == DOOR {
        leave_room(next_pos);
    }
    *hero_ptr() = *next_pos;
}

#[inline]
unsafe fn try_passgo_turn(dy: &mut c_int, dx: &mut c_int) -> bool {
    let current_room = (*thing_t(&raw mut player)).t_room;
    if passgo == 0 || running == 0 || current_room.is_null() || ((*current_room).r_flags & 0o000002) == 0 || player_has(ISBLIND) {
        return false;
    }

    let hero = hero_pos();
    if runch == b'h' as c_char || runch == b'l' as c_char {
        let b1 = hero.y != 1 && turn_ok(hero.y - 1, hero.x) != 0;
        let b2 = hero.y != NUMLINES - 2 && turn_ok(hero.y + 1, hero.x) != 0;
        if !(b1 ^ b2) {
            return false;
        }
        if b1 {
            runch = b'k' as c_char;
            *dy = -1;
        } else {
            runch = b'j' as c_char;
            *dy = 1;
        }
        *dx = 0;
        turnref();
        true
    } else if runch == b'j' as c_char || runch == b'k' as c_char {
        let b1 = hero.x != 0 && turn_ok(hero.y, hero.x - 1) != 0;
        let b2 = hero.x != NUMCOLS - 1 && turn_ok(hero.y, hero.x + 1) != 0;
        if !(b1 ^ b2) {
            return false;
        }
        if b1 {
            runch = b'h' as c_char;
            *dx = -1;
        } else {
            runch = b'l' as c_char;
            *dx = 1;
        }
        *dy = 0;
        turnref();
        true
    } else {
        false
    }
}

/// Global "next hero position" used by the save/load subsystem (state.c).
#[no_mangle]
pub static mut nh: CCoord = CCoord { x: 0, y: 0 };

/// do_run:
/// Start the hero running in the chosen direction.
#[no_mangle]
pub unsafe extern "C" fn do_run(ch: c_char) {
    running = TRUE;
    after = FALSE;
    runch = ch;
}

/// do_move:
/// Check to see that a move is legal. If it is, handle the consequences.
#[no_mangle]
pub unsafe extern "C" fn do_move(dy: c_int, dx: c_int) {
    let mut next_pos = CCoord { x: 0, y: 0 };
    let mut current_dy = dy;
    let mut current_dx = dx;
    let hero = hero_pos();
    let mut ch: c_char;
    let fl: c_char;

    firstmove = FALSE;
    if no_move != 0 {
        no_move -= 1;
        msg(c"you are still stuck in the bear trap".as_ptr());
        return;
    }

    if player_has(ISHUH) && rnd(5) != 0 {
        next_pos = *rndmove(&raw mut player);
        if coord_eq(next_pos, hero) {
            after = FALSE;
            running = FALSE;
            to_death = FALSE;
            return;
        }
    } else {
        next_pos.y = hero.y + current_dy;
        next_pos.x = hero.x + current_dx;
    }

    loop {
        if next_pos.x < 0 || next_pos.x >= NUMCOLS || next_pos.y <= 0 || next_pos.y >= NUMLINES - 1 {
            if try_passgo_turn(&mut current_dy, &mut current_dx) {
                next_pos.y = hero.y + current_dy;
                next_pos.x = hero.x + current_dx;
                continue;
            }
            running = FALSE;
            after = FALSE;
            return;
        }
        break;
    }

    if diag_ok(hero_ptr(), &mut next_pos) == 0 {
        after = FALSE;
        running = FALSE;
        return;
    }

    if running != 0 && coord_eq(hero, next_pos) {
        running = FALSE;
    }

    fl = flat_at(next_pos.y, next_pos.x);
    ch = winat(next_pos.y, next_pos.x);

    if (fl as u8 & F_REAL as u8) == 0 && ch == FLOOR {
        if !player_has(ISLEVIT) {
            set_tile_char(next_pos.y, next_pos.x, TRAP);
            add_flat_flag(next_pos.y, next_pos.x, F_REAL);
            ch = TRAP;
        }
    } else if player_has(ISHELD) && ch != b'F' as c_char {
        msg(c"you are being held".as_ptr());
        return;
    }
    match ch {
        SPACE | H_WALL | V_WALL => {
            running = FALSE;
            after = FALSE;
        }
        DOOR => {
            running = FALSE;
            if (flat_at(hero.y, hero.x) as u8 & F_PASS as u8) != 0 {
                enter_room(&mut next_pos);
            }
            move_stuff(&mut next_pos, fl);
        }
        TRAP => {
            let trap = be_trapped(next_pos);
            if trap == T_DOOR || trap == T_TELEP {
                return;
            }
            move_stuff(&mut next_pos, fl);
        }
        PASSAGE => {
            (*thing_t(&raw mut player)).t_room = roomin(hero_ptr());
            move_stuff(&mut next_pos, fl);
        }
        FLOOR => {
            if (fl as u8 & F_REAL as u8) == 0 {
                be_trapped(hero_pos());
            }
            move_stuff(&mut next_pos, fl);
        }
        STAIRS => {
            seenstairs = TRUE;
            running = FALSE;
            if is_upper(ch) || !(*place_at((&raw mut places) as *mut CPlace, next_pos.y, next_pos.x)).p_monst.is_null() {
                fight(&mut next_pos, cur_weapon, FALSE);
            } else {
                take = ch;
                move_stuff(&mut next_pos, fl);
            }
        }
        _ => {
            running = FALSE;
            if is_upper(ch) || !(*place_at((&raw mut places) as *mut CPlace, next_pos.y, next_pos.x)).p_monst.is_null() {
                fight(&mut next_pos, cur_weapon, FALSE);
            } else {
                if ch != STAIRS {
                    take = ch;
                }
                move_stuff(&mut next_pos, fl);
            }
        }
    }
}
