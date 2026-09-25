//! Monster creation and behaviour.
//!
//! Ported from `src/c/monsters.c` to Rust.
use crate::config::GameConfig;
use crate::daemon::{fuse, lengthen};
use crate::daemons::unconfuse;
use crate::entity::chase::{dist, roomin, runto};
use crate::entity::fight::set_mname;
use crate::entity::player::{Thing, ThingMonster, ThingObject, MonsterFlags};
use crate::game::PLAYER;
use crate::item::rings::RingType;
use crate::item::thing_list::{attach_pack, new_actor};
use crate::item::things::new_thing;
use crate::level::find_floor;
use crate::misc::{rnd_thing, spread};
use crate::rnd::rnd;
use crate::startup::roll;
use crate::ui::output;
use crate::ui::output::{addmsg_str, msg_str};
use crate::ui::runtime;
use glam::IVec2;
use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_int, c_uchar};

use crate::globals::monsters;

const LAMPDIST: c_int = 3;
const HUHDURATION: c_int = 20;
const AFTER: c_int = 2;
const VS_MAGIC: c_int = 0o03;

pub use crate::globals::CMonster;

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
    static mut max_level: c_int;
    static mut wizard: c_int;

    fn strcmp(a: *const c_char, b: *const c_char) -> c_int;
    fn abort() -> !;
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
unsafe fn has_flag(tp: *mut Thing, flag: MonsterFlags) -> bool {
    (*thing_t(tp)).t_flags.contains(flag)
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    crate::game::PLAYER.has_flag(flag)
}

#[inline]
unsafe fn iswearing(which: RingType) -> bool {
    (!PLAYER.left_ring().is_null()
        && RingType::from_raw((*thing_o(PLAYER.left_ring())).o_which) == Some(which))
        || (!PLAYER.right_ring().is_null()
            && RingType::from_raw((*thing_o(PLAYER.right_ring())).o_which) == Some(which))
}

/// Picks an appropriate monster glyph for the current depth.
#[no_mangle]
pub unsafe fn randmonster(wander: bool) -> c_char {
    let mons = if wander { &WAND_MONS } else { &LVL_MONS };
    let level = crate::game::current_depth();
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
pub unsafe extern "C" fn new_monster(tp: *mut Thing, monster_type: c_char, cp: *mut IVec2) {
    let level = crate::game::current_depth();
    let mut lev_add = level - GameConfig::AMULET_LEVEL;
    if lev_add < 0 {
        lev_add = 0;
    }

    // `tp` was already allocated into `MLIST` by `new_actor`; no attach needed.

    (*thing_t(tp)).t_type = monster_type as u8;
    (*thing_t(tp)).t_disguise = monster_type as u8;
    (*thing_t(tp)).t_pos = *cp;

    (*thing_t(tp)).t_oldch = crate::draw::cell_glyph((*cp).y, (*cp).x) as u8;
    (*thing_t(tp)).t_room = roomin(cp);
    // Record the monster in the per-cell occupancy map.
    crate::game::set_monster((*cp).y, (*cp).x, tp);

    let mp = &monsters[(monster_type as i32 - 'A' as i32) as usize];
    (*thing_t(tp)).t_stats.level = mp.m_stats.level + lev_add;
    (*thing_t(tp)).t_stats.max_hit_points = roll((*thing_t(tp)).t_stats.level, 8);
    (*thing_t(tp)).t_stats.hit_points = (*thing_t(tp)).t_stats.max_hit_points;
    (*thing_t(tp)).t_stats.armor = mp.m_stats.armor - lev_add;
    (*thing_t(tp)).t_stats.damage = mp.m_stats.damage;
    (*thing_t(tp)).t_stats.strength = mp.m_stats.strength;
    (*thing_t(tp)).t_stats.experience = mp.m_stats.experience + lev_add * 10 + exp_add(tp);
    (*thing_t(tp)).t_flags = MonsterFlags::from_bits(mp.m_flags);
    if level > 29 {
        (*thing_t(tp)).t_flags.insert(MonsterFlags::HASTE);
    }
    (*thing_t(tp)).t_turn = true;
    crate::entity::player::set_thing_pack(tp, std::ptr::null_mut());

    if iswearing(RingType::Aggravate) {
        runto(cp);
    }
    if monster_type == 'X' as c_char {
        (*thing_t(tp)).t_disguise = rnd_thing() as u8;
    }
}

/// Computes bonus experience from a monster's level and max HP.
#[no_mangle]
pub unsafe extern "C" fn exp_add(tp: *mut Thing) -> c_int {
    let mut modu = if (*thing_t(tp)).t_stats.level == 1 {
        (*thing_t(tp)).t_stats.max_hit_points / 8
    } else {
        (*thing_t(tp)).t_stats.max_hit_points / 6
    };

    if (*thing_t(tp)).t_stats.level > 9 {
        modu *= 20;
    } else if (*thing_t(tp)).t_stats.level > 6 {
        modu *= 4;
    }
    modu
}

/// Spawns a wandering monster in a different room and sets it running toward the hero.
#[no_mangle]
pub unsafe extern "C" fn wanderer() {
    let tp = new_actor();
    let mut cp;

    loop {
        cp = find_floor(None, 0, true).unwrap_or(IVec2::ZERO);
        if roomin(&mut cp) != crate::game::PLAYER.room() {
            break;
        }
    }

    new_monster(tp, randmonster(true), &mut cp);

    if player_has(MonsterFlags::SEEMONST) {
        output::set_standout(true);
        if !player_has(MonsterFlags::HALU) {
            output::write_glyph(((*thing_t(tp)).t_type as u8) as char);
        } else {
            output::write_glyph((rnd(26) as u8 + b'A') as char);
        }
        output::set_standout(false);
    }

    runto(&mut (*thing_t(tp)).t_pos);

    if wizard != 0 {
        msg_str(&format!(
            "started a wandering {}",
            monsters[((*thing_t(tp)).t_type as i32 - 'A' as i32) as usize].m_name
        ));
    }
}

/// Wakes and updates an adjacent monster's pursuit behavior and special gaze logic.
#[no_mangle]
pub unsafe extern "C" fn wake_monster(y: c_int, x: c_int) -> *mut Thing {
    let tp = crate::game::monster_at(y, x);
    if tp.is_null() {
        runtime::shutdown();
        abort();
    }

    let ch = (*thing_t(tp)).t_type;

    if !has_flag(tp, MonsterFlags::RUN)
        && rnd(3) != 0
        && has_flag(tp, MonsterFlags::MEAN)
        && !has_flag(tp, MonsterFlags::HELD)
        && !iswearing(RingType::Stealth)
        && !player_has(MonsterFlags::LEVIT)
    {
        crate::entity::player::set_thing_dest_hero(tp);
        (*thing_t(tp)).t_flags.insert(MonsterFlags::RUN);
    }

    if ch == b'M'
        && !player_has(MonsterFlags::BLIND)
        && !player_has(MonsterFlags::HALU)
        && !has_flag(tp, MonsterFlags::FOUND)
        && !has_flag(tp, MonsterFlags::CANCELLED)
        && has_flag(tp, MonsterFlags::RUN)
    {
        let rp = crate::game::PLAYER.room();
        let hero = crate::game::PLAYER.pos();
        if (rp.is_some() && !crate::game::room_dark(rp)) || dist(y, x, hero.y, hero.x) < LAMPDIST {
            (*thing_t(tp)).t_flags.insert(MonsterFlags::FOUND);
            if save(VS_MAGIC) == 0 {
                if player_has(MonsterFlags::HUH) {
                    lengthen(unconfuse as *const c_void, spread(HUHDURATION));
                } else {
                    fuse(unconfuse as *const c_void, 0, spread(HUHDURATION), AFTER);
                }
                crate::game::PLAYER.add_flag(MonsterFlags::HUH);
                let mname = set_mname(tp);
                addmsg_str(&CStr::from_ptr(mname).to_string_lossy());
                if strcmp(mname, c"it".as_ptr()) != 0 {
                    addmsg_str("'");
                }
                msg_str("s gaze has confused you");
            }
        }
    }

    if has_flag(tp, MonsterFlags::GREED) && !has_flag(tp, MonsterFlags::RUN) {
        (*thing_t(tp)).t_flags.insert(MonsterFlags::RUN);
        let pr = crate::game::PLAYER.room();
        if pr.is_some() && crate::game::room_goldval(pr) != 0 {
            crate::entity::player::set_thing_dest(tp, crate::game::room_gold_ptr(pr));
        } else {
            crate::entity::player::set_thing_dest_hero(tp);
        }
    }

    tp
}

/// Potentially gives a monster a carried item based on depth and monster carry chance.
#[no_mangle]
pub unsafe extern "C" fn give_pack(tp: *mut Thing) {
    if crate::game::current_depth() >= max_level
        && rnd(100) < monsters[((*thing_t(tp)).t_type as i32 - 'A' as i32) as usize].m_carry
    {
        attach_pack(tp, new_thing());
    }
}

/// Rolls a saving throw for any creature against an effect category.
#[no_mangle]
pub unsafe extern "C" fn save_throw(which: c_int, tp: *mut Thing) -> c_int {
    save_throw_for_level(which, (*thing_t(tp)).t_stats.level)
}

/// Roll a saving throw using an explicit caster level (used for the hero, whose
/// `Thing` is no longer reachable as a raw pointer).
#[inline]
fn save_throw_for_level(which: c_int, level: c_int) -> c_int {
    let need = 14 + which - level / 2;
    if unsafe { roll(1, 20) } >= need {
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
        if !PLAYER.left_ring().is_null()
            && RingType::from_raw((*thing_o(PLAYER.left_ring())).o_which)
                == Some(RingType::Protection)
        {
            adj -= (*thing_o(PLAYER.left_ring())).o_arm;
        }
        if !PLAYER.right_ring().is_null()
            && RingType::from_raw((*thing_o(PLAYER.right_ring())).o_which)
                == Some(RingType::Protection)
        {
            adj -= (*thing_o(PLAYER.right_ring())).o_arm;
        }
    }
    save_throw_for_level(adj, PLAYER.level())
}
