//! All the fighting gets done here.
//!
//! Ported from `src/c/fight.c` to Rust.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use crate::rnd::rnd;

use crate::entity::chase::{runto, see_monst};
use crate::entity::monsters::save;
use crate::game::PLAYER;
use crate::init::pick_color;
use crate::item::armor::rust_armor;
use crate::item::pack::leave_pack;
use crate::item::potions::is_magic;
use crate::misc::{check_level, chg_str, choose_str};
use crate::rip::death;
use crate::ui::output::{addmsg_str, endmsg, msg_str, status};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};

use crate::entity::player::{MonsterFlags, ObjectFlags, Thing, ThingMonster, ThingObject};
use crate::globals::{monsters, weap_info};
use crate::item::rings::RingType;
use crate::entity::player::{attach_pack, detach_pack, discard};
use crate::item::arena::new_item;
use crate::item::things::inv_name;
use crate::item::weapons::{fall, fallpos};
use crate::machdep::flush_type;
use crate::startup::roll;
use crate::ui::output;
use glam::IVec2;

// ─── Constants ────────────────────────────────────────────────────────────────

const MAXSTR: usize = 1024;

// Item types
const WEAPON: c_int = b')' as c_int;
const GOLD: c_int = b'*' as c_int;

// Misc constants
const BORE_LEVEL: c_int = 50;

// Save-vs constants
const VS_POISON: c_int = 0;
const VS_MAGIC: c_int = 0o03;

// ─── Adjustments due to strength ─────────────────────────────────────────────

static STR_PLUS: [c_int; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 3,
];

static ADD_DAM: [c_int; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 3, 3, 4, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 6,
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

static mut MNAME_BUF: [c_char; MAXSTR] = [0; MAXSTR];
static mut MNAME_INIT: bool = false;

// Static name buffer for prname
static mut PRNAME_BUF: [c_char; MAXSTR] = [0; MAXSTR];

unsafe fn copy_str_to_c_buffer(dst: *mut c_char, capacity: usize, text: &str) {
    if capacity == 0 {
        return;
    }

    let bytes = text.as_bytes();
    let copy_len = bytes.len().min(capacity - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), dst, copy_len);
    *dst.add(copy_len) = 0;
}

// ─── Extern C globals ─────────────────────────────────────────────────────────

unsafe extern "C" {
    static mut e_levels: [c_int; 21];

    static mut count: c_int;
    static mut quiet: c_int;
    static mut running: c_uchar;
    static mut to_death: c_uchar;
    static mut kamikaze: c_uchar;
    static mut has_hit: c_uchar;
    static mut terse: c_uchar;
    static mut fight_flush: c_uchar;
    static mut no_command: c_int;
    static mut vf_hit: c_int;
    static mut max_hit: c_int;
    static mut purse: c_int;
    static mut max_level: c_int;
}

// ─── Inline helpers ───────────────────────────────────────────────────────────

#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

#[inline]
unsafe fn on_p(tp: *mut Thing, flag: MonsterFlags) -> bool {
    (*thing_t(tp)).t_flags.contains(flag)
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    PLAYER.has_flag(flag)
}

#[inline]
unsafe fn isring(ring: *mut Thing, ring_type: RingType) -> bool {
    !ring.is_null() && RingType::from_raw((*thing_o(ring)).o_which) == Some(ring_type)
}

#[inline]
unsafe fn iswearing(ring_type: RingType) -> bool {
    isring(PLAYER.left_ring(), ring_type) || isring(PLAYER.right_ring(), ring_type)
}

#[inline]
unsafe fn moat(y: c_int, x: c_int) -> *mut Thing {
    crate::game::monster_at(y, x)
}

#[inline]
unsafe fn set_moat(y: c_int, x: c_int, val: *mut Thing) {
    crate::game::set_monster(y, x, val);
}

// ─── Exported functions ───────────────────────────────────────────────────────

/// fight:
/// The player attacks the monster.
#[no_mangle]
pub unsafe extern "C" fn fight(mp: *mut IVec2, weap: *mut Thing, thrown: c_uchar) -> c_int {
    let tp = moat((*mp).y, (*mp).x);

    // Since we are fighting, things are not quiet — no healing.
    count = 0;
    quiet = 0;
    runto(mp);

    // Let him know it was really a xeroc (if it was one).
    let mut ch: c_char = b'\0' as c_char;
    if (*thing_t(tp)).t_type == b'X'
        && (*thing_t(tp)).t_disguise != b'X'
        && !player_has(MonsterFlags::BLIND)
    {
        (*thing_t(tp)).t_disguise = b'X';
        if player_has(MonsterFlags::HALU) {
            ch = (rnd(26) + b'A' as c_int) as c_char;
            output::write_glyph_at(
                IVec2::new((*thing_t(tp)).t_pos.x, (*thing_t(tp)).t_pos.y),
                (ch as u8) as char,
            );
        }
        msg_str(choose_str(
            "heavy!  That's a nasty critter!",
            "wait!  That's a xeroc!",
        ));
        if thrown == 0 {
            return false as c_uchar as c_int;
        }
    }

    let mname = set_mname(tp);
    let mut did_hit = false as c_uchar;
    has_hit = if terse != 0 && to_death == 0 {
        true as c_uchar
    } else {
        false as c_uchar
    };

    if roll_em_hero_to(tp, weap, thrown) != 0 {
        did_hit = false as c_uchar;
        if thrown != 0 {
            thunk(weap, mname, terse);
        } else {
            hit(std::ptr::null_mut(), mname, terse);
        }
        if player_has(MonsterFlags::CANHUH) {
            did_hit = true as c_uchar;
            (*thing_t(tp)).t_flags.insert(MonsterFlags::HUH);
            PLAYER.remove_flag(MonsterFlags::CANHUH);
            endmsg();
            has_hit = false as c_uchar;
            msg_str(&format!("your hands stop glowing {}", pick_color("red")));
        }
        if (*thing_t(tp)).t_stats.hit_points <= 0 {
            killed(tp, true as c_uchar);
        } else if did_hit != 0 && !player_has(MonsterFlags::BLIND) {
            msg_str(&format!(
                "{} appears confused",
                CStr::from_ptr(mname).to_string_lossy()
            ));
        }
        did_hit = true as c_uchar;
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
pub unsafe extern "C" fn attack(mp: *mut Thing) -> c_int {
    // Stop running / healing.
    running = false as c_uchar;
    count = 0;
    quiet = 0;

    if to_death != 0 && !on_p(mp, MonsterFlags::TARGET) {
        to_death = false as c_uchar;
        kamikaze = false as c_uchar;
    }

    if (*thing_t(mp)).t_type == b'X'
        && (*thing_t(mp)).t_disguise != b'X'
        && !player_has(MonsterFlags::BLIND)
    {
        (*thing_t(mp)).t_disguise = b'X';
        if player_has(MonsterFlags::HALU) {
            output::write_glyph_at(
                IVec2::new((*thing_t(mp)).t_pos.x, (*thing_t(mp)).t_pos.y),
                (rnd(26) as u8 + b'A') as char,
            );
        }
    }

    let mname = set_mname(mp);
    let oldhp = PLAYER.stats().hit_points;

    if roll_em_to_hero(mp, std::ptr::null_mut(), false as c_uchar) != 0 {
        if (*thing_t(mp)).t_type != b'I' {
            if has_hit != 0 {
                addmsg_str(".  ");
            }
            hit(mname, std::ptr::null_mut(), false as c_uchar);
        } else if has_hit != 0 {
            endmsg();
        }
        has_hit = false as c_uchar;

        if PLAYER.stats().hit_points <= 0 {
            death((*thing_t(mp)).t_type as c_char);
        } else if kamikaze == 0 {
            let damage_dealt = oldhp - PLAYER.stats().hit_points;
            if damage_dealt > max_hit {
                max_hit = damage_dealt;
            }
            if PLAYER.stats().hit_points <= max_hit {
                to_death = false as c_uchar;
            }
        }

        if !on_p(mp, MonsterFlags::CANCELLED) {
            let mtype = (*thing_t(mp)).t_type;
            if mtype == b'A' {
                // Aquator: corrode armor
                rust_armor(PLAYER.armor());
            } else if mtype == b'I' {
                // Ice monster: freeze player
                PLAYER.remove_flag(MonsterFlags::RUN);
                if no_command == 0 {
                    addmsg_str("you are frozen");
                    if terse == 0 {
                        addmsg_str(&format!(
                            " by the {}",
                            CStr::from_ptr(mname).to_string_lossy()
                        ));
                    }
                    endmsg();
                }
                no_command += rnd(2) + 2;
                if no_command > BORE_LEVEL {
                    death(b'h' as c_char);
                }
            } else if mtype == b'R' {
                // Rattlesnake: poisonous bite
                if save(VS_POISON) == 0 {
                    if !iswearing(RingType::SustainStrength) {
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
            } else if mtype == b'W' || mtype == b'V' {
                // Wraith / Vampire: drain energy or max HP
                let threshold = if mtype == b'W' { 15 } else { 30 };
                if rnd(100) < threshold {
                    let fewer;
                    if mtype == b'W' {
                        if PLAYER.stats().experience == 0 {
                            death(b'W' as c_char);
                        }
                        PLAYER.with_stats_mut(|pstats| {
                            pstats.level -= 1;
                            if pstats.level == 0 {
                                pstats.experience = 0;
                                pstats.level = 1;
                            } else {
                                pstats.experience = e_levels[(pstats.level - 1) as usize] + 1;
                            }
                        });
                        fewer = roll(1, 10);
                    } else {
                        fewer = roll(1, 3);
                    }
                    {
                        let mut dead = false;
                        PLAYER.with_stats_mut(|pstats| {
                            pstats.hit_points -= fewer;
                            pstats.max_hit_points -= fewer;
                            if pstats.hit_points <= 0 {
                                pstats.hit_points = 1;
                            }
                            if pstats.max_hit_points <= 0 {
                                dead = true;
                            }
                        });
                        if dead {
                            death(mtype as c_char);
                        }
                    }
                    msg_str("you suddenly feel weaker");
                }
            } else if mtype == b'F' {
                // Venus flytrap: holds the player, deals ongoing damage
                PLAYER.add_flag(MonsterFlags::HELD);
                vf_hit += 1;
                let text = format!("{}x1", vf_hit);
                let bytes = text.as_bytes();
                let damage = &mut monsters[(b'F' as usize) - (b'A' as usize)].m_stats.damage;
                let copy_len = bytes.len().min(damage.len() - 1);
                damage[..copy_len].copy_from_slice(&bytes[..copy_len]);
                damage[copy_len] = 0;
                PLAYER.with_stats_mut(|stats| stats.hit_points -= 1);
                if PLAYER.stats().hit_points <= 0 {
                    death(b'F' as c_char);
                }
            } else if mtype == b'L' {
                // Leprechaun: steals gold
                let level = crate::game::current_depth();
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
                remove_mon(
                    &(*thing_t(mp)).t_pos as *const IVec2 as *mut IVec2,
                    mp,
                    false as c_uchar,
                );
                if purse != lastpurse {
                    msg_str("your purse feels lighter");
                }
                // mp is now dangling; fall out of the if-chain cleanly
                count = 0;
                status();
                return -1;
            } else if mtype == b'N' {
                // Nymph: steals a magic item
                let mut steal: *mut Thing = std::ptr::null_mut();
                let mut nobj: c_int = 0;
                let mut obj = PLAYER.pack();
                while !obj.is_null() {
                    let obj_next = crate::entity::player::thing_next(obj);
                    if obj != PLAYER.armor()
                        && obj != PLAYER.weapon()
                        && obj != PLAYER.left_ring()
                        && obj != PLAYER.right_ring()
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
                        &(*thing_t(mp)).t_pos as *const IVec2 as *mut IVec2,
                        moat((*thing_t(mp)).t_pos.y, (*thing_t(mp)).t_pos.x),
                        false as c_uchar,
                    );
                    leave_pack(steal, false as c_uchar, false as c_uchar);
                    msg_str(&format!("she stole {}!", inv_name(steal, true as c_uchar)));
                    discard(steal);
                    count = 0;
                    status();
                    return -1;
                }
            }
        }
    } else if (*thing_t(mp)).t_type != b'I' {
        // Miss branch
        if has_hit != 0 {
            addmsg_str(".  ");
            has_hit = false as c_uchar;
        }
        if (*thing_t(mp)).t_type == b'F' {
            PLAYER.with_stats_mut(|stats| stats.hit_points -= vf_hit);
            if PLAYER.stats().hit_points <= 0 {
                death((*thing_t(mp)).t_type as c_char);
            }
        }
        miss(mname, std::ptr::null_mut(), false as c_uchar);
    }

    if fight_flush != 0 && to_death == 0 {
        flush_type();
    }
    count = 0;
    status();
    0
}

/// Helper: forward to is_magic C function (from potions.rs).
unsafe fn is_magic_item(obj: *mut Thing) -> c_uchar {
    is_magic(obj)
}

/// set_mname:
/// Return the monster name for the given monster.
#[no_mangle]
pub unsafe extern "C" fn set_mname(tp: *mut Thing) -> *mut c_char {
    if see_monst(tp) == 0 && !player_has(MonsterFlags::SEEMONST) {
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

    let mname: &'static str;
    if player_has(MonsterFlags::HALU) {
        output::move_cursor(IVec2::new((*thing_t(tp)).t_pos.x, (*thing_t(tp)).t_pos.y));
        let ch = (output::glyph_at_cursor() as u8).to_ascii_uppercase() as c_int;
        let idx = if (ch as u8).is_ascii_uppercase() {
            (ch - b'A' as c_int) as usize
        } else {
            rnd(26) as usize
        };
        mname = monsters[idx].m_name;
    } else {
        let idx = (*thing_t(tp)).t_type.wrapping_sub(b'A') as usize;
        mname = monsters[idx].m_name;
    }

    let buffer = std::ptr::addr_of_mut!(MNAME_BUF).cast::<c_char>();
    copy_str_to_c_buffer(buffer.add(4), MAXSTR - 4, mname);
    buffer
}

/// swing:
/// Returns true (1) if the swing hits.
#[no_mangle]
pub unsafe extern "C" fn swing(at_lvl: c_int, op_arm: c_int, wplus: c_int) -> c_int {
    let res = rnd(20);
    let need = (20 - at_lvl) - op_arm;
    (res + wplus >= need) as c_int
}

/// roll_em:
/// Roll several attacks and apply damage.
#[no_mangle]
pub unsafe extern "C" fn roll_em(
    thatt: *mut Thing,
    thdef: *mut Thing,
    weap: *mut Thing,
    hurl: c_uchar,
) -> c_int {
    let att_stats = (*thing_t(thatt)).t_stats;
    roll_em_impl(&att_stats, thdef, false, weap, hurl)
}

/// The hero attacks monster `thdef`.
unsafe fn roll_em_hero_to(thdef: *mut Thing, weap: *mut Thing, hurl: c_uchar) -> c_int {
    let att_stats = PLAYER.stats();
    roll_em_impl(&att_stats, thdef, false, weap, hurl)
}

/// Monster `thatt` attacks the hero.
unsafe fn roll_em_to_hero(thatt: *mut Thing, weap: *mut Thing, hurl: c_uchar) -> c_int {
    let att_stats = (*thing_t(thatt)).t_stats;
    roll_em_impl(&att_stats, std::ptr::null_mut(), true, weap, hurl)
}

/// Shared roll_em implementation. `def_is_hero` selects the hero's armor class.
unsafe fn roll_em_impl(
    att_stats: &crate::entity::player::Stats,
    thdef: *mut Thing,
    def_is_hero: bool,
    weap: *mut Thing,
    hurl: c_uchar,
) -> c_int {
    let damage: [u8; 8];
    let hplus: c_int;
    let dplus: c_int;

    if weap.is_null() {
        let src = &att_stats.damage;
        let mut buf = [0u8; 8];
        let n = src.len().min(8);
        buf[..n].copy_from_slice(&src[..n]);
        damage = buf;
        dplus = 0;
        hplus = 0;
    } else {
        let mut hp = (*thing_o(weap)).o_hplus;
        let mut dp = (*thing_o(weap)).o_dplus;
        if weap == PLAYER.weapon() {
            if isring(PLAYER.left_ring(), RingType::AddDamage) {
                dp += (*thing_o(PLAYER.left_ring())).o_arm;
            } else if isring(PLAYER.left_ring(), RingType::AddHit) {
                hp += (*thing_o(PLAYER.left_ring())).o_arm;
            }
            if isring(PLAYER.right_ring(), RingType::AddDamage) {
                dp += (*thing_o(PLAYER.right_ring())).o_arm;
            } else if isring(PLAYER.right_ring(), RingType::AddHit) {
                hp += (*thing_o(PLAYER.right_ring())).o_arm;
            }
        }
        if hurl != 0 {
            if (*thing_o(weap)).o_flags.contains(ObjectFlags::MISL)
                && !PLAYER.weapon().is_null()
                && (*thing_o(PLAYER.weapon())).o_which == (*thing_o(weap)).o_launch
            {
                return roll_em_inner(
                    att_stats,
                    thdef,
                    def_is_hero,
                    &(*thing_o(weap)).o_hurldmg,
                    hp + (*thing_o(PLAYER.weapon())).o_hplus,
                    dp + (*thing_o(PLAYER.weapon())).o_dplus,
                );
            } else if (*thing_o(weap)).o_launch < 0 {
                return roll_em_inner(att_stats, thdef, def_is_hero, &(*thing_o(weap)).o_hurldmg, hp, dp);
            }
        }
        damage = (*thing_o(weap)).o_damage;
        hplus = hp;
        dplus = dp;
    }

    roll_em_inner(att_stats, thdef, def_is_hero, &damage, hplus, dplus)
}

/// Parses a legacy damage specification like `"2x4"` or `"1x8/1x8/3x10"`
/// into `(ndice, nsides)` pairs.
fn parse_damage(spec: &[u8]) -> Vec<(i32, i32)> {
    let text: String = spec
        .iter()
        .take_while(|b| **b != 0)
        .map(|b| *b as char)
        .collect();
    let mut out = Vec::new();
    for part in text.split('/') {
        let mut it = part.split('x');
        if let (Some(a), Some(b)) = (it.next(), it.next()) {
            if let (Ok(n), Ok(s)) = (a.trim().parse::<i32>(), b.trim().parse::<i32>()) {
                out.push((n, s));
            }
        }
    }
    out
}

/// Inner roll loop, factored out to handle the hurldmg shortcut cleanly.
unsafe fn roll_em_inner(
    att_stats: &crate::entity::player::Stats,
    thdef: *mut Thing,
    def_is_hero: bool,
    damage_spec: &[u8],
    hplus: c_int,
    dplus: c_int,
) -> c_int {
    // If the defender is not running (asleep or held), attacker gets +4 to hit.
    let def_running = if def_is_hero {
        player_has(MonsterFlags::RUN)
    } else {
        on_p(thdef, MonsterFlags::RUN)
    };
    let hplus = hplus + if !def_running { 4 } else { 0 };

    // Defender's armor class
    let mut def_arm = if def_is_hero {
        PLAYER.stats().armor
    } else {
        (*thing_t(thdef)).t_stats.armor
    };
    if def_is_hero {
        if !PLAYER.armor().is_null() {
            def_arm = (*thing_o(PLAYER.armor())).o_arm;
        }
        if isring(PLAYER.left_ring(), RingType::Protection) {
            def_arm -= (*thing_o(PLAYER.left_ring())).o_arm;
        }
        if isring(PLAYER.right_ring(), RingType::Protection) {
            def_arm -= (*thing_o(PLAYER.right_ring())).o_arm;
        }
    }

    let att_str = att_stats.strength as usize;
    let att_lvl = att_stats.level;
    let str_idx = att_str.min(STR_PLUS.len() - 1);
    let mut did_hit = 0i32;
    let mut total_damage = 0i32;

    for (ndice, nsides) in parse_damage(damage_spec) {
        if swing(att_lvl, def_arm, hplus + STR_PLUS[str_idx]) != 0 {
            let proll = roll(ndice, nsides);
            let damage = dplus + proll + ADD_DAM[str_idx];
            total_damage += if damage > 0 { damage } else { 0 };
            did_hit = 1;
        }
    }

    if did_hit != 0 {
        if def_is_hero {
            PLAYER.with_stats_mut(|stats| stats.hit_points -= total_damage);
        } else {
            (*thing_t(thdef)).t_stats.hit_points -= total_damage;
        }
    }
    did_hit
}

/// prname:
/// The print name of a combatant.
#[no_mangle]
pub unsafe extern "C" fn prname(mname: *const c_char, upper: c_uchar) -> *mut c_char {
    let text: &str = if mname.is_null() {
        "you"
    } else {
        // Borrow the incoming C string without allocating.
        let mut len = 0usize;
        while *mname.add(len) != 0 {
            len += 1;
        }
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(
            mname as *const u8,
            len,
        ))
    };
    let buffer = std::ptr::addr_of_mut!(PRNAME_BUF).cast::<c_char>();
    copy_str_to_c_buffer(buffer, MAXSTR, text);
    if upper != 0 && PRNAME_BUF[0] != 0 {
        let first = (PRNAME_BUF[0] as u8).to_ascii_uppercase();
        PRNAME_BUF[0] = first as c_char;
    }
    std::ptr::addr_of_mut!(PRNAME_BUF).cast()
}

/// thunk:
/// A missile hits a monster.
#[no_mangle]
pub unsafe extern "C" fn thunk(weap: *mut Thing, mname: *const c_char, noend: c_uchar) {
    if to_death != 0 {
        return;
    }
    if (*thing_o(weap)).o_type == WEAPON {
        addmsg_str(&format!(
            "the {} hits ",
            weap_info[(*thing_o(weap)).o_which as usize].oi_name
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
    addmsg_str(&CStr::from_ptr(prname(er, true as c_uchar)).to_string_lossy());
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
        addmsg_str(&CStr::from_ptr(prname(ee, false as c_uchar)).to_string_lossy());
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
    addmsg_str(&CStr::from_ptr(prname(er, true as c_uchar)).to_string_lossy());
    let i: usize = if terse != 0 {
        if !er.is_null() {
            4
        } else {
            0
        }
    } else {
        let base = rnd(4) as usize;
        if !er.is_null() {
            base + 4
        } else {
            base
        }
    };
    addmsg_str(&CStr::from_ptr(m_names[i]).to_string_lossy());
    if terse == 0 {
        addmsg_str(&format!(
            " {}",
            CStr::from_ptr(prname(ee, false as c_uchar)).to_string_lossy()
        ));
    }
    if noend == 0 {
        endmsg();
    }
}

/// bounce:
/// A missile misses a monster.
#[no_mangle]
pub unsafe extern "C" fn bounce(weap: *mut Thing, mname: *const c_char, noend: c_uchar) {
    if to_death != 0 {
        return;
    }
    if (*thing_o(weap)).o_type == WEAPON {
        addmsg_str(&format!(
            "the {} misses ",
            weap_info[(*thing_o(weap)).o_which as usize].oi_name
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
pub unsafe extern "C" fn remove_mon(mp: *mut IVec2, tp: *mut Thing, waskill: c_uchar) {
    let mut obj = crate::entity::player::thing_pack(tp);
    while !obj.is_null() {
        let nexti = crate::entity::player::thing_next(obj);
        (*thing_o(obj)).o_pos = (*thing_t(tp)).t_pos;
        detach_pack(tp, obj);
        if waskill != 0 {
            fall(obj, false as c_uchar);
        } else {
            discard(obj);
        }
        obj = nexti;
    }
    set_moat((*mp).y, (*mp).x, std::ptr::null_mut());
    // Re-draw the underlying character.
    let oldch = (*thing_t(tp)).t_oldch;
    output::write_glyph_at(IVec2::new((*mp).x, (*mp).y), (oldch as u8) as char);

    if on_p(tp, MonsterFlags::TARGET) {
        kamikaze = false as c_uchar;
        to_death = false as c_uchar;
        if fight_flush != 0 {
            flush_type();
        }
    }
    discard(tp);
}

/// killed:
/// Called to put a monster to death.
#[no_mangle]
pub unsafe extern "C" fn killed(tp: *mut Thing, pr: c_uchar) {
    let gained = (*thing_t(tp)).t_stats.experience;
    PLAYER.with_stats_mut(|stats| stats.experience += gained);

    let mtype = (*thing_t(tp)).t_type;

    if mtype == b'F' {
        PLAYER.remove_flag(MonsterFlags::HELD);
        vf_hit = 0;
        // Reset damage string to "000x0"
        let damage = &mut monsters[(b'F' as usize) - (b'A' as usize)].m_stats.damage;
        damage[..b"000x0\0".len()].copy_from_slice(b"000x0\0");
    } else if mtype == b'L' {
        let tp_room = (*thing_t(tp)).t_room;
        let level = crate::game::current_depth();
        if tp_room.is_some()
            && fallpos(
                &mut (*thing_t(tp)).t_pos,
                crate::game::room_gold_ptr(tp_room),
            ) != 0
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
            attach_pack(tp, gold);
        }
    }

    let mname = set_mname(tp);
    remove_mon(&mut (*thing_t(tp)).t_pos, tp, true as c_uchar);

    if pr != 0 {
        if has_hit != 0 {
            addmsg_str(".  Defeated ");
            has_hit = false as c_uchar;
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

#[cfg(test)]
mod tests {
    use super::{copy_str_to_c_buffer, MAXSTR};
    use std::ffi::CStr;
    use std::os::raw::c_char;

    #[test]
    fn monster_name_copy_is_nul_terminated() {
        let mut buffer = [b'x' as c_char; MAXSTR];
        buffer[0] = b't' as c_char;
        buffer[1] = b'h' as c_char;
        buffer[2] = b'e' as c_char;
        buffer[3] = b' ' as c_char;

        unsafe {
            copy_str_to_c_buffer(buffer[4..].as_mut_ptr(), MAXSTR - 4, "emu");
            assert_eq!(CStr::from_ptr(buffer.as_ptr()).to_str().unwrap(), "the emu");
        }
        assert_eq!(buffer[7], 0);
    }
}
