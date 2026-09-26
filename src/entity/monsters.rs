//! Monster creation and behaviour.
//!
//! Ported from `src/c/monsters.c` to Rust.
use crate::config::GameConfig;
use crate::daemon::{fuse, lengthen, Daemon};
use crate::entity::chase::{dist, roomin, runto};
use crate::entity::fight::set_mname;
use crate::entity::player::{MonsterFlags, Thing, ThingMonster, ThingObject};
use crate::game::PLAYER;
use crate::item::rings::RingType;
use crate::entity::player::attach_pack;
use crate::game::new_actor;
use crate::item::things::new_thing;
use crate::level::find_floor;
use crate::misc::{rnd_thing, spread};
use crate::rnd::rnd;
use crate::startup::roll;
use crate::ui::output;
use crate::ui::output::{addmsg_str, msg_str};
use crate::ui::runtime;
use glam::IVec2;

use crate::globals::monsters;

const LAMPDIST: i32 = 3;
const HUHDURATION: i32 = 20;
const AFTER: i32 = 2;
const VS_MAGIC: i32 = 0o03;

pub use crate::globals::CMonster;

/// The identity of a monster kind — the typed replacement for the legacy
/// `t_type` ASCII letter (`'A'`..=`'Z'`).
///
/// The variant order matches the `monsters` stat table in
/// [`crate::globals`] (`Aquator` = index 0 … `Zombie` = index 25), so
/// [`MonsterType::index`] is a direct table index and [`MonsterType::glyph`]
/// reproduces the original `'A'..='Z'` byte used by the save format and by
/// [`crate::globals::monsters`].
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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
    pub fn name(self) -> &'static str {
        // SAFETY: the `monsters` table is initialised before any monster exists.
        unsafe { crate::globals::monsters[self.index()].m_name }
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

use crate::globals::{max_level, wizard};


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

/// Picks an appropriate monster kind for the current depth.
pub unsafe fn randmonster(wander: bool) -> MonsterType {
    let level = crate::game::current_depth();
    loop {
        let mut d = level + (rnd(10) - 6);
        if d < 0 {
            d = rnd(5);
        }
        if d > 25 {
            d = rnd(5) + 21;
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

/// Initializes a freshly allocated monster thing and places it on the map.
pub unsafe fn new_monster(tp: *mut Thing, monster_type: MonsterType, cp: *mut IVec2) {
    let level = crate::game::current_depth();
    let mut lev_add = level - GameConfig::AMULET_LEVEL;
    if lev_add < 0 {
        lev_add = 0;
    }

    // `tp` was already allocated into `MLIST` by `new_actor`; no attach needed.

    (*thing_t(tp)).t_type = Some(monster_type);
    (*thing_t(tp)).t_disguise = monster_type.glyph();
    (*thing_t(tp)).t_pos = *cp;

    (*thing_t(tp)).t_oldch = crate::draw::cell_glyph((*cp).y, (*cp).x) as u8;
    (*thing_t(tp)).t_room = roomin(cp);
    // Record the monster in the per-cell occupancy map.
    crate::game::set_monster((*cp).y, (*cp).x, tp);

    let mp = &monsters[monster_type.index()];
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
    if monster_type == MonsterType::Xeroc {
        (*thing_t(tp)).t_disguise = rnd_thing() as u8;
    }
}

/// Computes bonus experience from a monster's level and max HP.
pub unsafe fn exp_add(tp: *mut Thing) -> i32 {
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
pub unsafe fn wanderer() {
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
            output::write_glyph(crate::draw::monster_type_glyph(tp));
        } else {
            output::write_glyph(crate::draw::hallucination_glyph());
        }
        output::set_standout(false);
    }

    runto(&mut (*thing_t(tp)).t_pos);

    if wizard != 0 {
        msg_str(&format!(
            "started a wandering {}",
            (*thing_t(tp)).t_type.map_or("", |m| m.name())
        ));
    }
}

/// Wakes and updates an adjacent monster's pursuit behavior and special gaze logic.
pub unsafe fn wake_monster(y: i32, x: i32) -> *mut Thing {
    let tp = crate::game::monster_at(y, x);
    if tp.is_null() {
        runtime::shutdown();
        std::process::abort();
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

    if ch == Some(MonsterType::Medusa)
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
                    lengthen(Daemon::Unconfuse, spread(HUHDURATION));
                } else {
                    fuse(Daemon::Unconfuse, 0, spread(HUHDURATION), AFTER);
                }
                crate::game::PLAYER.add_flag(MonsterFlags::HUH);
                let mname_str = set_mname(tp);
                addmsg_str(&mname_str);
                if mname_str != "it" {
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
pub unsafe fn give_pack(tp: *mut Thing) {
    if crate::game::current_depth() >= max_level
        && rnd(100) < monsters[(*thing_t(tp)).t_type.map_or(0, |m| m.index())].m_carry
    {
        attach_pack(tp, new_thing());
    }
}

/// Rolls a saving throw for any creature against an effect category.
pub unsafe fn save_throw(which: i32, tp: *mut Thing) -> i32 {
    save_throw_for_level(which, (*thing_t(tp)).t_stats.level)
}

/// Roll a saving throw using an explicit caster level (used for the hero, whose
/// `Thing` is no longer reachable as a raw pointer).
#[inline]
fn save_throw_for_level(which: i32, level: i32) -> i32 {
    let need = 14 + which - level / 2;
    if unsafe { roll(1, 20) } >= need {
        1
    } else {
        0
    }
}

/// Rolls the hero's saving throw, applying ring of protection magic adjustment.
pub unsafe fn save(which: i32) -> i32 {
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
