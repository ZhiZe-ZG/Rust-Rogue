use crate::rnd::rnd;
/*
 * All the fighting gets done here.
 *
 * Ported from fight.c to Rust.
 *
 * Rogue: Exploring the Dungeons of Doom
 * Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
 * All rights reserved.
 *
 * See the file LICENSE.TXT for full copyright and licensing information.
 */

use crate::io::{addmsg_str, msg_str};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::curses as cur;
use crate::player::{CCoord, CStats, CThing, CThingMonster, CThingObject};

// ─── Constants ────────────────────────────────────────────────────────────────

const TRUE:  c_uchar = 1;
const FALSE: c_uchar = 0;
const MAXSTR: usize  = 1024;

// Item types
const WEAPON: c_int = b')' as c_int;
const GOLD:   c_int = b'*' as c_int;

// Ring-slot indices
const LEFT:  usize = 0;
const RIGHT: usize = 1;

// Ring types
const R_ADDDAM:  c_int = 8;
const R_ADDHIT:  c_int = 7;
const R_PROTECT: c_int = 0;
const R_SUSTSTR: c_int = 2;

// Object flags
const ISMISL: c_int = 0o000004;

// Monster/player flags
const CANHUH:   c_short = 0o000001;
const ISBLIND:  c_short = 0o000004;
const ISCANC:   c_short = 0o000010;
const ISTARGET: c_short = 0o000200;
const ISHELD:   c_short = 0o000400;
const ISHUH:    c_short = 0o001000;
const ISHALU:   c_short = 0o004000;
const ISRUN:    c_short = 0o020000;
const SEEMONST: c_short = 0o040000;

// Misc constants
const BORE_LEVEL: c_int = 50;

// Save-vs constants
const VS_POISON: c_int = 0;
const VS_MAGIC:  c_int = 0o03;

// ─── Adjustments due to strength ─────────────────────────────────────────────

static STR_PLUS: [c_int; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1,
    1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3,
];

static ADD_DAM: [c_int; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 3,
    3, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6,
];

// ─── Hit/miss message tables ──────────────────────────────────────────────────

#[no_mangle]
pub static mut h_names: [*const c_char; 8] = [
    c" scored an excellent hit on ".as_ptr(),
    c" hit ".as_ptr(),
    c" have injured ".as_ptr(),
    c" swing and hit ".as_ptr(),
    c" scored an excellent hit on ".as_ptr(),
    c" hit ".as_ptr(),
    c" has injured ".as_ptr(),
    c" swings and hits ".as_ptr(),
];

#[no_mangle]
pub static mut m_names: [*const c_char; 8] = [
    c" miss".as_ptr(),
    c" swing and miss".as_ptr(),
    c" barely miss".as_ptr(),
    c" don't hit".as_ptr(),
    c" misses".as_ptr(),
    c" swings and misses".as_ptr(),
    c" barely misses".as_ptr(),
    c" doesn't hit".as_ptr(),
];

// ─── Static name buffer for set_mname ────────────────────────────────────────

static mut MNAME_BUF:  [c_char; MAXSTR] = [0; MAXSTR];
static mut MNAME_INIT: bool = false;

// Static name buffer for prname
static mut PRNAME_BUF: [c_char; MAXSTR] = [0; MAXSTR];

// ─── Extern C globals ─────────────────────────────────────────────────────────

unsafe extern "C" {
    static mut player:      CThing;
    static mut mlist:       *mut CThing;
    static mut monsters:    [CMonster; 26];
    static mut cur_armor:   *mut CThing;
    static mut cur_ring:    [*mut CThing; 2];
    static mut cur_weapon:  *mut CThing;
    static mut weap_info:   [CObjInfo; 10]; // MAXWEAPONS + 1
    static mut e_levels:    [c_int; 21];

    static mut count:       c_int;
    static mut quiet:       c_int;
    static mut running:     c_uchar;
    static mut to_death:    c_uchar;
    static mut kamikaze:    c_uchar;
    static mut has_hit:     c_uchar;
    static mut terse:       c_uchar;
    static mut fight_flush: c_uchar;
    static mut no_command:  c_int;
    static mut vf_hit:      c_int;
    static mut max_hit:     c_int;
    static mut purse:       c_int;
    static mut level:       c_int;
    static mut max_level:   c_int;
}

// ─── Extern C functions ───────────────────────────────────────────────────────

unsafe extern "C" {
    fn roll(n: c_int, sides: c_int) -> c_int;
    fn endmsg() -> c_int;
    fn isupper(c: c_int) -> c_int;
    fn toupper(c: c_int) -> c_int;
    fn toascii(c: c_int) -> c_int;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strchr(s: *const c_char, c: c_int) -> *mut c_char;
    fn atoi(s: *const c_char) -> c_int;
    fn sprintf(s: *mut c_char, fmt: *const c_char, ...) -> c_int;

    fn runto(cp: *mut CCoord);
    fn choose_str(ts: *const c_char, ns: *const c_char) -> *const c_char;
    fn pick_color(col: *const c_char) -> *const c_char;
    fn see_monst(mp: *mut CThing) -> c_uchar;
    fn death(monst: c_char) -> !;
    fn check_level();
    fn chg_str(amt: c_int);
    fn rust_armor(arm: *mut CThing);
    fn save(which: c_int) -> c_int;
    fn new_item() -> *mut CThing;
    fn fallpos(pos: *mut CCoord, newpos: *mut CCoord) -> c_uchar;
    fn fall(obj: *mut CThing, pr: c_uchar);
    fn discard(item: *mut CThing);
    fn inv_name(obj: *mut CThing, drop_it: c_uchar) -> *mut c_char;
    fn leave_pack(obj: *mut CThing, newobj: c_uchar, all: c_uchar) -> *mut CThing;
    fn status();
    fn flush_type();
    fn _detach(list: *mut *mut CThing, item: *mut CThing);
    fn _attach(list: *mut *mut CThing, item: *mut CThing);
    fn spread(nm: c_int) -> c_int;
}

// ─── Local structs (repr(C)) ──────────────────────────────────────────────────

#[repr(C)]
pub struct CMonster {
    pub m_name:  *mut c_char,
    pub m_carry: c_int,
    pub m_flags: c_short,
    pub m_stats: CStats,
}

#[repr(C)]
pub struct CObjInfo {
    pub oi_name:  *mut c_char,
    pub oi_prob:  c_int,
    pub oi_worth: c_int,
    pub oi_guess: *mut c_char,
    pub oi_know:  c_uchar,
}

// ─── Inline helpers ───────────────────────────────────────────────────────────

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn on_p(tp: *mut CThing, flag: c_short) -> bool {
    ((*thing_t(tp)).t_flags & flag) != 0
}

#[inline]
unsafe fn chat(y: c_int, x: c_int) -> c_char {
    crate::draw::chat_at(y, x)
}

#[inline]
unsafe fn isring(hand: usize, ring_type: c_int) -> bool {
    !cur_ring[hand].is_null() && (*thing_o(cur_ring[hand])).o_which == ring_type
}

#[inline]
unsafe fn iswearing(ring_type: c_int) -> bool {
    isring(LEFT, ring_type) || isring(RIGHT, ring_type)
}

#[inline]
unsafe fn moat(y: c_int, x: c_int) -> *mut CThing {
    crate::game::monster_at(y, x)
}

#[inline]
unsafe fn set_moat(y: c_int, x: c_int, val: *mut CThing) {
    crate::game::set_monster(y, x, val);
}

// ─── Exported functions ───────────────────────────────────────────────────────

/// fight:
/// The player attacks the monster.
#[no_mangle]
pub unsafe extern "C" fn fight(mp: *mut CCoord, weap: *mut CThing, thrown: c_uchar) -> c_int {
    let tp = moat((*mp).y, (*mp).x);

    // Since we are fighting, things are not quiet — no healing.
    count = 0;
    quiet = 0;
    runto(mp);

    // Let him know it was really a xeroc (if it was one).
    let mut ch: c_char = b'\0' as c_char;
    if (*thing_t(tp)).t_type == b'X' as c_char
        && (*thing_t(tp)).t_disguise != b'X' as c_char
        && !on_p(&raw mut player, ISBLIND)
    {
        (*thing_t(tp)).t_disguise = b'X' as c_char;
        if on_p(&raw mut player, ISHALU) {
            ch = (rnd(26) + b'A' as c_int) as c_char;
            cur::mvaddch((*thing_t(tp)).t_pos.y, (*thing_t(tp)).t_pos.x, ch as c_uint);
        }
        msg_str(&CStr::from_ptr(choose_str(
            c"heavy!  That's a nasty critter!".as_ptr(),
            c"wait!  That's a xeroc!".as_ptr(),
        )).to_string_lossy());
        if thrown == 0 {
            return FALSE as c_int;
        }
    }

    let mname = set_mname(tp);
    let mut did_hit = FALSE;
    has_hit = if terse != 0 && to_death == 0 { TRUE } else { FALSE };

    if roll_em(&raw mut player, tp, weap, thrown) != 0 {
        did_hit = FALSE;
        if thrown != 0 {
            thunk(weap, mname, terse);
        } else {
            hit(std::ptr::null_mut(), mname, terse);
        }
        if on_p(&raw mut player, CANHUH) {
            did_hit = TRUE;
            (*thing_t(tp)).t_flags |= ISHUH;
            (*thing_t(&raw mut player)).t_flags &= !CANHUH;
            endmsg();
            has_hit = FALSE;
            msg_str(&format!(
                "your hands stop glowing {}",
                CStr::from_ptr(pick_color(c"red".as_ptr())).to_string_lossy()
            ));
        }
        if (*thing_t(tp)).t_stats.s_hpt <= 0 {
            killed(tp, TRUE);
        } else if did_hit != 0 && !on_p(&raw mut player, ISBLIND) {
            msg_str(&format!("{} appears confused", CStr::from_ptr(mname).to_string_lossy()));
        }
        did_hit = TRUE;
    } else if thrown != 0 {
        bounce(weap, mname, terse);
    } else {
        miss(std::ptr::null_mut(), mname, terse);
    }
    did_hit as c_int
}

/// attack:
/// The monster attacks the player.
#[no_mangle]
pub unsafe extern "C" fn attack(mp: *mut CThing) -> c_int {
    // Stop running / healing.
    running = FALSE;
    count = 0;
    quiet = 0;

    if to_death != 0 && !on_p(mp, ISTARGET) {
        to_death = FALSE;
        kamikaze = FALSE;
    }

    if (*thing_t(mp)).t_type == b'X' as c_char
        && (*thing_t(mp)).t_disguise != b'X' as c_char
        && !on_p(&raw mut player, ISBLIND)
    {
        (*thing_t(mp)).t_disguise = b'X' as c_char;
        if on_p(&raw mut player, ISHALU) {
            cur::mvaddch(
                (*thing_t(mp)).t_pos.y,
                (*thing_t(mp)).t_pos.x,
                (rnd(26) + b'A' as c_int) as c_uint,
            );
        }
    }

    let mname = set_mname(mp);
    let oldhp = (*thing_t(&raw mut player)).t_stats.s_hpt;

    if roll_em(mp, &raw mut player, std::ptr::null_mut(), FALSE) != 0 {
        if (*thing_t(mp)).t_type != b'I' as c_char {
            if has_hit != 0 {
                addmsg_str(".  ");
            }
            hit(mname, std::ptr::null_mut(), FALSE);
        } else if has_hit != 0 {
            endmsg();
        }
        has_hit = FALSE;

        if (*thing_t(&raw mut player)).t_stats.s_hpt <= 0 {
            death((*thing_t(mp)).t_type);
        } else if kamikaze == 0 {
            let damage_dealt = oldhp - (*thing_t(&raw mut player)).t_stats.s_hpt;
            if damage_dealt > max_hit {
                max_hit = damage_dealt;
            }
            if (*thing_t(&raw mut player)).t_stats.s_hpt <= max_hit {
                to_death = FALSE;
            }
        }

        if !on_p(mp, ISCANC) {
            let mtype = (*thing_t(mp)).t_type;
            if mtype == b'A' as c_char {
                // Aquator: corrode armor
                rust_armor(cur_armor);
            } else if mtype == b'I' as c_char {
                // Ice monster: freeze player
                (*thing_t(&raw mut player)).t_flags &= !ISRUN;
                if no_command == 0 {
                    addmsg_str("you are frozen");
                    if terse == 0 {
                        addmsg_str(&format!(" by the {}", CStr::from_ptr(mname).to_string_lossy()));
                    }
                    endmsg();
                }
                no_command += rnd(2) + 2;
                if no_command > BORE_LEVEL {
                    death(b'h' as c_char);
                }
            } else if mtype == b'R' as c_char {
                // Rattlesnake: poisonous bite
                if save(VS_POISON) == 0 {
                    if !iswearing(R_SUSTSTR) {
                        chg_str(-1);
                        if terse == 0 {
                            msg_str("you feel a bite in your leg and now feel weaker");
                        } else {
                            msg_str("a bite has weakened you");
                        }
                    } else if to_death == 0 {
                        if terse == 0 {
                            msg_str("a bite momentarily weakens you");
                        } else {
                            msg_str("bite has no effect");
                        }
                    }
                }
            } else if mtype == b'W' as c_char || mtype == b'V' as c_char {
                // Wraith / Vampire: drain energy or max HP
                let threshold = if mtype == b'W' as c_char { 15 } else { 30 };
                if rnd(100) < threshold {
                    let fewer;
                    if mtype == b'W' as c_char {
                        let pstats = &mut (*thing_t(&raw mut player)).t_stats;
                        if pstats.s_exp == 0 {
                            death(b'W' as c_char);
                        }
                        pstats.s_lvl -= 1;
                        if pstats.s_lvl == 0 {
                            pstats.s_exp = 0;
                            pstats.s_lvl = 1;
                        } else {
                            pstats.s_exp = e_levels[(pstats.s_lvl - 1) as usize] + 1;
                        }
                        fewer = roll(1, 10);
                    } else {
                        fewer = roll(1, 3);
                    }
                    {
                        let pstats = &mut (*thing_t(&raw mut player)).t_stats;
                        pstats.s_hpt -= fewer;
                        pstats.s_maxhp -= fewer;
                        if pstats.s_hpt <= 0 {
                            pstats.s_hpt = 1;
                        }
                        if pstats.s_maxhp <= 0 {
                            death(mtype);
                        }
                    }
                    msg_str("you suddenly feel weaker");
                }
            } else if mtype == b'F' as c_char {
                // Venus flytrap: holds the player, deals ongoing damage
                (*thing_t(&raw mut player)).t_flags |= ISHELD;
                vf_hit += 1;
                sprintf(
                    monsters[(b'F' as usize) - (b'A' as usize)].m_stats.s_dmg.as_mut_ptr(),
                    c"%dx1".as_ptr(),
                    vf_hit,
                );
                (*thing_t(&raw mut player)).t_stats.s_hpt -= 1;
                if (*thing_t(&raw mut player)).t_stats.s_hpt <= 0 {
                    death(b'F' as c_char);
                }
            } else if mtype == b'L' as c_char {
                // Leprechaun: steals gold
                let lastpurse = purse;
                purse -= rnd(50 + 10 * level) + 2; // GOLDCALC
                if save(VS_MAGIC) == 0 {
                    let g = rnd(50 + 10 * level) + 2;
                    purse -= g + g + g + g;
                }
                if purse < 0 {
                    purse = 0;
                }
                let mp_pos = (*thing_t(mp)).t_pos;
                remove_mon(&(*thing_t(mp)).t_pos as *const CCoord as *mut CCoord, mp, FALSE);
                if purse != lastpurse {
                    msg_str("your purse feels lighter");
                }
                // mp is now dangling; fall out of the if-chain cleanly
                count = 0;
                status();
                return -1;
            } else if mtype == b'N' as c_char {
                // Nymph: steals a magic item
                let mut steal: *mut CThing = std::ptr::null_mut();
                let mut nobj: c_int = 0;
                let mut obj = (*thing_t(&raw mut player)).t_pack;
                while !obj.is_null() {
                    let obj_next = (*thing_t(obj)).l_next;
                    if obj != cur_armor
                        && obj != cur_weapon
                        && obj != cur_ring[LEFT]
                        && obj != cur_ring[RIGHT]
                        && is_magic_item(obj) != 0
                    {
                        nobj += 1;
                        if rnd(nobj) == 0 {
                            steal = obj;
                        }
                    }
                    obj = obj_next;
                }
                if !steal.is_null() {
                    remove_mon(
                        &(*thing_t(mp)).t_pos as *const CCoord as *mut CCoord,
                        moat((*thing_t(mp)).t_pos.y, (*thing_t(mp)).t_pos.x),
                        FALSE,
                    );
                    leave_pack(steal, FALSE, FALSE);
                    msg_str(&format!(
                        "she stole {}!",
                        CStr::from_ptr(inv_name(steal, TRUE)).to_string_lossy()
                    ));
                    discard(steal);
                    count = 0;
                    status();
                    return -1;
                }
            }
        }
    } else if (*thing_t(mp)).t_type != b'I' as c_char {
        // Miss branch
        if has_hit != 0 {
            addmsg_str(".  ");
            has_hit = FALSE;
        }
        if (*thing_t(mp)).t_type == b'F' as c_char {
            (*thing_t(&raw mut player)).t_stats.s_hpt -= vf_hit;
            if (*thing_t(&raw mut player)).t_stats.s_hpt <= 0 {
                death((*thing_t(mp)).t_type);
            }
        }
        miss(mname, std::ptr::null_mut(), FALSE);
    }

    if fight_flush != 0 && to_death == 0 {
        flush_type();
    }
    count = 0;
    status();
    0
}

/// Helper: forward to is_magic C function (from potions.rs).
unsafe fn is_magic_item(obj: *mut CThing) -> c_uchar {
    extern "C" {
        fn is_magic(obj: *mut CThing) -> c_uchar;
    }
    is_magic(obj)
}

/// set_mname:
/// Return the monster name for the given monster.
#[no_mangle]
pub unsafe extern "C" fn set_mname(tp: *mut CThing) -> *mut c_char {
    if see_monst(tp) == 0 && !on_p(&raw mut player, SEEMONST) {
        return if terse != 0 {
            c"it".as_ptr() as *mut c_char
        } else {
            c"something".as_ptr() as *mut c_char
        };
    }

    // Ensure "the " prefix is initialised.
    if !MNAME_INIT {
        MNAME_BUF[0] = b't' as c_char;
        MNAME_BUF[1] = b'h' as c_char;
        MNAME_BUF[2] = b'e' as c_char;
        MNAME_BUF[3] = b' ' as c_char;
        MNAME_INIT = true;
    }

    let mname: *mut c_char;
    if on_p(&raw mut player, ISHALU) {
        cur::move_((*thing_t(tp)).t_pos.y, (*thing_t(tp)).t_pos.x);
        let ch = toascii(cur::inch() as c_int);
        let idx = if isupper(ch) != 0 {
            (ch - b'A' as c_int) as usize
        } else {
            rnd(26) as usize
        };
        mname = monsters[idx].m_name;
    } else {
        let idx = ((*thing_t(tp)).t_type as u8).wrapping_sub(b'A') as usize;
        mname = monsters[idx].m_name;
    }

    // Copy into static buffer starting at offset 4 ("the ").
    strcpy(MNAME_BUF[4..].as_mut_ptr(), mname);
    MNAME_BUF.as_mut_ptr()
}

/// swing:
/// Returns true (1) if the swing hits.
#[no_mangle]
pub unsafe extern "C" fn swing(at_lvl: c_int, op_arm: c_int, wplus: c_int) -> c_int {
    let res  = rnd(20);
    let need = (20 - at_lvl) - op_arm;
    (res + wplus >= need) as c_int
}

/// roll_em:
/// Roll several attacks and apply damage.
#[no_mangle]
pub unsafe extern "C" fn roll_em(
    thatt: *mut CThing,
    thdef: *mut CThing,
    weap:  *mut CThing,
    hurl:  c_uchar,
) -> c_int {
    let cp: *mut c_char;
    let hplus: c_int;
    let dplus: c_int;

    if weap.is_null() {
        cp    = (*thing_t(thatt)).t_stats.s_dmg.as_mut_ptr();
        dplus = 0;
        hplus = 0;
    } else {
        let mut hp = (*thing_o(weap)).o_hplus;
        let mut dp = (*thing_o(weap)).o_dplus;
        if weap == cur_weapon {
            if isring(LEFT, R_ADDDAM) {
                dp += (*thing_o(cur_ring[LEFT])).o_arm;
            } else if isring(LEFT, R_ADDHIT) {
                hp += (*thing_o(cur_ring[LEFT])).o_arm;
            }
            if isring(RIGHT, R_ADDDAM) {
                dp += (*thing_o(cur_ring[RIGHT])).o_arm;
            } else if isring(RIGHT, R_ADDHIT) {
                hp += (*thing_o(cur_ring[RIGHT])).o_arm;
            }
        }
        if hurl != 0 {
            if ((*thing_o(weap)).o_flags & ISMISL) != 0
                && !cur_weapon.is_null()
                && (*thing_o(cur_weapon)).o_which == (*thing_o(weap)).o_launch
            {
                let hurldmg_ptr = (*thing_o(weap)).o_hurldmg.as_mut_ptr();
                return roll_em_inner(thatt, thdef, hurldmg_ptr,
                    hp + (*thing_o(cur_weapon)).o_hplus,
                    dp + (*thing_o(cur_weapon)).o_dplus);
            } else if (*thing_o(weap)).o_launch < 0 {
                let hurldmg_ptr = (*thing_o(weap)).o_hurldmg.as_mut_ptr();
                return roll_em_inner(thatt, thdef, hurldmg_ptr, hp, dp);
            }
        }
        cp    = (*thing_o(weap)).o_damage.as_mut_ptr();
        hplus = hp;
        dplus = dp;
    }

    roll_em_inner(thatt, thdef, cp, hplus, dplus)
}

/// Inner roll loop, factored out to handle the hurldmg shortcut cleanly.
unsafe fn roll_em_inner(
    thatt: *mut CThing,
    thdef: *mut CThing,
    mut cp:   *mut c_char,
    hplus: c_int,
    dplus: c_int,
) -> c_int {
    // If the defender is not running (asleep or held), attacker gets +4 to hit.
    let hplus = hplus + if !on_p(thdef, ISRUN) { 4 } else { 0 };

    // Defender's armor class
    let def_arm_base = (*thing_t(thdef)).t_stats.s_arm;
    let player_is_def = thdef as *const u8 == (&raw const player) as *const u8;
    let mut def_arm = def_arm_base;
    if player_is_def {
        if !cur_armor.is_null() {
            def_arm = (*thing_o(cur_armor)).o_arm;
        }
        if isring(LEFT, R_PROTECT) {
            def_arm -= (*thing_o(cur_ring[LEFT])).o_arm;
        }
        if isring(RIGHT, R_PROTECT) {
            def_arm -= (*thing_o(cur_ring[RIGHT])).o_arm;
        }
    }

    let att_str = (*thing_t(thatt)).t_stats.s_str as usize;
    let att_lvl = (*thing_t(thatt)).t_stats.s_lvl;
    let str_idx = att_str.min(STR_PLUS.len() - 1);
    let mut did_hit = 0i32;

    loop {
        if cp.is_null() || *cp == 0 {
            break;
        }
        let ndice = atoi(cp);
        cp = strchr(cp, b'x' as c_int);
        if cp.is_null() {
            break;
        }
        cp = cp.add(1);
        let nsides = atoi(cp);
        if swing(att_lvl, def_arm, hplus + STR_PLUS[str_idx]) != 0 {
            let proll  = roll(ndice, nsides);
            let damage = dplus + proll + ADD_DAM[str_idx];
            (*thing_t(thdef)).t_stats.s_hpt -= if damage > 0 { damage } else { 0 };
            did_hit = 1;
        }
        cp = strchr(cp, b'/' as c_int);
        if cp.is_null() {
            break;
        }
        cp = cp.add(1);
    }
    did_hit
}

/// prname:
/// The print name of a combatant.
#[no_mangle]
pub unsafe extern "C" fn prname(mname: *const c_char, upper: c_uchar) -> *mut c_char {
    PRNAME_BUF[0] = 0;
    if mname.is_null() {
        strcpy(PRNAME_BUF.as_mut_ptr(), c"you".as_ptr());
    } else {
        strcpy(PRNAME_BUF.as_mut_ptr(), mname);
    }
    if upper != 0 && PRNAME_BUF[0] != 0 {
        PRNAME_BUF[0] = toupper(PRNAME_BUF[0] as c_uchar as c_int) as c_char;
    }
    PRNAME_BUF.as_mut_ptr()
}

/// thunk:
/// A missile hits a monster.
#[no_mangle]
pub unsafe extern "C" fn thunk(weap: *mut CThing, mname: *const c_char, noend: c_uchar) {
    if to_death != 0 {
        return;
    }
    if (*thing_o(weap)).o_type == WEAPON {
        addmsg_str(&format!(
            "the {} hits ",
            CStr::from_ptr(weap_info[(*thing_o(weap)).o_which as usize].oi_name).to_string_lossy()
        ));
    } else {
        addmsg_str("you hit ");
    }
    addmsg_str(&CStr::from_ptr(mname).to_string_lossy());
    if noend == 0 {
        endmsg();
    }
}

/// hit:
/// Print a message to indicate a successful hit.
#[no_mangle]
pub unsafe extern "C" fn hit(er: *const c_char, ee: *const c_char, noend: c_uchar) {
    if to_death != 0 {
        return;
    }
    addmsg_str(&CStr::from_ptr(prname(er, TRUE)).to_string_lossy());
    let s: *const c_char = if terse != 0 {
        c" hit".as_ptr()
    } else {
        let mut i = rnd(4) as usize;
        if !er.is_null() {
            i += 4;
        }
        h_names[i]
    };
    addmsg_str(&CStr::from_ptr(s).to_string_lossy());
    if terse == 0 {
        addmsg_str(&CStr::from_ptr(prname(ee, FALSE)).to_string_lossy());
    }
    if noend == 0 {
        endmsg();
    }
}

/// miss:
/// Print a message to indicate a poor swing.
#[no_mangle]
pub unsafe extern "C" fn miss(er: *const c_char, ee: *const c_char, noend: c_uchar) {
    if to_death != 0 {
        return;
    }
    addmsg_str(&CStr::from_ptr(prname(er, TRUE)).to_string_lossy());
    let i: usize = if terse != 0 {
        if !er.is_null() { 4 } else { 0 }
    } else {
        let base = rnd(4) as usize;
        if !er.is_null() { base + 4 } else { base }
    };
    addmsg_str(&CStr::from_ptr(m_names[i]).to_string_lossy());
    if terse == 0 {
        addmsg_str(&format!(" {}", CStr::from_ptr(prname(ee, FALSE)).to_string_lossy()));
    }
    if noend == 0 {
        endmsg();
    }
}

/// bounce:
/// A missile misses a monster.
#[no_mangle]
pub unsafe extern "C" fn bounce(weap: *mut CThing, mname: *const c_char, noend: c_uchar) {
    if to_death != 0 {
        return;
    }
    if (*thing_o(weap)).o_type == WEAPON {
        addmsg_str(&format!(
            "the {} misses ",
            CStr::from_ptr(weap_info[(*thing_o(weap)).o_which as usize].oi_name).to_string_lossy()
        ));
    } else {
        addmsg_str("you missed ");
    }
    addmsg_str(&CStr::from_ptr(mname).to_string_lossy());
    if noend == 0 {
        endmsg();
    }
}

/// remove_mon:
/// Remove a monster from the screen.
#[no_mangle]
pub unsafe extern "C" fn remove_mon(mp: *mut CCoord, tp: *mut CThing, waskill: c_uchar) {
    let mut obj = (*thing_t(tp)).t_pack;
    while !obj.is_null() {
        let nexti = (*thing_t(obj)).l_next;
        (*thing_o(obj)).o_pos = (*thing_t(tp)).t_pos;
        _detach(
            &mut (*thing_t(tp)).t_pack as *mut *mut CThing,
            obj,
        );
        if waskill != 0 {
            fall(obj, FALSE);
        } else {
            discard(obj);
        }
        obj = nexti;
    }
    set_moat((*mp).y, (*mp).x, std::ptr::null_mut());
    // Re-draw the underlying character.
    let oldch = (*thing_t(tp)).t_oldch;
    cur::mvaddch((*mp).y, (*mp).x, oldch as c_uchar as c_uint);

    _detach(&raw mut mlist as *mut *mut CThing, tp);

    if on_p(tp, ISTARGET) {
        kamikaze = FALSE;
        to_death = FALSE;
        if fight_flush != 0 {
            flush_type();
        }
    }
    discard(tp);
}

/// killed:
/// Called to put a monster to death.
#[no_mangle]
pub unsafe extern "C" fn killed(tp: *mut CThing, pr: c_uchar) {
    (*thing_t(&raw mut player)).t_stats.s_exp += (*thing_t(tp)).t_stats.s_exp;

    let mtype = (*thing_t(tp)).t_type;

    if mtype == b'F' as c_char {
        (*thing_t(&raw mut player)).t_flags &= !ISHELD;
        vf_hit = 0;
        // Reset damage string to "000x0"
        let dmg = monsters[(b'F' as usize) - (b'A' as usize)].m_stats.s_dmg.as_mut_ptr();
        strcpy(dmg, c"000x0".as_ptr());
    } else if mtype == b'L' as c_char {
        let mut gold_pos = CCoord { x: 0, y: 0 };
        let tp_room = (*thing_t(tp)).t_room;
        if !tp_room.is_null() && fallpos(&mut (*thing_t(tp)).t_pos, &mut (*tp_room).r_gold) != 0
            && level >= max_level
        {
            let gold = new_item();
            (*thing_o(gold)).o_type = GOLD;
            // o_goldval is #define'd to o_arm
            (*thing_o(gold)).o_arm = rnd(50 + 10 * level) + 2; // GOLDCALC
            if save(VS_MAGIC) != 0 {
                let extra = rnd(50 + 10 * level) + 2;
                (*thing_o(gold)).o_arm += extra + extra + extra + extra;
            }
            _attach(&mut (*thing_t(tp)).t_pack as *mut *mut CThing, gold);
        }
    }

    let mname = set_mname(tp);
    remove_mon(&mut (*thing_t(tp)).t_pos, tp, TRUE);

    if pr != 0 {
        if has_hit != 0 {
            addmsg_str(".  Defeated ");
            has_hit = FALSE;
        } else {
            if terse == 0 {
                addmsg_str("you have ");
            }
            addmsg_str("defeated ");
        }
        msg_str(&CStr::from_ptr(mname).to_string_lossy());
    }

    check_level();
    if fight_flush != 0 {
        flush_type();
    }
}
