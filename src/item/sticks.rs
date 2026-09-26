//! Wands and staves (the legacy `sticks`): zapping and their bolt effects.
//!
//! Ported from `src/c/sticks.c` to Rust.
use crate::entity::monsters::{save, save_throw};
use crate::entity::player::{ObjectFlags, Thing, ThingMonster, ThingObject};
use crate::game::PLAYER;
use crate::globals::ws_info;
use crate::item::pack::get_item;
use crate::item::weapons::{do_motion, hit_monster};
use crate::rip::death;
use crate::rnd::rnd;
use crate::startup::roll;
use crate::ui::output;
use crate::ui::output::msg_str;
use glam::IVec2;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

const STICK: c_int = '/' as c_int;
const WEAPON: c_int = ')' as c_int;
const FLAME: c_int = 9;
const VS_MAGIC: c_int = 3;

#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum StickType {
    Light = 0,
    Invis = 1,
    Elect = 2,
    Fire = 3,
    Cold = 4,
    Polymorph = 5,
    Missile = 6,
    HasteM = 7,
    SlowM = 8,
    Drain = 9,
    Nop = 10,
    TelAway = 11,
    TelTo = 12,
    Cancel = 13,
}

impl StickType {
    const COUNT: usize = 14;

    fn from_raw(value: c_int) -> Option<Self> {
        match value {
            0 => Some(Self::Light),
            1 => Some(Self::Invis),
            2 => Some(Self::Elect),
            3 => Some(Self::Fire),
            4 => Some(Self::Cold),
            5 => Some(Self::Polymorph),
            6 => Some(Self::Missile),
            7 => Some(Self::HasteM),
            8 => Some(Self::SlowM),
            9 => Some(Self::Drain),
            10 => Some(Self::Nop),
            11 => Some(Self::TelAway),
            12 => Some(Self::TelTo),
            13 => Some(Self::Cancel),
            _ => None,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

const MAXSTICKS: usize = StickType::COUNT;

unsafe extern "C" {
    static mut terse: c_uchar;
    static mut after: c_uchar;
    static mut delta: IVec2;

}

#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
fn hero_pos() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
unsafe fn moat_at(y: c_int, x: c_int) -> *mut Thing {
    crate::game::monster_at(y, x) as *mut Thing
}

#[inline]
unsafe fn ce_coord(a: IVec2, b: IVec2) -> c_uchar {
    if a.x == b.x && a.y == b.y {
        1
    } else {
        0
    }
}

#[inline]
unsafe fn set_c_string(dst: &mut [u8], src: &str) {
    let bytes = src.as_bytes();
    let limit = bytes.len().min(dst.len().saturating_sub(1));
    for (idx, byte) in bytes[..limit].iter().enumerate() {
        dst[idx] = *byte;
    }
    if dst.len() > limit {
        dst[limit] = 0;
    }
}

#[inline]
unsafe fn stick_type(obj: *mut Thing) -> Option<StickType> {
    StickType::from_raw((*thing_o(obj)).o_which)
}

/// fix_stick:
/// Set up a new stick with the expected damage and charge values.
#[no_mangle]
pub unsafe extern "C" fn fix_stick(cur: *mut Thing) {
    if (*thing_o(cur)).o_type != STICK {
        return;
    }

    if stick_type(cur) == Some(StickType::Light) {
        set_c_string(&mut (*thing_o(cur)).o_damage, "2x3");
    } else {
        set_c_string(&mut (*thing_o(cur)).o_damage, "1x1");
    }
    set_c_string(&mut (*thing_o(cur)).o_hurldmg, "1x1");

    (*thing_o(cur)).o_arm = if stick_type(cur) == Some(StickType::Light) {
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
        after = false as c_uchar;
        msg_str("you can't zap with that!");
        return;
    }
    if (*thing_o(obj)).o_arm == 0 {
        msg_str("nothing happens");
        return;
    }

    let kind = stick_type(obj);

    match kind {
        Some(StickType::Light) => {
            ws_info[StickType::Light.index()].oi_know = true;
            msg_str("the corridor glows and then fades");
        }
        Some(StickType::Drain) => {
            if crate::game::PLAYER.stats().hit_points < 2 {
                msg_str("you are too weak to use it");
                return;
            }
            drain();
        }
        Some(StickType::Invis)
        | Some(StickType::Polymorph)
        | Some(StickType::TelAway)
        | Some(StickType::TelTo)
        | Some(StickType::Cancel) => {
            let hero = hero_pos();
            let mut y = hero.y;
            let mut x = hero.x;
            while crate::game::cell_is_walkable(y, x) {
                y += delta.y;
                x += delta.x;
            }
            if !moat_at(y, x).is_null() {
                msg_str("the spell takes effect");
            }
        }
        Some(StickType::Missile) => {
            ws_info[StickType::Missile.index()].oi_know = true;
            let mut bolt = Thing::object(ThingObject::default());
            (*thing_o(&mut bolt)).o_type = WEAPON;
            (*thing_o(&mut bolt)).o_which = FLAME;
            set_c_string(&mut (*thing_o(&mut bolt)).o_hurldmg, "1x4");
            (*thing_o(&mut bolt)).o_hplus = 100;
            (*thing_o(&mut bolt)).o_dplus = 1;
            (*thing_o(&mut bolt)).o_flags = ObjectFlags::MISL;
            if !PLAYER.weapon().is_null() {
                (*thing_o(&mut bolt)).o_launch = (*thing_o(PLAYER.weapon())).o_which;
            }
            do_motion(&mut bolt, delta.y, delta.x);
            let bolt_pos = (*thing_o(&mut bolt)).o_pos;
            if !moat_at(bolt_pos.y, bolt_pos.x).is_null()
                && save_throw(VS_MAGIC, moat_at(bolt_pos.y, bolt_pos.x)) == 0
            {
                hit_monster(bolt_pos.y, bolt_pos.x, &mut bolt);
            } else if terse != 0 {
                msg_str("missle vanishes");
            } else {
                msg_str("the missle vanishes with a puff of smoke");
            }
        }
        Some(StickType::HasteM) | Some(StickType::SlowM) => {
            let hero = hero_pos();
            let mut y = hero.y;
            let mut x = hero.x;
            while crate::game::cell_is_walkable(y, x) {
                y += delta.y;
                x += delta.x;
            }
            if !moat_at(y, x).is_null() {
                msg_str("the spell takes effect");
            }
        }
        Some(StickType::Elect) | Some(StickType::Fire) | Some(StickType::Cold) => {
            let name = match kind {
                Some(StickType::Elect) => c"bolt",
                Some(StickType::Fire) => c"flame",
                _ => c"ice",
            };
            let mut hero = hero_pos();
            fire_bolt(&mut hero, &raw mut delta, name.as_ptr() as *mut c_char);
            if let Some(kind) = kind {
                ws_info[kind.index()].oi_know = true;
            }
        }
        Some(StickType::Nop) => {}
        None => {
            msg_str("what a bizarre schtick!");
        }
    }

    (*thing_o(obj)).o_arm -= 1;
}

/// drain:
/// Reduce the hero's hit points and apply a simple draining effect.
#[no_mangle]
pub unsafe extern "C" fn drain() {
    crate::game::PLAYER.with_stats_mut(|stats| {
        if stats.hit_points >= 2 {
            stats.hit_points /= 2;
        }
    });
    msg_str("you have a tingling feeling");
}

/// fire_bolt:
/// Fire a bolt in a given direction from a specific starting place.
#[no_mangle]
pub unsafe extern "C" fn fire_bolt(start: *mut IVec2, dir: *mut IVec2, name: *mut c_char) {
    let mut pos = *start;
    let mut hero = hero_pos();
    let hit_hero = start != &mut hero;
    let mut bolt = Thing::object(ThingObject::default());

    (*thing_o(&mut bolt)).o_type = WEAPON;
    (*thing_o(&mut bolt)).o_which = FLAME;
    set_c_string(&mut (*thing_o(&mut bolt)).o_hurldmg, "6x6");
    (*thing_o(&mut bolt)).o_hplus = 100;
    (*thing_o(&mut bolt)).o_dplus = 0;

    pos.y += (*dir).y;
    pos.x += (*dir).x;
    if hit_hero && ce_coord(pos, hero) != 0 {
        if save(VS_MAGIC) == 0 {
            if (crate::game::PLAYER.stats().hit_points - roll(6, 6)) <= 0 {
                death('b' as c_char);
            }
            msg_str("the bolt hits");
        } else {
            msg_str("the bolt whizzes by you");
        }
    } else {
        if !moat_at(pos.y, pos.x).is_null() && save_throw(VS_MAGIC, moat_at(pos.y, pos.x)) == 0 {
            hit_monster(pos.y, pos.x, &mut bolt);
        } else {
            msg_str("the bolt misses");
        }
    }

    output::write_glyph_at(IVec2::new(pos.x, pos.y), '/');
    output::refresh();
}

/// charge_str:
/// Return an appropriate string for a wand charge display.
unsafe fn charge_str(obj: *mut Thing) -> *mut c_char {
    static mut BUF: [u8; 20] = [0; 20];
    if !(*thing_o(obj)).o_flags.contains(ObjectFlags::KNOW) {
        BUF[0] = 0;
    } else if terse != 0 {
        let text = format!(" [{}]", (*thing_o(obj)).o_arm);
        set_c_string(&mut BUF, &text);
    } else {
        let text = format!(" [{} charges]", (*thing_o(obj)).o_arm);
        set_c_string(&mut BUF, &text);
    }
    BUF.as_mut_ptr() as *mut c_char
}
