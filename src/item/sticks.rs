//! Wands and staves (the legacy `sticks`): zapping and their bolt effects.
//!
//! Ported from `src/c/sticks.c` to Rust.
use crate::command::Command;
use crate::direction::Direction;
use crate::entity::monsters::{save, save_throw_id};
use crate::entity::player::ObjectFlags;
use crate::dungeon::Dungeon;
use crate::game::globals::{MonsterInfo, ObjInfo};
use crate::item::arena::{ThingArena, ThingId};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::get_item_id;
use crate::item::weapons::{do_motion, hit_monster};
use crate::rip::death;
use crate::rnd::Random;
use crate::ui::output;
use crate::ui::output::msg_str;
use glam::IVec2;

const STICK: i32 = '/' as i32;
const WEAPON: i32 = ')' as i32;
const FLAME: i32 = 9;
const VS_MAGIC: i32 = 3;

#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StickType {
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
    /// Number of wand/staff kinds.
    pub const COUNT: usize = 14;

    pub fn from_raw(value: i32) -> Option<Self> {
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

    pub fn index(self) -> usize {
        self as usize
    }
}

const MAXSTICKS: usize = StickType::COUNT;

#[inline]
fn hero_pos(dungeon: &Dungeon) -> IVec2 {
    dungeon.player.pos()
}

#[inline]
unsafe fn ce_coord(a: IVec2, b: IVec2) -> u8 {
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
fn stick_type(id: ThingId, dungeon: &Dungeon) -> Option<StickType> {
    dungeon.objects
        .with_object(id, |o| StickType::from_raw(o.o_which))
        .flatten()
}

/// Set up a new stick with the expected damage and charge values.
pub unsafe fn fix_stick_id(id: ThingId, random: &mut Random, dungeon: &Dungeon) {
    fix_stick_in(&dungeon.objects, id, random);
}

/// Set up a new stick in the supplied arena.
pub(crate) unsafe fn fix_stick_in(objects: &ThingArena, id: ThingId, random: &mut Random) {
    if !matches!(
        objects.with_object(id, |o| o.o_type),
        Some(ItemType::Stick(_))
    ) {
        return;
    }

    let light = objects
        .with_object(id, |o| StickType::from_raw(o.o_which))
        .flatten()
        == Some(StickType::Light);
    objects.with_object_mut(id, |o| {
        if light {
            set_c_string(&mut o.o_damage, "2x3");
        } else {
            set_c_string(&mut o.o_damage, "1x1");
        }
        set_c_string(&mut o.o_hurldmg, "1x1");
        o.o_arm = if light {
            random.rnd(10) + 10
        } else {
            random.rnd(5) + 3
        };
    });
}

/// do_zap:
/// Perform a zap with a wand or staff and apply a simplified effect.
///
/// # Safety
/// State/table pointers must remain valid throughout reentrant combat and UI
/// calls, without conflicting live references. Metadata slices must contain
/// every kind, including the ten weapon entries used by transient bolts.
/// `quiet` is shared atomically and may be updated by reentrant helpers.
pub unsafe fn do_zap(
    random: &mut Random,
    dungeon: &Dungeon,
    ws_info: *mut [ObjInfo],
    after: *mut u8,
    delta: IVec2,
    terse: u8,
    again: u8,
    l_last_comm: Command,
    l_last_dir: Direction,
    l_last_pick: Option<ThingId>,
    last_comm: &crate::command_dispatch::CommandValue<Command>,
    last_dir: &crate::command_dispatch::CommandValue<Direction>,
    last_pick: &crate::command_dispatch::CommandValue<Option<ThingId>>,
    mpos: &std::sync::atomic::AtomicI32,
    msg_esc: &std::sync::atomic::AtomicU8,
    n_objs: *mut i32,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    count: &std::sync::atomic::AtomicI32,
    quiet: &std::sync::atomic::AtomicI32,
    has_hit: *mut u8,
    to_death: *mut u8,
    kamikaze: *mut u8,
    vf_hit: *mut i32,
    fight_flush: *const u8,
    monsters: *mut [MonsterInfo; 26],
    e_levels: *const [i32; 21],
) {
    // The live stick table stays raw while the selection prompt can reenter.
    let ws_names = (&*ws_info).to_vec();
    let Some(obj) = get_item_id("zap with", ItemFilter::Category(ItemType::STICK), dungeon, after, again, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick, mpos, msg_esc, n_objs, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, &ws_names) else {
        return;
    };
    if !matches!(
        dungeon.objects.with_object(obj, |o| o.o_type),
        Some(ItemType::Stick(_))
    ) {
        *after = false as u8;
        msg_str("you can't zap with that!");
        return;
    }
    if dungeon.objects.with_object(obj, |o| o.o_arm).unwrap_or(0) == 0 {
        msg_str("nothing happens");
        return;
    }

    let kind = stick_type(obj, dungeon);

    match kind {
        Some(StickType::Light) => {
            (&mut *ws_info)[StickType::Light.index()].oi_know = true;
            msg_str("the corridor glows and then fades");
        }
        Some(StickType::Drain) => {
            if dungeon.player.stats().hit_points < 2 {
                msg_str("you are too weak to use it");
                return;
            }
            drain(dungeon);
        }
        Some(StickType::Invis)
        | Some(StickType::Polymorph)
        | Some(StickType::TelAway)
        | Some(StickType::TelTo)
        | Some(StickType::Cancel) => {
            let hero = hero_pos(dungeon);
            let mut y = hero.y;
            let mut x = hero.x;
            while dungeon.monster_map.at(y as usize, x as usize).is_none()
                && dungeon.with_level(|level| level.tile_at(y as usize, x as usize).is_walkable())
            {
                y += delta.y;
                x += delta.x;
            }
            if dungeon.monster_map.at(y as usize, x as usize).is_some() {
                msg_str("the spell takes effect");
            }
        }
        Some(StickType::Missile) => {
            (&mut *ws_info)[StickType::Missile.index()].oi_know = true;
            let bolt = crate::item::arena::new_item_id(&dungeon.objects);
            let launch = dungeon.player.weapon_which();
            dungeon.objects.with_object_mut(bolt, |o| {
                o.o_type = ItemType::Weapon(FLAME);
                o.o_which = FLAME;
                set_c_string(&mut o.o_hurldmg, "1x4");
                o.o_hplus = 100;
                o.o_dplus = 1;
                o.o_flags = ObjectFlags::MISL;
                if let Some(which) = launch {
                    o.o_launch = which;
                }
            });
            do_motion(bolt, delta.y, delta.x, dungeon, terse);
            let bolt_pos =
                crate::item::arena::with_object(&dungeon.objects, bolt, |o| o.o_pos).unwrap_or(IVec2::ZERO);
            if let Some(mid) = dungeon.monster_map.at(bolt_pos.y as usize, bolt_pos.x as usize) {
                if save_throw_id(VS_MAGIC, mid, random, dungeon) == 0 {
                    let combat_weap_info: [ObjInfo; 10] =
                        std::array::from_fn(|i| weap_info[i].clone());
                    hit_monster(bolt_pos.y, bolt_pos.x, bolt, random, dungeon,
                        count, quiet, has_hit, to_death, kamikaze, vf_hit,
                        fight_flush, monsters, &combat_weap_info, e_levels);
                } else if terse != 0 {
                    msg_str("missle vanishes");
                } else {
                    msg_str("the missle vanishes with a puff of smoke");
                }
            } else if terse != 0 {
                msg_str("missle vanishes");
            } else {
                msg_str("the missle vanishes with a puff of smoke");
            }
            let _ = dungeon.objects.remove(bolt);
        }
        Some(StickType::HasteM) | Some(StickType::SlowM) => {
            let hero = hero_pos(dungeon);
            let mut y = hero.y;
            let mut x = hero.x;
            while dungeon.monster_map.at(y as usize, x as usize).is_none()
                && dungeon.with_level(|level| level.tile_at(y as usize, x as usize).is_walkable())
            {
                y += delta.y;
                x += delta.x;
            }
            if dungeon.monster_map.at(y as usize, x as usize).is_some() {
                msg_str("the spell takes effect");
            }
        }
        Some(StickType::Elect) | Some(StickType::Fire) | Some(StickType::Cold) => {
            let name = match kind {
                Some(StickType::Elect) => "bolt",
                Some(StickType::Fire) => "flame",
                _ => "ice",
            };
            let hero = hero_pos(dungeon);
            let combat_weap_info: [ObjInfo; 10] =
                std::array::from_fn(|i| weap_info[i].clone());
            fire_bolt(hero, delta, name, random, dungeon, count, quiet, has_hit,
                to_death, kamikaze, vf_hit, fight_flush, monsters,
                &combat_weap_info, e_levels);
            if let Some(kind) = kind {
                (&mut *ws_info)[kind.index()].oi_know = true;
            }
        }
        Some(StickType::Nop) => {}
        None => {
            msg_str("what a bizarre schtick!");
        }
    }

    dungeon.objects.with_object_mut(obj, |o| o.o_arm -= 1);
}

/// drain:
/// Reduce the hero's hit points and apply a simple draining effect.
pub unsafe fn drain(dungeon: &Dungeon) {
    dungeon.player.with_stats_mut(|stats| {
        if stats.hit_points >= 2 {
            stats.hit_points /= 2;
        }
    });
    msg_str("you have a tingling feeling");
}

/// fire_bolt:
/// Fire a bolt in a given direction from a specific starting place.
///
/// # Safety
/// All state/table pointers must remain valid throughout reentrant combat,
/// without live references conflicting with writes to mutable state.
/// `quiet` is shared atomically and may be updated by reentrant helpers.
pub unsafe fn fire_bolt(
    start: IVec2,
    dir: IVec2,
    _name: &str,
    random: &mut Random,
    dungeon: &Dungeon,
    count: &std::sync::atomic::AtomicI32,
    quiet: &std::sync::atomic::AtomicI32,
    has_hit: *mut u8,
    to_death: *mut u8,
    kamikaze: *mut u8,
    vf_hit: *mut i32,
    fight_flush: *const u8,
    monsters: *mut [MonsterInfo; 26],
    weap_info: *const [ObjInfo; 10],
    e_levels: *const [i32; 21],
) {
    let mut pos = start;
    let hero = hero_pos(dungeon);
    let hit_hero = start != hero;
    let bolt = crate::item::arena::new_item_id(&dungeon.objects);

    dungeon.objects.with_object_mut(bolt, |o| {
        o.o_type = ItemType::Weapon(FLAME);
        o.o_which = FLAME;
        set_c_string(&mut o.o_hurldmg, "6x6");
        o.o_hplus = 100;
        o.o_dplus = 0;
    });

    pos.y += dir.y;
    pos.x += dir.x;
    if hit_hero && ce_coord(pos, hero) != 0 {
        if save(VS_MAGIC, random, dungeon) == 0 {
            if (dungeon.player.stats().hit_points - random.roll(6, 6)) <= 0 {
                death('b' as u8);
            }
            msg_str("the bolt hits");
        } else {
            msg_str("the bolt whizzes by you");
        }
    } else {
        if let Some(mid) = dungeon.monster_map.at(pos.y as usize, pos.x as usize) {
            if save_throw_id(VS_MAGIC, mid, random, dungeon) == 0 {
                hit_monster(pos.y, pos.x, bolt, random, dungeon, count, quiet,
                    has_hit, to_death, kamikaze, vf_hit, fight_flush, monsters,
                    weap_info, e_levels);
            } else {
                msg_str("the bolt misses");
            }
        } else {
            msg_str("the bolt misses");
        }
    }

    let _ = dungeon.objects.remove(bolt);
    crate::draw::write_cell_glyph(pos, '/');
    output::refresh();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bolt_combat_exposes_all_entity_dependencies() {
        let _: unsafe fn(
            IVec2, IVec2, &str, &mut Random, &Dungeon,
            &std::sync::atomic::AtomicI32, &std::sync::atomic::AtomicI32,
            *mut u8, *mut u8, *mut u8, *mut i32, *const u8,
            *mut [MonsterInfo; 26], *const [ObjInfo; 10], *const [i32; 21],
        ) = fire_bolt;
    }

    #[test]
    fn drain_preserves_weak_heroes_and_halves_other_hit_points() {
        let dungeon = Dungeon::empty();
        let other = Dungeon::empty();
        other.player.with_stats_mut(|stats| stats.hit_points = 19);

        for (before, expected) in [(0, 0), (1, 1), (2, 1), (9, 4), (10, 5)] {
            dungeon.player.with_stats_mut(|stats| stats.hit_points = before);
            unsafe { drain(&dungeon); }
            assert_eq!(dungeon.player.stats().hit_points, expected);
            assert_eq!(other.player.stats().hit_points, 19);
        }
    }

    #[test]
    fn stick_lookup_uses_the_supplied_arena() {
        let dungeon = Dungeon::empty();
        let other = Dungeon::empty();
        let id = dungeon.objects.new_item_id();
        let other_id = other.objects.new_item_id();
        assert_eq!(id, other_id);
        dungeon.objects.with_object_mut(id, |o| o.o_which = StickType::Light as i32);
        other.objects.with_object_mut(other_id, |o| o.o_which = StickType::Drain as i32);

        assert_eq!(stick_type(id, &dungeon), Some(StickType::Light));
        assert_eq!(stick_type(other_id, &other), Some(StickType::Drain));
        dungeon.objects.remove(id);
        assert_eq!(stick_type(id, &dungeon), None);
    }
}
