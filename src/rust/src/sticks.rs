use std::os::raw::{c_char, c_int, c_uchar, c_void};

const STICK: c_int = '/' as c_int;
const WEAPON: c_int = ')' as c_int;
const FLAME: c_int = 9;
const ISKNOW: c_int = 0o000002;
const ISMISL: c_int = 0o000004;
const WS_LIGHT: c_int = 0;
const WS_INVIS: c_int = 1;
const WS_ELECT: c_int = 2;
const WS_FIRE: c_int = 3;
const WS_COLD: c_int = 4;
const WS_POLYMORPH: c_int = 5;
const WS_MISSILE: c_int = 6;
const WS_HASTE_M: c_int = 7;
const WS_SLOW_M: c_int = 8;
const WS_DRAIN: c_int = 9;
const WS_NOP: c_int = 10;
const WS_TELAWAY: c_int = 11;
const WS_TELTO: c_int = 12;
const WS_CANCEL: c_int = 13;
const VS_MAGIC: c_int = 3;
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
    pub s_str: u32,
    pub s_exp: c_int,
    pub s_lvl: c_int,
    pub s_arm: c_int,
    pub s_hpt: c_int,
    pub s_dmg: [c_char; 13],
    pub s_maxhp: c_int,
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
    pub t_flags: c_int,
    pub t_stats: CStats,
    pub t_room: *mut c_void,
    pub t_pack: *mut CThing,
    pub t_reserved: c_int,
}

#[repr(C)]
pub union CThing {
    pub t: CThingMonster,
    pub o: CThingObject,
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
    static mut after: c_uchar;
    static mut delta: CCoord;
    static mut cur_weapon: *mut CThing;
    static mut ws_info: [CObjInfo; 14];
    static mut places: [CPlace; 32 * 80];
    static mut player: CThing;
    static mut weap_info: [CObjInfo; 10];

    fn get_item(purpose: *const c_char, item_type: c_int) -> *mut CThing;
    fn addmsg(fmt: *const c_char, ...);
    fn endmsg() -> c_int;
    fn msg(fmt: *const c_char, ...);
    fn mvaddch(y: c_int, x: c_int, ch: c_char);
    fn refresh();
    fn save_throw(kind: c_int, tp: *mut CThing) -> c_uchar;
    fn save(kind: c_int) -> c_uchar;
    fn hit_monster(y: c_int, x: c_int, obj: *mut CThing);
    fn do_motion(obj: *mut CThing, y: c_int, x: c_int);
    fn rnd(amt: c_int) -> c_int;
    fn roll(num: c_int, sides: c_int) -> c_int;
    fn death(thing: c_char) -> !;
    fn runto(pos: *mut CCoord);
    fn set_mname(tp: *mut CThing) -> *mut c_char;
    fn cansee(y: c_int, x: c_int) -> c_uchar;
    fn step_ok(ch: c_char) -> c_uchar;
    fn new_monster(tp: *mut CThing, kind: c_char, delta: *const CCoord);
    fn relocate(tp: *mut CThing, new_pos: *const CCoord);
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CPlace {
    pub p_ch: c_char,
    pub p_flags: c_char,
    pub p_monst: *mut CThing,
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn hero_pos() -> CCoord {
    unsafe { (*thing_t(&raw mut player)).t_pos }
}

#[inline]
unsafe fn hero_stats_mut() -> *mut CStats {
    &mut (*thing_t(&raw mut player)).t_stats
}

#[inline]
unsafe fn place_idx(y: c_int, x: c_int) -> usize {
    ((x as usize) << 5) + (y as usize)
}

#[inline]
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    places[place_idx(y, x)].p_ch
}

#[inline]
unsafe fn moat_at(y: c_int, x: c_int) -> *mut CThing {
    places[place_idx(y, x)].p_monst
}

#[inline]
unsafe fn ce_coord(a: CCoord, b: CCoord) -> c_uchar {
    if a.x == b.x && a.y == b.y { 1 } else { 0 }
}

#[inline]
unsafe fn set_c_string(dst: &mut [c_char], src: &str) {
    let bytes = src.as_bytes();
    let limit = bytes.len().min(dst.len().saturating_sub(1));
    for (idx, byte) in bytes[..limit].iter().enumerate() {
        dst[idx] = *byte as c_char;
    }
    if dst.len() > limit {
        dst[limit] = 0;
    }
}

/// fix_stick:
/// Set up a new stick with the expected damage and charge values.
#[no_mangle]
pub unsafe extern "C" fn fix_stick(cur: *mut CThing) {
    if (*thing_o(cur)).o_type != STICK {
        return;
    }

    if (*thing_o(cur)).o_which == WS_LIGHT {
        set_c_string(&mut (*thing_o(cur)).o_damage, "2x3");
    } else {
        set_c_string(&mut (*thing_o(cur)).o_damage, "1x1");
    }
    set_c_string(&mut (*thing_o(cur)).o_hurldmg, "1x1");

    (*thing_o(cur)).o_arm = if (*thing_o(cur)).o_which == WS_LIGHT {
        rnd(10) + 10
    } else {
        rnd(5) + 3
    };
}

/// do_zap:
/// Perform a zap with a wand or staff and apply a simplified effect.
#[no_mangle]
pub unsafe extern "C" fn do_zap() {
    let obj = get_item(c"zap with".as_ptr(), STICK);
    if obj.is_null() {
        return;
    }
    if (*thing_o(obj)).o_type != STICK {
        after = FALSE;
        msg(c"you can't zap with that!".as_ptr());
        return;
    }
    if (*thing_o(obj)).o_arm == 0 {
        msg(c"nothing happens".as_ptr());
        return;
    }

    match (*thing_o(obj)).o_which {
        WS_LIGHT => {
            ws_info[WS_LIGHT as usize].oi_know = TRUE;
            msg(c"the corridor glows and then fades".as_ptr());
        }
        WS_DRAIN => {
            if unsafe { (*hero_stats_mut()).s_hpt } < 2 {
                msg(c"you are too weak to use it".as_ptr());
                return;
            }
            drain();
        }
        WS_INVIS | WS_POLYMORPH | WS_TELAWAY | WS_TELTO | WS_CANCEL => {
            let hero = hero_pos();
            let mut y = hero.y;
            let mut x = hero.x;
            while step_ok(winat(y, x)) != 0 {
                y += delta.y;
                x += delta.x;
            }
            if !moat_at(y, x).is_null() {
                msg(c"the spell takes effect".as_ptr());
            }
        }
        WS_MISSILE => {
            ws_info[WS_MISSILE as usize].oi_know = TRUE;
            let mut bolt = std::mem::zeroed::<CThing>();
            (*thing_o(&mut bolt)).o_type = WEAPON;
            (*thing_o(&mut bolt)).o_which = FLAME;
            set_c_string(&mut (*thing_o(&mut bolt)).o_hurldmg, "1x4");
            (*thing_o(&mut bolt)).o_hplus = 100;
            (*thing_o(&mut bolt)).o_dplus = 1;
            (*thing_o(&mut bolt)).o_flags = ISMISL;
            if !cur_weapon.is_null() {
                (*thing_o(&mut bolt)).o_launch = (*thing_o(cur_weapon)).o_which;
            }
            do_motion(&mut bolt, delta.y, delta.x);
            let bolt_pos = (*thing_o(&mut bolt)).o_pos;
            if !moat_at(bolt_pos.y, bolt_pos.x).is_null()
                && save_throw(VS_MAGIC, moat_at(bolt_pos.y, bolt_pos.x)) == 0
            {
                hit_monster(bolt_pos.y, bolt_pos.x, &mut bolt);
            } else if terse != 0 {
                msg(c"missle vanishes".as_ptr());
            } else {
                msg(c"the missle vanishes with a puff of smoke".as_ptr());
            }
        }
        WS_HASTE_M | WS_SLOW_M => {
            let hero = hero_pos();
            let mut y = hero.y;
            let mut x = hero.x;
            while step_ok(winat(y, x)) != 0 {
                y += delta.y;
                x += delta.x;
            }
            if !moat_at(y, x).is_null() {
                msg(c"the spell takes effect".as_ptr());
            }
        }
        WS_ELECT | WS_FIRE | WS_COLD => {
            let name = match (*thing_o(obj)).o_which {
                WS_ELECT => c"bolt",
                WS_FIRE => c"flame",
                _ => c"ice",
            };
            let mut hero = hero_pos();
            fire_bolt(&mut hero, &raw mut delta, name.as_ptr() as *mut c_char);
            ws_info[(*thing_o(obj)).o_which as usize].oi_know = TRUE;
        }
        WS_NOP => {}
        _ => {
            msg(c"what a bizarre schtick!".as_ptr());
        }
    }

    (*thing_o(obj)).o_arm -= 1;
}

/// drain:
/// Reduce the hero's hit points and apply a simple draining effect.
#[no_mangle]
pub unsafe extern "C" fn drain() {
    if (*hero_stats_mut()).s_hpt >= 2 {
        (*hero_stats_mut()).s_hpt /= 2;
    }
    msg(c"you have a tingling feeling".as_ptr());
}

/// fire_bolt:
/// Fire a bolt in a given direction from a specific starting place.
#[no_mangle]
pub unsafe extern "C" fn fire_bolt(start: *mut CCoord, dir: *mut CCoord, name: *mut c_char) {
    let mut pos = *start;
    let mut hero = hero_pos();
    let mut hit_hero = start != &mut hero;
    let mut bolt = std::mem::zeroed::<CThing>();

    (*thing_o(&mut bolt)).o_type = WEAPON;
    (*thing_o(&mut bolt)).o_which = FLAME;
    set_c_string(&mut (*thing_o(&mut bolt)).o_hurldmg, "6x6");
    (*thing_o(&mut bolt)).o_hplus = 100;
    (*thing_o(&mut bolt)).o_dplus = 0;
    weap_info[FLAME as usize].oi_name = name;

    pos.y += (*dir).y;
    pos.x += (*dir).x;
    if hit_hero && ce_coord(pos, hero) != 0 {
        if save(VS_MAGIC) == 0 {
            if ((*hero_stats_mut()).s_hpt - roll(6, 6)) <= 0 {
                death('b' as c_char);
            }
            msg(c"the bolt hits".as_ptr());
        } else {
            msg(c"the bolt whizzes by you".as_ptr());
        }
    } else {
        if !moat_at(pos.y, pos.x).is_null() && save_throw(VS_MAGIC, moat_at(pos.y, pos.x)) == 0 {
            hit_monster(pos.y, pos.x, &mut bolt);
        } else {
            msg(c"the bolt misses".as_ptr());
        }
    }

    mvaddch(pos.y, pos.x, '/' as c_char);
    refresh();
}

/// charge_str:
/// Return an appropriate string for a wand charge display.
#[no_mangle]
pub unsafe extern "C" fn charge_str(obj: *mut CThing) -> *mut c_char {
    static mut BUF: [c_char; 20] = [0; 20];
    if (*thing_o(obj)).o_flags & ISKNOW == 0 {
        BUF[0] = 0;
    } else if terse != 0 {
        let text = format!(" [{}]", (*thing_o(obj)).o_arm);
        set_c_string(&mut BUF, &text);
    } else {
        let text = format!(" [{} charges]", (*thing_o(obj)).o_arm);
        set_c_string(&mut BUF, &text);
    }
    BUF.as_mut_ptr()
}

unsafe fn winat(y: c_int, x: c_int) -> c_char {
    let tp = moat_at(y, x);
    if tp.is_null() {
        chat_at(y, x)
    } else {
        (*thing_o(tp)).o_packch
    }
}
