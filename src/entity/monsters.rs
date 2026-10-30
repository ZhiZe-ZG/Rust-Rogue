//! Monster creation and behaviour.
//!
//! Ported from `src/c/monsters.c` to Rust.
//!
//! Raw table pointers must remain valid and initialized throughout each call,
//! including reentrant helpers; callers serialize access and avoid conflicting
//! live references. Tables passed onward by reference are snapshotted first.
use crate::config::GameConfig;
use crate::daemon::{fuse, lengthen, Daemon};
use crate::entity::chase::{dist, roomin, runto};
use crate::entity::fight::set_mname;
use crate::entity::player::DestRef;
use crate::entity::player::{MonsterFlags, Thing};
use crate::dungeon::Dungeon;
use crate::game::MonsterId;
use crate::item::rings::RingType;
use crate::item::things::new_thing_id;
use crate::misc::{rnd_thing, spread};
use crate::rnd::Random;
use crate::ui::output::{addmsg_str, flush_now, msg_str};
use glam::IVec2;

use crate::game::globals::ObjInfo;

const LAMPDIST: i32 = 3;
const HUHDURATION: i32 = 20;
const AFTER: i32 = 2;
const VS_MAGIC: i32 = 0o03;

pub use crate::game::globals::MonsterInfo;

/// The identity of a monster kind — the typed replacement for the legacy
/// `t_type` ASCII letter (`'A'`..=`'Z'`).
///
/// The variant order matches the `monsters` stat table in
/// [`crate::game::globals`] (`Aquator` = index 0 … `Zombie` = index 25), so
/// [`MonsterType::index`] is a direct table index and [`MonsterType::glyph`]
/// reproduces the original `'A'..='Z'` byte used by the save format and by
/// [`crate::game::globals::monsters`].
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum MonsterType {
    Aquator = b'A',
    Bat = b'B',
    Centaur = b'C',
    Dragon = b'D',
    Emu = b'E',
    VenusFlytrap = b'F',
    Griffin = b'G',
    Hobgoblin = b'H',
    IceMonster = b'I',
    Jabberwock = b'J',
    Kestrel = b'K',
    Leprechaun = b'L',
    Medusa = b'M',
    Nymph = b'N',
    Orc = b'O',
    Phantom = b'P',
    Quagga = b'Q',
    Rattlesnake = b'R',
    Snake = b'S',
    Troll = b'T',
    BlackUnicorn = b'U',
    Vampire = b'V',
    Wraith = b'W',
    Xeroc = b'X',
    Yeti = b'Y',
    Zombie = b'Z',
}

impl MonsterType {
    /// Number of monster kinds (the size of the `monsters` stat table).
    pub const COUNT: usize = 26;

    /// The legacy `'A'..='Z'` identity byte.
    #[inline]
    pub const fn glyph(self) -> u8 {
        self as u8
    }

    /// The zero-based index into the `monsters` stat table.
    #[inline]
    pub const fn index(self) -> usize {
        (self as u8 - b'A') as usize
    }

    /// Rebuild a kind from a legacy identity byte (`None` for the hero's `0`
    /// or any non-`'A'..='Z'` byte).
    #[inline]
    pub const fn from_glyph(ch: u8) -> Option<Self> {
        Some(match ch {
            b'A' => Self::Aquator,
            b'B' => Self::Bat,
            b'C' => Self::Centaur,
            b'D' => Self::Dragon,
            b'E' => Self::Emu,
            b'F' => Self::VenusFlytrap,
            b'G' => Self::Griffin,
            b'H' => Self::Hobgoblin,
            b'I' => Self::IceMonster,
            b'J' => Self::Jabberwock,
            b'K' => Self::Kestrel,
            b'L' => Self::Leprechaun,
            b'M' => Self::Medusa,
            b'N' => Self::Nymph,
            b'O' => Self::Orc,
            b'P' => Self::Phantom,
            b'Q' => Self::Quagga,
            b'R' => Self::Rattlesnake,
            b'S' => Self::Snake,
            b'T' => Self::Troll,
            b'U' => Self::BlackUnicorn,
            b'V' => Self::Vampire,
            b'W' => Self::Wraith,
            b'X' => Self::Xeroc,
            b'Y' => Self::Yeti,
            b'Z' => Self::Zombie,
            _ => return None,
        })
    }

    /// The monster's display name (from the `monsters` stat table).
    #[inline]
    pub fn name(self, monsters: &[MonsterInfo]) -> &'static str {
        monsters[self.index()].m_name
    }
}

/// Monster-type letters (`'A'..='Z'`) ordered weakest → strongest and indexed by
/// an adjusted dungeon depth (see [`randmonster`]).
///
/// `LVL_MONS[0]` is the weakest monster tier and `LVL_MONS[25]` the strongest.
/// This is the source-of-truth ordering for *normal* (non-wandering) monster
/// spawns, mirroring the `lvl_mons[]` table in the original `src/c/monsters.c`.
static LVL_MONS: [MonsterType; 26] = [
    MonsterType::Kestrel,
    MonsterType::Emu,
    MonsterType::Bat,
    MonsterType::Snake,
    MonsterType::Hobgoblin,
    MonsterType::IceMonster,
    MonsterType::Rattlesnake,
    MonsterType::Orc,
    MonsterType::Zombie,
    MonsterType::Leprechaun,
    MonsterType::Centaur,
    MonsterType::Quagga,
    MonsterType::Aquator,
    MonsterType::Nymph,
    MonsterType::Yeti,
    MonsterType::VenusFlytrap,
    MonsterType::Troll,
    MonsterType::Wraith,
    MonsterType::Phantom,
    MonsterType::Xeroc,
    MonsterType::BlackUnicorn,
    MonsterType::Medusa,
    MonsterType::Vampire,
    MonsterType::Griffin,
    MonsterType::Jabberwock,
    MonsterType::Dragon,
];

/// Like [`LVL_MONS`], but for *wandering* monster spawns.
///
/// The `None` entries are deliberate "holes": monster tiers excluded from
/// wandering spawns because they are too strong to appear as a roamer.
/// [`randmonster`] rerolls whenever it lands on a hole, so an absent tier is
/// never spawned this way. Mirrors the `wand_mons[]` table in the original
/// `src/c/monsters.c`.
static WAND_MONS: [Option<MonsterType>; 26] = [
    Some(MonsterType::Kestrel),
    Some(MonsterType::Emu),
    Some(MonsterType::Bat),
    Some(MonsterType::Snake),
    Some(MonsterType::Hobgoblin),
    None,
    Some(MonsterType::Rattlesnake),
    Some(MonsterType::Orc),
    Some(MonsterType::Zombie),
    None,
    Some(MonsterType::Centaur),
    Some(MonsterType::Quagga),
    Some(MonsterType::Aquator),
    None,
    Some(MonsterType::Yeti),
    None,
    Some(MonsterType::Troll),
    Some(MonsterType::Wraith),
    Some(MonsterType::Phantom),
    None,
    Some(MonsterType::BlackUnicorn),
    Some(MonsterType::Medusa),
    Some(MonsterType::Vampire),
    Some(MonsterType::Griffin),
    Some(MonsterType::Jabberwock),
    None,
];

#[inline]
fn has_flag(id: MonsterId, flag: MonsterFlags, dungeon: &Dungeon) -> bool {
    dungeon.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => data.t_flags.contains(flag),
            Thing::Object { .. } => false,
        })
        .unwrap_or(false)
}

#[inline]
fn player_has(flag: MonsterFlags, dungeon: &Dungeon) -> bool {
    dungeon.player.has_flag(flag)
}

#[inline]
fn iswearing(which: RingType, dungeon: &Dungeon) -> bool {
    dungeon.player.wearing_ring(which)
}

/// Picks an appropriate monster kind for the current depth.
pub unsafe fn randmonster(wander: bool, random: &mut Random, dungeon: &Dungeon) -> MonsterType {
    let level = dungeon.current_depth();
    loop {
        let mut d = level + (random.rnd(10) - 6);
        if d < 0 {
            d = random.rnd(5);
        }
        if d > 25 {
            d = random.rnd(5) + 21;
        }
        let m = if wander {
            WAND_MONS[d as usize]
        } else {
            Some(LVL_MONS[d as usize])
        };
        if let Some(kind) = m {
            return kind;
        }
    }
}

/// Initializes the already-spawned monster `id` and places it on the map.
pub unsafe fn new_monster_id(
    id: MonsterId, monster_type: MonsterType, cp: IVec2, random: &mut Random,
    dungeon: &Dungeon, monsters: *const [MonsterInfo; 26],
) {
    let level = dungeon.current_depth();
    let mut lev_add = level - GameConfig::AMULET_LEVEL;
    if lev_add < 0 {
        lev_add = 0;
    }

    // `id` was already allocated into this dungeon by `spawn_actor`.

    let oldch = crate::entity::player::cell_glyph(cp.y, cp.x, dungeon);
    let room = roomin(cp, dungeon);
    // Record the monster in the per-cell occupancy map.
    dungeon.monster_map.set(cp.y as usize, cp.x as usize, Some(id));

    let mp = (*monsters)[monster_type.index()];
    dungeon.monster_list.with_mut(id, |t| {
        let Thing::Monster { data } = t else {
            return;
        };
        data.t_type = Some(monster_type);
        data.t_disguise = monster_type.glyph();
        data.t_pos = cp;
        data.t_oldch = oldch;
        data.t_room = room;
        data.t_stats.level = mp.m_stats.level + lev_add;
        data.t_stats.max_hit_points = random.roll(data.t_stats.level, 8);
        data.t_stats.hit_points = data.t_stats.max_hit_points;
        data.t_stats.armor = mp.m_stats.armor - lev_add;
        data.t_stats.damage = mp.m_stats.damage;
        data.t_stats.strength = mp.m_stats.strength;
        data.t_stats.experience = mp.m_stats.experience
            + lev_add * 10
            + exp_add_for(data.t_stats.level, data.t_stats.max_hit_points);
        data.t_flags = MonsterFlags::from_bits(mp.m_flags);
        if level > 29 {
            data.t_flags.insert(MonsterFlags::HASTE);
        }
        data.t_turn = true;
        data.t_pack = Vec::new();
        if monster_type == MonsterType::Xeroc {
            data.t_disguise = rnd_thing(random) as u8;
        }
    });

    if iswearing(RingType::Aggravate, dungeon) {
        runto(cp, random, dungeon, monsters);
    }
}

/// Computes bonus experience from a monster's level and max HP.
pub fn exp_add(id: MonsterId, dungeon: &Dungeon) -> i32 {
    let (level, max_hp) = dungeon.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => (data.t_stats.level, data.t_stats.max_hit_points),
            Thing::Object { .. } => (0, 0),
        })
        .unwrap_or((0, 0));
    exp_add_for(level, max_hp)
}

/// Bonus experience from an explicit monster `level` and `max_hp`.
#[inline]
fn exp_add_for(level: i32, max_hp: i32) -> i32 {
    let mut modu = if level == 1 { max_hp / 8 } else { max_hp / 6 };
    if level > 9 {
        modu *= 20;
    } else if level > 6 {
        modu *= 4;
    }
    modu
}

/// Spawns a wandering monster in a different room and sets it running toward the hero.
pub unsafe fn wanderer(
    random: &mut Random, dungeon: &Dungeon,
    monsters: *const [MonsterInfo; 26], wizard: bool,
) {
    let id = dungeon.monster_list.spawn_actor();
    let mut cp;

    loop {
        cp = dungeon.find_floor(None, 0, true, random).unwrap_or(IVec2::ZERO);
        if roomin(cp, dungeon) != dungeon.player.room() {
            break;
        }
    }

    new_monster_id(id, randmonster(true, random, dungeon), cp, random, dungeon, monsters);

    if player_has(MonsterFlags::SEEMONST, dungeon) {
        let glyph = if !player_has(MonsterFlags::HALU, dungeon) {
            dungeon.monster_list.with(id, |thing| match thing {
                Thing::Monster { data } => data.t_type.map_or(' ', |kind| kind.glyph() as char),
                Thing::Object { .. } => ' ',
            }).unwrap_or(' ')
        } else {
            crate::draw::hallucination_glyph(random)
        };
        crate::draw::write_reverse_video_cell_glyph(cp, glyph);
    }

    let pos = dungeon.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => data.t_pos,
            Thing::Object { .. } => IVec2::ZERO,
        })
        .unwrap_or(IVec2::ZERO);
    runto(pos, random, dungeon, monsters);

    if wizard {
        let name = dungeon.monster_list
            .with(id, |t| match t {
                Thing::Monster { data } => data.t_type.map_or("", |m| m.name(&*monsters)).to_string(),
                Thing::Object { .. } => String::new(),
            })
            .unwrap_or_default();
        msg_str(&format!("started a wandering {name}"));
    }
}

/// Wakes and updates an adjacent monster's pursuit behavior and special gaze logic.
pub unsafe fn wake_monster(
    y: i32, x: i32, random: &mut Random, dungeon: &Dungeon,
    monsters: *const [MonsterInfo; 26],
) -> Option<MonsterId> {
    let Some(id) = dungeon.monster_map.at(y as usize, x as usize) else {
        flush_now();
        std::process::abort();
    };

    let ch = dungeon.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => data.t_type,
            Thing::Object { .. } => None,
        })
        .flatten();

    if !has_flag(id, MonsterFlags::RUN, dungeon)
        && random.rnd(3) != 0
        && has_flag(id, MonsterFlags::MEAN, dungeon)
        && !has_flag(id, MonsterFlags::HELD, dungeon)
        && !iswearing(RingType::Stealth, dungeon)
        && !player_has(MonsterFlags::LEVIT, dungeon)
    {
        crate::entity::player::set_monster_dest_hero(id, dungeon);
        dungeon.monster_list.with_mut(id, |t| {
            if let Thing::Monster { data } = t {
                data.t_flags.insert(MonsterFlags::RUN);
            }
        });
    }

    if ch == Some(MonsterType::Medusa)
        && !player_has(MonsterFlags::BLIND, dungeon)
        && !player_has(MonsterFlags::HALU, dungeon)
        && !has_flag(id, MonsterFlags::FOUND, dungeon)
        && !has_flag(id, MonsterFlags::CANCELLED, dungeon)
        && has_flag(id, MonsterFlags::RUN, dungeon)
    {
        let rp = dungeon.player.room();
        let hero = dungeon.player.pos();
        if (rp.is_some() && !dungeon.with_level(|level| level.room_dark(rp))) || dist(y, x, hero.y, hero.x) < LAMPDIST {
            dungeon.monster_list.with_mut(id, |t| {
                if let Thing::Monster { data } = t {
                    data.t_flags.insert(MonsterFlags::FOUND);
                }
            });
            if save(VS_MAGIC, random, dungeon) == 0 {
                if player_has(MonsterFlags::HUH, dungeon) {
                    lengthen(Daemon::Unconfuse, spread(HUHDURATION, random));
                } else {
                    fuse(Daemon::Unconfuse, 0, spread(HUHDURATION, random), AFTER);
                }
                dungeon.player.add_flag(MonsterFlags::HUH);
                let mname_str = set_mname(id, random, dungeon, monsters);
                addmsg_str(&mname_str);
                if mname_str != "it" {
                    addmsg_str("'");
                }
                msg_str("s gaze has confused you");
            }
        }
    }

    if has_flag(id, MonsterFlags::GREED, dungeon) && !has_flag(id, MonsterFlags::RUN, dungeon) {
        dungeon.monster_list.with_mut(id, |t| {
            if let Thing::Monster { data } = t {
                data.t_flags.insert(MonsterFlags::RUN);
            }
        });
        let pr = dungeon.player.room();
        if let Some(room) = pr {
            if dungeon.with_level(|level| level.room_goldval(pr)) != 0 {
                crate::entity::player::set_monster_dest(id, DestRef::RoomGold(room), dungeon);
            } else {
                crate::entity::player::set_monster_dest_hero(id, dungeon);
            }
        } else {
            crate::entity::player::set_monster_dest_hero(id, dungeon);
        }
    }

    Some(id)
}

/// Potentially gives a monster a carried item based on depth and monster carry chance.
pub unsafe fn give_pack_id(
    id: MonsterId,
    random: &mut Random,
    dungeon: &Dungeon,
    monsters: *const [MonsterInfo; 26],
    things: *const [ObjInfo; 7],
    arm_info: *const [ObjInfo; 8],
    pot_info: *const [ObjInfo; 14],
    ring_info: *const [ObjInfo; 14],
    scr_info: *const [ObjInfo; 18],
    weap_info: *const [ObjInfo; 10],
    ws_info: *const [ObjInfo; 14],
    a_class: *const [i32; 8],
    group: *mut i32,
) {
    let kind = dungeon.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => data.t_type,
            Thing::Object { .. } => None,
        })
        .flatten();
    let carry = (*monsters)[kind.map_or(0, |m| m.index())].m_carry;
    if dungeon.current_depth() >= dungeon.max_depth()
        && random.rnd(100) < carry
    {
        let things = (*things).clone();
        let arm_info = (*arm_info).clone();
        let pot_info = (*pot_info).clone();
        let ring_info = (*ring_info).clone();
        let scr_info = (*scr_info).clone();
        let weap_info = (*weap_info).clone();
        let ws_info = (*ws_info).clone();
        let a_class = *a_class;
        let item = new_thing_id(
            random,
            dungeon,
            &things,
            &arm_info,
            &pot_info,
            &ring_info,
            &scr_info,
            &weap_info,
            &ws_info,
            &a_class,
            group,
        );
        dungeon.monster_list.with_mut(id, |t| {
            if let Thing::Monster { data } = t {
                data.t_pack.insert(0, item);
            }
        });
    }
}

/// Roll a saving throw for the monster behind `id` (pointer-free variant).
pub fn save_throw_id(which: i32, id: crate::game::MonsterId, random: &mut Random, dungeon: &Dungeon) -> i32 {
    let level = dungeon.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => data.t_stats.level,
            Thing::Object { .. } => 0,
        })
        .unwrap_or(0);
    save_throw_for_level(which, level, random)
}

/// Roll a saving throw using an explicit caster level (used for the hero, whose
/// `Thing` is no longer reachable as a raw pointer).
#[inline]
fn save_throw_for_level(which: i32, level: i32, random: &mut Random) -> i32 {
    let need = 14 + which - level / 2;
    if random.roll(1, 20) >= need
    {
        1
    } else {
        0
    }
}

/// Rolls the hero's saving throw, applying ring of protection magic adjustment.
pub unsafe fn save(which: i32, random: &mut Random, dungeon: &Dungeon) -> i32 {
    let mut adj = which;
    if which == VS_MAGIC {
        // Ring of protection lowers the save magic number by its `o_arm`.
        let equipment = dungeon.player.equipment();
        for hand in 0..2usize {
            if equipment.ring_type(hand) == Some(RingType::Protection) {
                adj -= equipment.ring_arm(hand).unwrap_or(0);
            }
        }
    }
    save_throw_for_level(adj, dungeon.player.level(), random)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experience_reads_only_the_supplied_dungeon() {
        let first = Dungeon::empty();
        let second = Dungeon::empty();
        let a = first.monster_list.spawn_actor();
        let b = second.monster_list.spawn_actor();
        first.monster_list.with_mut(a, |thing| {
            if let Thing::Monster { data } = thing {
                data.t_stats.level = 10;
                data.t_stats.max_hit_points = 60;
            }
        });
        second.monster_list.with_mut(b, |thing| {
            if let Thing::Monster { data } = thing {
                data.t_stats.level = 1;
                data.t_stats.max_hit_points = 8;
            }
        });
        assert_eq!(exp_add(a, &first), 200);
        assert_eq!(exp_add(b, &second), 1);
    }
}
