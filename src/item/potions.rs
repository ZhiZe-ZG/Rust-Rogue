//! Potions and quaffing.
//!
//! Ported from `src/c/potions.c` to Rust.
use crate::command::Command;
use crate::direction::Direction;
use crate::rnd::Random;

use crate::daemon::{fuse, lengthen, start_daemon, Daemon};
use crate::daemons::{come_down, sight};
use crate::draw::look;
use crate::dungeon::Dungeon;

use crate::entity::chase::see_monst;
use crate::entity::player::{check_level, chg_str, MonsterFlags, ObjectFlags, Stats, Thing};
use crate::game::globals::ObjInfo;
use crate::game::MonsterId;
use crate::item::arena::ThingId;
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::{get_item_id, leave_pack_id};
use crate::item::rings::RingType;
use crate::misc::{add_haste, add_str, call_it, choose_str, spread};
use crate::ui::output::{msg_str, show_win};
use crate::ui::terminal::UiState;
use glam::IVec2;

/// Potion and status-effect handling.
///
/// These helpers implement the potion logic in Rust; the legacy C entry points
/// are gone, so nothing here is exported across an FFI boundary.
const POTION: i32 = '!' as i32;
const SCROLL: i32 = '?' as i32;
const WEAPON: i32 = ')' as i32;
const ARMOR: i32 = ']' as i32;
const RING: i32 = '=' as i32;
const STICK: i32 = '/' as i32;
const AMULET: i32 = ',' as i32;
const FOOD: i32 = ':' as i32;
const MAGIC: i32 = '$' as i32;
const STAIRS: i32 = '%' as i32;
const FLOOR: i32 = '.' as i32;
const PASSAGE: i32 = '#' as i32;
const SPACE: i32 = ' ' as i32;
const H_WALL: i32 = '-' as i32;
const V_WALL: i32 = '|' as i32;
const TRAP: i32 = '^' as i32;

const MAXPOTIONS: usize = 14;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PotionType {
    Confuse = 0,
    Lsd = 1,
    Poison = 2,
    Strength = 3,
    SeeInvisible = 4,
    Healing = 5,
    MonsterFind = 6,
    TrapFind = 7,
    Raise = 8,
    ExtraHealing = 9,
    Haste = 10,
    Restore = 11,
    Blind = 12,
    Levitate = 13,
}

impl PotionType {
    /// Number of potion kinds.
    pub const COUNT: usize = 14;

    #[inline]
    pub fn from_raw(value: i32) -> Self {
        Self::try_from_raw(value).unwrap_or_else(|| panic!("invalid potion type: {value}"))
    }

    /// Build a potion kind from its legacy index (`None` when out of range).
    #[inline]
    pub fn try_from_raw(value: i32) -> Option<Self> {
        Some(match value {
            0 => Self::Confuse,
            1 => Self::Lsd,
            2 => Self::Poison,
            3 => Self::Strength,
            4 => Self::SeeInvisible,
            5 => Self::Healing,
            6 => Self::MonsterFind,
            7 => Self::TrapFind,
            8 => Self::Raise,
            9 => Self::ExtraHealing,
            10 => Self::Haste,
            11 => Self::Restore,
            12 => Self::Blind,
            13 => Self::Levitate,
            _ => return None,
        })
    }

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

const HUHDURATION: i32 = 20;
const SEEDURATION: i32 = 850;
const HEALTIME: i32 = 30;
const BEFORE: i32 = 1;
const AFTER: i32 = 2;

#[inline]
fn hero(dungeon: &Dungeon) -> IVec2 {
    dungeon.player.pos()
}

#[inline]
fn player_has(flag: MonsterFlags, dungeon: &Dungeon) -> bool {
    dungeon.player.has_flag(flag)
}

#[inline]
fn thing_has(id: MonsterId, flag: MonsterFlags, dungeon: &Dungeon) -> bool {
    dungeon.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => data.t_flags.contains(flag),
            Thing::Object { .. } => false,
        })
        .unwrap_or(false)
}

#[inline]
fn moat(y: i32, x: i32, dungeon: &Dungeon) -> Option<MonsterId> {
    dungeon.monster_map.at(y as usize, x as usize)
}

/// Shared implementation for potion effects that need the normal fuse/flag
/// setup and knowledge tracking used by the C version.
unsafe fn do_pot_impl(
    potion: PotionType,
    knowit: bool,
    random: &mut Random,
    dungeon: &Dungeon,
    pot_info: *mut [ObjInfo],
    fruit: &str,
) {
    let (flags, daemon, base_time, high_msg, straight_msg): (
        MonsterFlags,
        Option<Daemon>,
        i32,
        String,
        String,
    ) = {
        let taste = format!("this potion tastes like {} juice", fruit);
        match potion {
            PotionType::Confuse => (
                MonsterFlags::HUH,
                Some(Daemon::Unconfuse),
                HUHDURATION,
                "what a tripy feeling!".to_owned(),
                "wait, what's going on here. Huh? What? Who?".to_owned(),
            ),
            PotionType::Lsd => (
                MonsterFlags::HALU,
                Some(Daemon::ComeDown),
                SEEDURATION,
                "Oh, wow!  Everything seems so cosmic!".to_owned(),
                "Oh, wow!  Everything seems so cosmic!".to_owned(),
            ),
            PotionType::SeeInvisible => (
                MonsterFlags::CANSEE,
                Some(Daemon::Unsee),
                SEEDURATION,
                taste.clone(),
                taste,
            ),
            PotionType::Blind => (
                MonsterFlags::BLIND,
                Some(Daemon::Sight),
                SEEDURATION,
                "oh, bummer!  Everything is dark!  Help!".to_owned(),
                "a cloak of darkness falls around you".to_owned(),
            ),
            PotionType::Levitate => (
                MonsterFlags::LEVIT,
                Some(Daemon::Land),
                HEALTIME,
                "oh, wow!  You're floating in the air!".to_owned(),
                "you start to float in the air".to_owned(),
            ),
            _ => (MonsterFlags::NONE, None, 0, String::new(), String::new()),
        }
    };

    (&mut *pot_info)[potion.index()].oi_know = knowit;

    let Some(daemon) = daemon else {
        return;
    };
    if flags.is_empty() {
        return;
    }

    let t = spread(base_time, random);
    if !player_has(flags, dungeon) {
        dungeon.player.add_flag(flags);
        fuse(daemon, 0, t, AFTER);
        look(false as u8, random);
    } else {
        lengthen(daemon, t);
    }
    let chosen = if player_has(MonsterFlags::HALU, dungeon) {
        high_msg
    } else {
        straight_msg
    };
    msg_str(&chosen);
}

/// quaff:
/// Quaff a potion from the pack.
///
/// # Safety
/// `pot_info` and `after` must remain valid throughout the call,
/// without conflicting live references. The metadata slice must contain every
/// potion kind and `e_levels` all 21 experience entries. Raw pointers keep
/// borrows short across helpers that can reenter game state (including `call_it`).
/// `max_stats` is shared through a lock; no guard is held across gameplay helpers.
pub(crate) unsafe fn quaff(
    random: &mut Random,
    dungeon: &Dungeon,
    ui: &UiState,
    pot_info: *mut [ObjInfo],
    after: *mut u8,
    terse: u8,
    max_strength: u32,
    e_levels: &[i32],
    fruit: &str,
    again: u8,
    inpack: &std::sync::atomic::AtomicI32,
    l_last_comm: Command,
    l_last_dir: Direction,
    l_last_pick: Option<ThingId>,
    last_comm: *mut Command,
    last_dir: *mut Direction,
    last_pick: *mut Option<ThingId>,
    mpos: &std::sync::atomic::AtomicI32,
    msg_esc: &std::sync::atomic::AtomicU8,
    n_objs: *mut i32,
    pack_used: &std::sync::Mutex<[u8; 26]>,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
    max_stats: &std::sync::RwLock<Stats>,
) {
    // Input/output can reenter metadata updates. Borrow an owned snapshot for
    // naming rather than the mutable potion table across those callbacks.
    let pot_names = (&*pot_info).to_vec();
    let Some(obj) = get_item_id("quaff", ItemFilter::Category(ItemType::POTION), dungeon, after, again, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick, mpos, msg_esc, n_objs, terse, inv_describe, arm_info, &pot_names, ring_info, scr_info, weap_info, ws_info) else {
        return;
    };
    let mut show = false;
    let trip = player_has(MonsterFlags::HALU, dungeon);

    if !matches!(
        dungeon.objects.with_object(obj, |o| o.o_type),
        Some(ItemType::Potion(_))
    ) {
        if terse == 0 {
            msg_str("yuk! Why would you want to drink that?");
        } else {
            msg_str("that's undrinkable");
        }
        return;
    }
    if dungeon.player.equipment().weapon_id() == Some(obj) {
        dungeon.player.set_weapon_id(None);
    }

    let (o_count, o_which) = dungeon.objects
        .with_object(obj, |o| (o.o_count, o.o_which))
        .unwrap_or((0, 0));
    let discardit = o_count == 1;
    leave_pack_id(obj, false, false, dungeon, inpack, last_pick, pack_used);

    let potion = PotionType::from_raw(o_which);
    match potion {
        PotionType::Confuse => do_pot_impl(PotionType::Confuse, !trip, random, dungeon, pot_info, fruit),
        PotionType::Poison => {
            (&mut *pot_info)[PotionType::Poison.index()].oi_know = true;
            if dungeon.player.wearing_ring(RingType::SustainStrength) {
                msg_str("you feel momentarily sick");
            } else {
                chg_str(-(random.rnd(3) + 1), dungeon, max_stats);
                msg_str("you feel very sick now");
                come_down();
            }
        }
        PotionType::Healing => {
            (&mut *pot_info)[PotionType::Healing.index()].oi_know = true;
            dungeon.player.with_stats_mut(|stats| {
                stats.hit_points += random.roll(stats.level, 4);
                if stats.hit_points > stats.max_hit_points {
                    stats.max_hit_points += 1;
                    stats.hit_points = stats.max_hit_points;
                }
            });
            sight(random);
            msg_str("you begin to feel better");
        }
        PotionType::Strength => {
            (&mut *pot_info)[PotionType::Strength.index()].oi_know = true;
            chg_str(1, dungeon, max_stats);
            msg_str("you feel stronger, now.  What bulging muscles!");
        }
        PotionType::MonsterFind => {
            dungeon.player.add_flag(MonsterFlags::SEEMONST);
            fuse(Daemon::TurnSee, true as u8 as i32, HUHDURATION, AFTER);
            if turn_see(false as u8, random, dungeon) == 0 {
                msg_str(&format!(
                    "you have a {} feeling for a moment, then it passes",
                    choose_str("normal", "strange")
                ));
            }
        }
        PotionType::TrapFind => {
            let floor = dungeon.with_level(|level| {
                level.items.iter().copied()
                    .filter(|&id| dungeon.objects.contains(id))
                    .collect::<Vec<_>>()
            });
            if !floor.is_empty() {
                ui.clear();
                for id in floor {
                    if is_magic_id(id, dungeon) {
                        show = true;
                        if let Some(pos) = dungeon.objects.with_object(id, |o| o.o_pos) {
                            ui.move_cursor(IVec2::new(pos.x, pos.y));
                            crate::draw::write_cell_glyph(pos, MAGIC as u8 as char);
                        }
                        (&mut *pot_info)[PotionType::TrapFind.index()].oi_know = true;
                    }
                }
                for id in dungeon.monster_list.ids() {
                    let (mp_pos, pack) = dungeon.monster_list
                        .with(id, |t| match t {
                            Thing::Monster { data } => (Some(data.t_pos), data.t_pack.clone()),
                            Thing::Object { .. } => (None, Vec::new()),
                        })
                        .unwrap_or((None, Vec::new()));
                    let Some(mp_pos) = mp_pos else {
                        continue;
                    };
                    for pack_id in pack {
                        if is_magic_id(pack_id, dungeon) {
                            show = true;
                            ui.move_cursor(IVec2::new(mp_pos.x, mp_pos.y));
                            crate::draw::write_cell_glyph(mp_pos, MAGIC as u8 as char);
                        }
                    }
                }
            }
            if show {
                (&mut *pot_info)[PotionType::TrapFind.index()].oi_know = true;
                show_win("You sense the presence of magic on this level.--More--");
            } else {
                msg_str(&format!(
                    "you have a {} feeling for a moment, then it passes",
                    choose_str("normal", "strange")
                ));
            }
        }
        PotionType::Lsd => {
            if !trip {
                if player_has(MonsterFlags::SEEMONST, dungeon) {
                    turn_see(false as u8, random, dungeon);
                }
                start_daemon(Daemon::Visuals, 0, BEFORE);
                dungeon.set_seen_stairs(seen_stairs(dungeon) != 0);
            }
            do_pot_impl(PotionType::Lsd, true, random, dungeon, pot_info, fruit);
        }
        PotionType::SeeInvisible => {
            show = player_has(MonsterFlags::CANSEE, dungeon);
            do_pot_impl(PotionType::SeeInvisible, false, random, dungeon, pot_info, fruit);
            if !show {
                invis_on(dungeon);
            }
            sight(random);
        }
        PotionType::Raise => {
            (&mut *pot_info)[PotionType::Raise.index()].oi_know = true;
            msg_str("you suddenly feel much more skillful");
            raise_level(random, dungeon, e_levels);
        }
        PotionType::ExtraHealing => {
            (&mut *pot_info)[PotionType::ExtraHealing.index()].oi_know = true;
            dungeon.player.with_stats_mut(|stats| {
                stats.hit_points += random.roll(stats.level, 8);
                if stats.hit_points > stats.max_hit_points {
                    if stats.hit_points > stats.max_hit_points + stats.level + 1 {
                        stats.max_hit_points += 1;
                    }
                    stats.max_hit_points += 1;
                    stats.hit_points = stats.max_hit_points;
                }
            });
            sight(random);
            come_down();
            msg_str("you begin to feel much better");
        }
        PotionType::Haste => {
            (&mut *pot_info)[PotionType::Haste.index()].oi_know = true;
            *after = false as u8;
            if add_haste(true, random) {
                msg_str("you feel yourself moving much faster");
            }
        }
        PotionType::Restore => {
            // Sum the `o_arm` bonus of each ring of add-strength (pointer-free).
            let equipment = dungeon.player.equipment();
            let ring_bonus = |hand: usize| {
                if equipment.ring_type(hand) == Some(RingType::AddStrength) {
                    equipment.ring_arm(hand).unwrap_or(0)
                } else {
                    0
                }
            };
            let left_bonus = ring_bonus(0);
            let right_bonus = ring_bonus(1);
            dungeon.player.with_stats_mut(|stats| {
                if left_bonus != 0 {
                    add_str(&mut stats.strength, -left_bonus);
                }
                if right_bonus != 0 {
                    add_str(&mut stats.strength, -right_bonus);
                }
                if stats.strength < max_strength {
                    stats.strength = max_strength;
                }
                if left_bonus != 0 {
                    add_str(&mut stats.strength, left_bonus);
                }
                if right_bonus != 0 {
                    add_str(&mut stats.strength, right_bonus);
                }
            });
            msg_str("hey, this tastes great.  It make you feel warm all over");
        }
        PotionType::Blind => do_pot_impl(PotionType::Blind, true, random, dungeon, pot_info, fruit),
        PotionType::Levitate => do_pot_impl(PotionType::Levitate, true, random, dungeon, pot_info, fruit),
    }

    // `call_it` retains its argument across input. Never lend it an exclusive
    // reference into the shared table; commit only the resulting guess.
    let mut info = (&*pot_info)[o_which as usize].clone();
    call_it(&mut info);
    let current = &mut (&mut *pot_info)[o_which as usize];
    if current.oi_know {
        current.oi_guess = None;
    } else if current.oi_guess.is_none() {
        current.oi_guess = info.oi_guess;
    }
    if discardit {
        let _ = dungeon.objects.remove(obj);
    }
}

/// Whether the object `id` radiates magic (pointer-free).
pub fn is_magic_id(id: ThingId, dungeon: &Dungeon) -> bool {
    let Some((typ, prot, o_arm, o_hplus, o_dplus)) = dungeon.objects.with_object(id, |o| {
        (
            o.o_type,
            o.o_flags.contains(ObjectFlags::PROT),
            o.o_arm,
            o.o_hplus,
            o.o_dplus,
        )
    }) else {
        return false;
    };
    match typ {
        ItemType::Armor(_) => prot || o_arm != 0,
        ItemType::Weapon(_) => o_hplus != 0 || o_dplus != 0,
        ItemType::Potion(_)
        | ItemType::Scroll(_)
        | ItemType::Stick(_)
        | ItemType::Ring(_)
        | ItemType::Amulet => true,
        _ => false,
    }
}

/// invis_on:
/// Turn on the ability to see invisible.
pub unsafe fn invis_on(dungeon: &Dungeon) {
    dungeon.player.add_flag(MonsterFlags::CANSEE);
    for id in dungeon.monster_list.ids() {
        if thing_has(id, MonsterFlags::INVIS, dungeon)
            && see_monst(id, dungeon) != 0
            && !player_has(MonsterFlags::HALU, dungeon)
        {
            if let Some((pos, glyph)) = dungeon.monster_list
                .with(id, |t| match t {
                    Thing::Monster { data } => Some((data.t_pos, data.t_disguise as char)),
                    Thing::Object { .. } => None,
                })
                .flatten()
            {
                crate::draw::write_cell_glyph(pos, glyph);
            }
        }
    }
}

/// turn_see:
/// Put on or off seeing monsters on this level.
pub unsafe fn turn_see(turn_off: u8, random: &mut Random, dungeon: &Dungeon) -> u8 {
    let mut add_new = 0;

    for id in dungeon.monster_list.ids() {
        if let Some((pos, oldch, type_glyph)) = dungeon.monster_list
            .with(id, |t| match t {
                Thing::Monster { data } => Some((
                    data.t_pos,
                    data.t_oldch,
                    data.t_type.map_or(' ', |kind| kind.glyph() as char),
                )),
                Thing::Object { .. } => None,
            })
            .flatten()
        {
            let can_see = see_monst(id, dungeon) != 0;
            if turn_off != 0 {
                if !can_see {
                    crate::draw::write_cell_glyph(pos, oldch as char);
                }
            } else {
                let glyph = if !player_has(MonsterFlags::HALU, dungeon) {
                    type_glyph
                } else {
                    crate::draw::hallucination_glyph(random)
                };
                if can_see {
                    crate::draw::write_cell_glyph(pos, glyph);
                } else {
                    crate::draw::write_reverse_video_cell_glyph(pos, glyph);
                    add_new += 1;
                }
            }
        }
    }

    if turn_off != 0 {
        dungeon.player.remove_flag(MonsterFlags::SEEMONST);
    } else {
        dungeon.player.add_flag(MonsterFlags::SEEMONST);
    }

    if add_new != 0 {
        1
    } else {
        0
    }
}

/// seen_stairs:
/// Return true if the player has seen the stairs.
pub unsafe fn seen_stairs(dungeon: &Dungeon) -> u8 {
    let stairs = dungeon.stairs();

    if crate::draw::screen_glyph_at(stairs) as i32 == STAIRS {
        return 1;
    }
    if hero(dungeon).x == stairs.x && hero(dungeon).y == stairs.y {
        return 1;
    }

    if let Some(tp) = moat(stairs.y, stairs.x, dungeon) {
        if see_monst(tp, dungeon) != 0 && thing_has(tp, MonsterFlags::RUN, dungeon) {
            return 1;
        }
        let oldch = dungeon.monster_list
            .with(tp, |t| match t {
                Thing::Monster { data } => data.t_oldch as i32,
                Thing::Object { .. } => 0,
            })
            .unwrap_or(0);
        if player_has(MonsterFlags::SEEMONST, dungeon) && oldch == STAIRS {
            return 1;
        }
    }

    0
}

/// raise_level:
/// The player just magically went up a level.
/// `e_levels` must contain all 21 experience entries, including the terminator.
pub unsafe fn raise_level(random: &mut Random, dungeon: &Dungeon, e_levels: &[i32]) {
    let e_levels: [i32; 21] = std::array::from_fn(|i| e_levels[i]);
    let level = dungeon.player.level();
    dungeon.player.with_stats_mut(|stats| stats.experience = e_levels[level as usize - 1] + 1);
    check_level(random, dungeon, &e_levels);
}

/// do_pot:
/// Do a potion with the standard fuse/flag setup.
unsafe fn do_pot(
    type_id: i32,
    knowit: bool,
    random: &mut Random,
    dungeon: &Dungeon,
    pot_info: *mut [ObjInfo],
    fruit: &str,
) {
    do_pot_impl(PotionType::from_raw(type_id), knowit, random, dungeon, pot_info, fruit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_detection_uses_the_supplied_dungeon() {
        let dungeon = Dungeon::empty();
        let other = Dungeon::empty();
        let id = dungeon.objects.new_item_id();
        let other_id = other.objects.new_item_id();
        assert_eq!(id, other_id);
        dungeon.objects.with_object_mut(id, |o| o.o_type = ItemType::POTION);
        other.objects.with_object_mut(other_id, |o| o.o_type = ItemType::Food);

        assert!(is_magic_id(id, &dungeon));
        assert!(!is_magic_id(other_id, &other));
        dungeon.objects.remove(id);
        assert!(!is_magic_id(id, &dungeon));
    }

    #[test]
    fn magic_detection_preserves_equipment_rules() {
        let dungeon = Dungeon::empty();
        let id = dungeon.objects.new_item_id();
        dungeon.objects.with_object_mut(id, |o| {
            o.o_type = ItemType::Armor(0);
            o.o_arm = 0;
            o.o_flags = ObjectFlags::NONE;
        });
        assert!(!is_magic_id(id, &dungeon));
        dungeon.objects.with_object_mut(id, |o| o.o_flags.insert(ObjectFlags::PROT));
        assert!(is_magic_id(id, &dungeon));
        dungeon.objects.with_object_mut(id, |o| {
            o.o_type = ItemType::Weapon(0);
            o.o_hplus = 0;
            o.o_dplus = -1;
        });
        assert!(is_magic_id(id, &dungeon));
        dungeon.objects.with_object_mut(id, |o| o.o_dplus = 0);
        assert!(!is_magic_id(id, &dungeon));
    }
}
