//! Potions and quaffing.
//!
//! Ported from `src/c/potions.c` to Rust.
use crate::rnd::rnd;

use crate::daemon::{fuse, lengthen, start_daemon, Daemon};
use crate::daemons::{come_down, sight};
use crate::draw::look;

use crate::entity::chase::see_monst;
use crate::entity::player::{MonsterFlags, ObjectFlags, Thing};
use crate::game::globals::{pot_info, ObjInfo};
use crate::game::PLAYER;
use crate::game::{MonsterId, MONSTER_LIST};
use crate::item::arena::{ThingId, OBJECTS};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::{get_item_id, leave_pack_id};
use crate::item::rings::RingType;
use crate::misc::{add_haste, add_str, call_it, check_level, chg_str, choose_str, spread};
use crate::startup::roll;
use crate::ui::output::{msg_str, show_win};
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

/// Process-wide game state and UI flags read by the potion effects.
use crate::game::globals::{after, e_levels, max_stats, seenstairs, terse};

/// A mutable reference to the static `pot_info` entry at `index`.
///
/// Confined to this module: the returned reference lets the quaff logic update
/// the `oi_know` flag in place. (The game is single-threaded and the table is
/// only ever touched through this accessor.)
#[inline]
unsafe fn pot_info_at(index: usize) -> &'static mut ObjInfo {
    &mut pot_info[index]
}

#[inline]
fn hero() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    crate::game::PLAYER.has_flag(flag)
}

#[inline]
fn thing_has(id: MonsterId, flag: MonsterFlags) -> bool {
    MONSTER_LIST
        .with(id, |t| match t {
            Thing::Monster { data } => data.t_flags.contains(flag),
            Thing::Object { .. } => false,
        })
        .unwrap_or(false)
}

#[inline]
fn moat(y: i32, x: i32) -> Option<MonsterId> {
    crate::game::monster_id_at(y, x)
}

/// Shared implementation for potion effects that need the normal fuse/flag
/// setup and knowledge tracking used by the C version.
unsafe fn do_pot_impl(potion: PotionType, knowit: bool) {
    let (flags, daemon, base_time, high_msg, straight_msg): (
        MonsterFlags,
        Option<Daemon>,
        i32,
        String,
        String,
    ) = {
        let taste = format!(
            "this potion tastes like {} juice",
            crate::game::globals::fruit()
        );
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

    pot_info_at(potion.index()).oi_know = knowit;

    let Some(daemon) = daemon else {
        return;
    };
    if flags.is_empty() {
        return;
    }

    let t = spread(base_time);
    if !player_has(flags) {
        crate::game::PLAYER.add_flag(flags);
        fuse(daemon, 0, t, AFTER);
        look(false as u8);
    } else {
        lengthen(daemon, t);
    }
    let chosen = if player_has(MonsterFlags::HALU) {
        high_msg
    } else {
        straight_msg
    };
    msg_str(&chosen);
}

/// quaff:
/// Quaff a potion from the pack.
pub unsafe fn quaff() {
    let Some(obj) = get_item_id("quaff", ItemFilter::Category(ItemType::POTION)) else {
        return;
    };
    let mut show = false;
    let trip = player_has(MonsterFlags::HALU);

    if !matches!(
        OBJECTS.with_object(obj, |o| o.o_type),
        Some(ItemType::Potion(_))
    ) {
        if terse == 0 {
            msg_str("yuk! Why would you want to drink that?");
        } else {
            msg_str("that's undrinkable");
        }
        return;
    }
    if PLAYER.equipment().weapon_id() == Some(obj) {
        PLAYER.set_weapon_id(None);
    }

    let (o_count, o_which) = OBJECTS
        .with_object(obj, |o| (o.o_count, o.o_which))
        .unwrap_or((0, 0));
    let discardit = o_count == 1;
    leave_pack_id(obj, false, false);

    let potion = PotionType::from_raw(o_which);
    match potion {
        PotionType::Confuse => do_pot_impl(PotionType::Confuse, if trip { false } else { true }),
        PotionType::Poison => {
            pot_info_at(PotionType::Poison.index()).oi_know = true;
            if PLAYER.wearing_ring(RingType::SustainStrength) {
                msg_str("you feel momentarily sick");
            } else {
                chg_str(-(rnd(3) + 1));
                msg_str("you feel very sick now");
                come_down();
            }
        }
        PotionType::Healing => {
            pot_info_at(PotionType::Healing.index()).oi_know = true;
            crate::game::PLAYER.with_stats_mut(|stats| {
                stats.hit_points += roll(stats.level, 4);
                if stats.hit_points > stats.max_hit_points {
                    stats.max_hit_points += 1;
                    stats.hit_points = stats.max_hit_points;
                }
            });
            sight();
            msg_str("you begin to feel better");
        }
        PotionType::Strength => {
            pot_info_at(PotionType::Strength.index()).oi_know = true;
            chg_str(1);
            msg_str("you feel stronger, now.  What bulging muscles!");
        }
        PotionType::MonsterFind => {
            crate::game::PLAYER.add_flag(MonsterFlags::SEEMONST);
            fuse(Daemon::TurnSee, true as u8 as i32, HUHDURATION, AFTER);
            if turn_see(false as u8) == 0 {
                msg_str(&format!(
                    "you have a {} feeling for a moment, then it passes",
                    choose_str("normal", "strange")
                ));
            }
        }
        PotionType::TrapFind => {
            let floor = crate::game::item_ids();
            if !floor.is_empty() {
                crate::ui::terminal::clear();
                for id in floor {
                    if is_magic_id(id) {
                        show = true;
                        if let Some(pos) = OBJECTS.with_object(id, |o| o.o_pos) {
                            crate::ui::terminal::move_cursor(IVec2::new(pos.x, pos.y));
                            crate::draw::write_cell_glyph(pos, MAGIC as u8 as char);
                        }
                        pot_info_at(PotionType::TrapFind.index()).oi_know = true;
                    }
                }
                for id in MONSTER_LIST.ids() {
                    let (mp_pos, pack) = MONSTER_LIST
                        .with(id, |t| match t {
                            Thing::Monster { data } => (Some(data.t_pos), data.t_pack.clone()),
                            Thing::Object { .. } => (None, Vec::new()),
                        })
                        .unwrap_or((None, Vec::new()));
                    let Some(mp_pos) = mp_pos else {
                        continue;
                    };
                    for pack_id in pack {
                        if is_magic_id(pack_id) {
                            show = true;
                            crate::ui::terminal::move_cursor(IVec2::new(mp_pos.x, mp_pos.y));
                            crate::draw::write_cell_glyph(mp_pos, MAGIC as u8 as char);
                        }
                    }
                }
            }
            if show {
                pot_info_at(PotionType::TrapFind.index()).oi_know = true;
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
                if player_has(MonsterFlags::SEEMONST) {
                    turn_see(false as u8);
                }
                start_daemon(Daemon::Visuals, 0, BEFORE);
                seenstairs = seen_stairs();
            }
            do_pot_impl(PotionType::Lsd, true);
        }
        PotionType::SeeInvisible => {
            show = player_has(MonsterFlags::CANSEE);
            do_pot_impl(PotionType::SeeInvisible, false);
            if !show {
                invis_on();
            }
            sight();
        }
        PotionType::Raise => {
            pot_info_at(PotionType::Raise.index()).oi_know = true;
            msg_str("you suddenly feel much more skillful");
            raise_level();
        }
        PotionType::ExtraHealing => {
            pot_info_at(PotionType::ExtraHealing.index()).oi_know = true;
            crate::game::PLAYER.with_stats_mut(|stats| {
                stats.hit_points += roll(stats.level, 8);
                if stats.hit_points > stats.max_hit_points {
                    if stats.hit_points > stats.max_hit_points + stats.level + 1 {
                        stats.max_hit_points += 1;
                    }
                    stats.max_hit_points += 1;
                    stats.hit_points = stats.max_hit_points;
                }
            });
            sight();
            come_down();
            msg_str("you begin to feel much better");
        }
        PotionType::Haste => {
            pot_info_at(PotionType::Haste.index()).oi_know = true;
            after = false as u8;
            if add_haste(true) {
                msg_str("you feel yourself moving much faster");
            }
        }
        PotionType::Restore => {
            // Sum the `o_arm` bonus of each ring of add-strength (pointer-free).
            let equipment = PLAYER.equipment();
            let ring_bonus = |hand: usize| {
                if equipment.ring_type(hand) == Some(RingType::AddStrength) {
                    equipment.ring_arm(hand).unwrap_or(0)
                } else {
                    0
                }
            };
            let left_bonus = ring_bonus(0);
            let right_bonus = ring_bonus(1);
            crate::game::PLAYER.with_stats_mut(|stats| {
                if left_bonus != 0 {
                    add_str(&mut stats.strength, -left_bonus);
                }
                if right_bonus != 0 {
                    add_str(&mut stats.strength, -right_bonus);
                }
                if stats.strength < max_stats.strength {
                    stats.strength = max_stats.strength;
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
        PotionType::Blind => do_pot_impl(PotionType::Blind, true),
        PotionType::Levitate => do_pot_impl(PotionType::Levitate, true),
        _ => {
            msg_str("what an odd tasting potion!");
            return;
        }
    }

    call_it(pot_info_at(o_which as usize));
    if discardit {
        let _ = OBJECTS.remove(obj);
    }
}

/// Whether the object `id` radiates magic (pointer-free).
pub fn is_magic_id(id: ThingId) -> bool {
    let Some((typ, prot, o_arm, o_hplus, o_dplus)) = OBJECTS.with_object(id, |o| {
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
pub unsafe fn invis_on() {
    crate::game::PLAYER.add_flag(MonsterFlags::CANSEE);
    for id in MONSTER_LIST.ids() {
        if thing_has(id, MonsterFlags::INVIS)
            && see_monst(id) != 0
            && !player_has(MonsterFlags::HALU)
        {
            if let Some(pos) = MONSTER_LIST
                .with(id, |t| match t {
                    Thing::Monster { data } => Some(data.t_pos),
                    Thing::Object { .. } => None,
                })
                .flatten()
            {
                crate::draw::write_cell_glyph(pos, crate::draw::monster_glyph(id));
            }
        }
    }
}

/// turn_see:
/// Put on or off seeing monsters on this level.
pub unsafe fn turn_see(turn_off: u8) -> u8 {
    let mut add_new = 0;

    for id in MONSTER_LIST.ids() {
        if let Some((pos, oldch)) = MONSTER_LIST
            .with(id, |t| match t {
                Thing::Monster { data } => Some((data.t_pos, data.t_oldch)),
                Thing::Object { .. } => None,
            })
            .flatten()
        {
            let can_see = see_monst(id) != 0;
            if turn_off != 0 {
                if !can_see {
                    crate::draw::write_cell_glyph(pos, oldch as char);
                }
            } else {
                let glyph = if !player_has(MonsterFlags::HALU) {
                    crate::draw::monster_type_glyph(id)
                } else {
                    crate::draw::hallucination_glyph()
                };
                if can_see {
                    crate::draw::write_cell_glyph(pos, glyph);
                } else {
                    crate::draw::write_standout_cell_glyph(pos, glyph);
                    add_new += 1;
                }
            }
        }
    }

    if turn_off != 0 {
        crate::game::PLAYER.remove_flag(MonsterFlags::SEEMONST);
    } else {
        crate::game::PLAYER.add_flag(MonsterFlags::SEEMONST);
    }

    if add_new != 0 {
        1
    } else {
        0
    }
}

/// seen_stairs:
/// Return true if the player has seen the stairs.
pub unsafe fn seen_stairs() -> u8 {
    let stairs = crate::game::stairs();

    if crate::draw::screen_glyph_at(stairs) as i32 == STAIRS {
        return 1;
    }
    if hero().x == stairs.x && hero().y == stairs.y {
        return 1;
    }

    if let Some(tp) = moat(stairs.y, stairs.x) {
        if see_monst(tp) != 0 && thing_has(tp, MonsterFlags::RUN) {
            return 1;
        }
        let oldch = MONSTER_LIST
            .with(tp, |t| match t {
                Thing::Monster { data } => data.t_oldch as i32,
                Thing::Object { .. } => 0,
            })
            .unwrap_or(0);
        if player_has(MonsterFlags::SEEMONST) && oldch == STAIRS {
            return 1;
        }
    }

    0
}

/// raise_level:
/// The player just magically went up a level.
pub unsafe fn raise_level() {
    let level = crate::game::PLAYER.level();
    crate::game::PLAYER.with_stats_mut(|stats| stats.experience = e_levels[level as usize - 1] + 1);
    check_level();
}

/// do_pot:
/// Do a potion with the standard fuse/flag setup.
unsafe fn do_pot(type_id: i32, knowit: bool) {
    do_pot_impl(PotionType::from_raw(type_id), knowit);
}
