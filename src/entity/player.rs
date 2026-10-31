//! Player movement, running, and the player/monster `THING` types.
//!
//! Ported from `src/c/move.c` to Rust, together with the shared `THING`,
//! `PLACE`, and `COORD` layouts the rest of the port relies on.
//!
//! Raw dependency pointers must be valid and initialized for the entire call,
//! including reentrant helpers. Callers must serialize access and must not keep
//! references to those slots alive across calls that can mutate them.
use crate::config::GameConfig;
use crate::draw::{
    enter_room as draw_enter_room, leave_room as draw_leave_room, turnref as draw_turnref,
};
use crate::entity::chase::{diag_ok, roomin};
use crate::entity::fight::{fight, swing};
use crate::entity::monsters::save;
use crate::entity::monsters::MonsterType;
use crate::game::globals::{MonsterInfo, ObjInfo};
use crate::item::arena::new_item_id;
use crate::item::armor::rust_armor_id;
use crate::item::pack::floor_at;
use crate::item::rings::RingType;
use crate::item::weapons::{fall, init_weapon};
use crate::dungeon::Dungeon;
use crate::machdep::flush_type;
use crate::misc::spread;
use crate::rip::death;
use crate::rnd::Random;
use crate::tile::{TrapHit, TrapType};
use crate::ui::output::msg_str;
use crate::wizard::teleport;
use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use crate::item::arena::ThingId;

pub use crate::entity::stats::Stats;

/// Actor (monster/player) status flags — the typed replacement for the legacy
/// `t_flags` bit field of [`ThingMonster`].
///
/// The original 16-bit pattern is preserved exactly so save files stay
/// byte-compatible; callers use the named constants and bit operations below
/// instead of raw octal literals.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MonsterFlags(i16);

impl MonsterFlags {
    pub const NONE: Self = Self(0);
    pub const CANHUH: Self = Self(0o0000001);
    pub const CANSEE: Self = Self(0o0000002);
    pub const BLIND: Self = Self(0o0000004);
    /// Monster "cancel" flag; shares the bit used by [`Self::LEVIT`].
    pub const CANCELLED: Self = Self(0o0000010);
    pub const LEVIT: Self = Self(0o0000010);
    pub const FOUND: Self = Self(0o0000020);
    pub const GREED: Self = Self(0o0000040);
    pub const HASTE: Self = Self(0o0000100);
    pub const TARGET: Self = Self(0o0000200);
    pub const HELD: Self = Self(0o0000400);
    pub const HUH: Self = Self(0o0001000);
    pub const INVIS: Self = Self(0o0002000);
    pub const MEAN: Self = Self(0o0004000);
    pub const HALU: Self = Self(0o0004000);
    pub const FLY: Self = Self(0o0004000);
    pub const REGEN: Self = Self(0o0010000);
    pub const RUN: Self = Self(0o0020000);
    pub const SEEMONST: Self = Self(0o0040000);
    pub const SLOW: Self = Self(0o0100000u16 as i16);

    /// Build from a raw bit pattern (used by the save/load layer).
    #[inline]
    pub const fn from_bits(bits: i16) -> Self {
        Self(bits)
    }

    /// The raw bit pattern (used by the save/load layer).
    #[inline]
    pub const fn bits(self) -> i16 {
        self.0
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Mirrors the legacy C test `(flags & flag) != 0` (any shared bit).
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    #[inline]
    pub fn set(&mut self, other: Self, on: bool) {
        if on {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }
}

impl BitOr for MonsterFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for MonsterFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for MonsterFlags {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for MonsterFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for MonsterFlags {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl From<i16> for MonsterFlags {
    #[inline]
    fn from(bits: i16) -> Self {
        Self(bits)
    }
}

impl From<MonsterFlags> for i16 {
    #[inline]
    fn from(flags: MonsterFlags) -> Self {
        flags.0
    }
}

/// Object (item) flags — the typed replacement for the legacy `o_flags` bit
/// field of [`ThingObject`]. The 32-bit pattern is preserved exactly so save
/// files stay byte-compatible.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectFlags(i32);

impl ObjectFlags {
    pub const NONE: Self = Self(0);
    pub const CURSED: Self = Self(0o0000001);
    pub const KNOW: Self = Self(0o0000002);
    pub const MISL: Self = Self(0o0000004);
    pub const MANY: Self = Self(0o0000010);
    pub const FOUND: Self = Self(0o0000020);
    pub const PROT: Self = Self(0o0000040);

    /// Build from a raw bit pattern (used by the save/load layer).
    #[inline]
    pub const fn from_bits(bits: i32) -> Self {
        Self(bits)
    }

    /// The raw bit pattern (used by the save/load layer).
    #[inline]
    pub const fn bits(self) -> i32 {
        self.0
    }

    /// Bitwise union of two flag sets as a `const` (the `BitOr` impl is not
    /// `const`, so `static` initialisers compose flags with this instead).
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Mirrors the legacy C test `(flags & flag) != 0` (any shared bit).
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    #[inline]
    pub fn set(&mut self, other: Self, on: bool) {
        if on {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }
}

impl BitOr for ObjectFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ObjectFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ObjectFlags {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for ObjectFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for ObjectFlags {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl From<i32> for ObjectFlags {
    #[inline]
    fn from(bits: i32) -> Self {
        Self(bits)
    }
}

impl From<ObjectFlags> for i32 {
    #[inline]
    fn from(flags: ObjectFlags) -> Self {
        flags.0
    }
}

const DOOR: u8 = b'+' as u8;
const FLOOR: u8 = b'.' as u8;
const PASSAGE: u8 = b'#' as u8;
const TRAP: u8 = b'^' as u8;
const STAIRS: u8 = b'%' as u8;
const SPACE: u8 = b' ' as u8;
const H_WALL: u8 = b'-' as u8;
const V_WALL: u8 = b'|' as u8;

const F_PASS: u8 = 0x80u8 as u8;
const F_REAL: u8 = 0x10u8 as u8;

const ARROW: i32 = 3;
const VS_POISON: i32 = 0;

/// The live chase destination of an actor, replacing the legacy stored
/// `*mut IVec2`.
///
/// Every variant names its target by a stable handle or index; the actual
/// coordinate is resolved on demand. No raw pointer to a monster, item, or
/// room-gold slot is ever stored on a [`ThingMonster`].
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum DestRef {
    /// No chase target.
    #[default]
    None,
    /// Chasing the hero (position read live from the supplied dungeon).
    Hero,
    /// Chasing the live monster with this id.
    Monster(crate::game::MonsterId),
    /// Chasing the floor object with this id.
    Object(crate::item::arena::ThingId),
    /// Heading for the gold stash of this room index.
    RoomGold(usize),
}

impl DestRef {
    /// Whether this destination is the hero.
    #[inline]
    pub const fn is_hero(self) -> bool {
        matches!(self, DestRef::Hero)
    }
}

/// Monster/player (actor) data for a [`Thing`], using native Rust types.
#[derive(Clone)]
pub struct ThingMonster {
    pub t_pos: IVec2,
    pub t_turn: bool,
    /// The monster's real identity (its kind).
    pub t_type: Option<MonsterType>,
    /// The glyph actually drawn (usually `t_type.glyph()`, but hidden as an
    /// item for Xerocs and randomized under hallucination).
    pub t_disguise: u8,
    pub t_oldch: u8,
    /// Chase destination, stored as a stable handle (see [`DestRef`]).
    pub t_dest: DestRef,
    pub t_flags: MonsterFlags,
    pub t_stats: Stats,
    pub t_room: Option<usize>,
    /// Items carried in this actor's pack, as arena handles (head first).
    pub t_pack: Vec<ThingId>,
    pub t_reserved: i32,
}

/// Object (item) data for a [`Thing`], using native Rust types. The `o_text`
/// and `o_label` string fields are owned Rust `String`s rather than C pointers.
///
/// This is a pure value type (no pointers), so it derives `Serialize` /
/// `Deserialize` for the RON save format directly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThingObject {
    /// The item's kind (typed replacement for the legacy ASCII `o_type`).
    pub o_type: crate::item::item_type::ItemType,
    pub o_pos: IVec2,
    pub o_text: Option<String>,
    pub o_launch: i32,
    pub o_packch: u8,
    pub o_damage: [u8; 8],
    pub o_hurldmg: [u8; 8],
    pub o_count: i32,
    pub o_which: i32,
    pub o_hplus: i32,
    pub o_dplus: i32,
    pub o_arm: i32,
    pub o_flags: ObjectFlags,
    pub o_group: i32,
    pub o_label: Option<String>,
}

/// A game thing: either an actor (monster/player) or an object (item). This is
/// a pure Rust enum (`union`/C ABI and the intrusive list header have been
/// removed). It is `Clone` but not `Copy` because object string fields and the
/// actor pack are owned values.
#[derive(Clone)]
pub enum Thing {
    Monster { data: ThingMonster },
    Object { data: ThingObject },
}

// `Thing` is automatically `Send + Sync` because every field it holds
// (`ThingMonster`/`ThingObject` and their owned `String`/`Vec` members) is
// itself `Send + Sync`. It is stored inside `Mutex`/`RwLock`, which require
// those bounds, so no manual `unsafe impl` is needed.
impl Thing {
    /// Build an actor thing.
    pub const fn actor(data: ThingMonster) -> Self {
        Thing::Monster { data }
    }

    /// Build an object thing.
    pub const fn object(data: ThingObject) -> Self {
        Thing::Object { data }
    }
}

impl Default for ThingMonster {
    fn default() -> Self {
        ThingMonster {
            t_pos: IVec2 { x: 0, y: 0 },
            t_turn: false,
            t_type: None,
            t_disguise: 0,
            t_oldch: 0,
            t_dest: DestRef::None,
            t_flags: MonsterFlags::NONE,
            t_stats: Stats::default(),
            t_room: None,
            t_pack: Vec::new(),
            t_reserved: 0,
        }
    }
}

impl Default for ThingObject {
    fn default() -> Self {
        ThingObject {
            o_type: crate::item::item_type::ItemType::None,
            o_pos: IVec2 { x: 0, y: 0 },
            o_text: None,
            o_launch: 0,
            o_packch: 0,
            o_damage: [0; 8],
            o_hurldmg: [0; 8],
            o_count: 0,
            o_which: 0,
            o_hplus: 0,
            o_dplus: 0,
            o_arm: 0,
            o_flags: ObjectFlags::NONE,
            o_group: 0,
            o_label: None,
        }
    }
}

// ─── Pointer-free actor (monster) accessors ─────────────────────────────────

/// Read the actor's chase destination (a stable [`DestRef`] handle).
#[inline]
pub fn monster_dest(id: crate::game::MonsterId, dungeon: &Dungeon) -> DestRef {
    dungeon.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => data.t_dest,
            Thing::Object { .. } => DestRef::None,
        })
        .unwrap_or(DestRef::None)
}

/// Set the actor's chase destination.
#[inline]
pub fn set_monster_dest(id: crate::game::MonsterId, value: DestRef, dungeon: &Dungeon) {
    dungeon.monster_list.with_mut(id, |t| {
        if let Thing::Monster { data } = t {
            data.t_dest = value;
        }
    });
}

/// Whether the actor is chasing the hero.
#[inline]
pub fn is_monster_dest_hero(id: crate::game::MonsterId, dungeon: &Dungeon) -> bool {
    matches!(monster_dest(id, dungeon), DestRef::Hero)
}

/// Make the actor chase the hero (clearing any stored destination).
#[inline]
pub fn set_monster_dest_hero(id: crate::game::MonsterId, dungeon: &Dungeon) {
    set_monster_dest(id, DestRef::Hero, dungeon);
}

/// The actor's pack as arena handles, head first.
#[inline]
pub fn monster_pack(id: crate::game::MonsterId, dungeon: &Dungeon) -> Vec<ThingId> {
    dungeon.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => data.t_pack.clone(),
            Thing::Object { .. } => Vec::new(),
        })
        .unwrap_or_default()
}

/// Replace the actor's pack with `pack`.
#[inline]
pub fn set_monster_pack(id: crate::game::MonsterId, pack: Vec<ThingId>, dungeon: &Dungeon) {
    dungeon.monster_list.with_mut(id, |t| {
        if let Thing::Monster { data } = t {
            data.t_pack = pack;
        }
    });
}

/// Prepend the object `item` to the actor `owner`'s pack.
pub fn attach_pack_id(owner: crate::game::MonsterId, item: ThingId, dungeon: &Dungeon) {
    dungeon.monster_list.with_mut(owner, |t| {
        if let Thing::Monster { data } = t {
            data.t_pack.insert(0, item);
        }
    });
}

/// Unlink the object `item` from the actor `owner`'s pack.
pub fn detach_pack_id(owner: crate::game::MonsterId, item: ThingId, dungeon: &Dungeon) {
    dungeon.monster_list.with_mut(owner, |t| {
        if let Thing::Monster { data } = t {
            data.t_pack.retain(|&x| x != item);
        }
    });
}

/// Drop every item in the actor `owner`'s pack.
pub fn free_pack_id(owner: crate::game::MonsterId, dungeon: &Dungeon) {
    let pack = dungeon.monster_list
        .with_mut(owner, |t| match t {
            Thing::Monster { data } => std::mem::take(&mut data.t_pack),
            Thing::Object { .. } => Vec::new(),
        })
        .unwrap_or_default();
    for id in pack {
        let _ = dungeon.objects.remove(id);
    }
}

/// Remove the monster `id` from the live monster list.
pub fn discard_monster(id: crate::game::MonsterId, dungeon: &Dungeon) {
    let _ = dungeon.monster_list.remove(id);
}

#[inline]
fn player_has(flag: MonsterFlags, dungeon: &Dungeon) -> bool {
    dungeon.player.has_flag(flag)
}

/// Rendering glyph from the supplied model, without a monster overlay.
pub(crate) fn cell_glyph(y: i32, x: i32, dungeon: &Dungeon) -> u8 {
    let items = dungeon.with_level(|level| level.items.clone());
    for id in items {
        if let Some(glyph) = dungeon.objects.with_object(id, |object| {
            (object.o_pos == IVec2::new(x, y)).then(|| crate::draw::item_glyph(object.o_type) as u8)
        }).flatten() {
            return glyph;
        }
    }
    dungeon.with_level(|level| {
        use crate::tile::Tile;
        let pos = IVec2::new(x, y);
        let idx = y as usize * GameConfig::LEVEL_WIDTH + x as usize;
        let wall = || {
            let is_wall = |pos| matches!(level.map.get(pos), Some(Tile::Wall | Tile::HiddenDoor | Tile::Door));
            if y > 0 && is_wall(pos - IVec2::Y) && is_wall(pos + IVec2::Y) {
                V_WALL
            } else {
                H_WALL
            }
        };
        match level.map.get(pos).unwrap_or(Tile::Empty) {
            Tile::Empty => SPACE,
            Tile::Floor => FLOOR,
            Tile::Wall => wall(),
            Tile::HiddenDoor if !level.flags.real[idx] => wall(),
            Tile::HiddenDoor | Tile::Door => DOOR,
            Tile::Passage => PASSAGE,
            Tile::Stairs => STAIRS,
            Tile::Trap(_) if level.flags.seen[idx] => TRAP,
            Tile::Trap(_) => FLOOR,
        }
    })
}

fn winat(y: i32, x: i32, dungeon: &Dungeon) -> u8 {
    dungeon.monster_map.at(y as usize, x as usize)
        .and_then(|id| dungeon.monster_list.with(id, |thing| match thing {
            Thing::Monster { data } => Some(data.t_disguise),
            Thing::Object { .. } => None,
        }).flatten())
        .unwrap_or_else(|| cell_glyph(y, x, dungeon))
}

fn flat_at(y: i32, x: i32, dungeon: &Dungeon) -> u8 {
    dungeon.with_level(|level| {
        let idx = y as usize * GameConfig::LEVEL_WIDTH + x as usize;
        let mut flags = level.flags.passnum[idx] & crate::draw::F_PNUM;
        if level.flags.passage[idx] { flags |= F_PASS; }
        if level.flags.seen[idx] { flags |= crate::draw::F_SEEN; }
        if level.flags.real[idx] { flags |= F_REAL; }
        flags | (level.tile_at(y as usize, x as usize).trap() as u8 & crate::draw::F_TMASK)
    })
}

/// Update the supplied hero's experience level and hit points.
///
/// `e_levels` must point to a valid, zero-terminated experience table.
pub unsafe fn check_level(random: &mut Random, dungeon: &Dungeon, e_levels: *const [i32; 21]) {
    let experience = dungeon.player.stats().experience;
    let mut level = 0;
    while level < 21 && (*e_levels)[level] != 0 && (*e_levels)[level] <= experience {
        level += 1;
    }
    let level = level as i32 + 1;
    let old_level = dungeon.player.level();
    dungeon.player.with_stats_mut(|stats| stats.level = level);
    if level > old_level {
        let add = random.roll(level - old_level, 10);
        dungeon.player.with_stats_mut(|stats| {
            stats.max_hit_points += add;
            stats.hit_points += add;
        });
        msg_str(&format!("welcome to level {level}"));
    }
}

/// Change the supplied hero's strength and update its unmodified maximum.
///
/// The maximum's write lock recovers poison and is acquired only after other
/// gameplay calls, so no guard spans a callback or reentrant helper.
pub unsafe fn chg_str(amt: i32, dungeon: &Dungeon, max_stats: &std::sync::RwLock<Stats>) {
    if amt == 0 { return; }
    let strength = (dungeon.player.stats().strength as i32 + amt).clamp(3, 31) as u32;
    dungeon.player.with_stats_mut(|stats| stats.strength = strength);
    let mut unmodified = strength;
    for hand in 0..2 {
        if let Some(bonus) = dungeon.player.equipment().ring_arm(hand) {
            unmodified = (unmodified as i32 - bonus).max(3) as u32;
        }
    }
    let mut maximum = max_stats.write().unwrap_or_else(|poison| poison.into_inner());
    if unmodified > maximum.strength {
        maximum.strength = unmodified;
    }
}

#[inline]
unsafe fn coord_eq(a: IVec2, b: IVec2) -> bool {
    a.x == b.x && a.y == b.y
}

#[inline]
unsafe fn is_upper(ch: u8) -> bool {
    (ch as u8).is_ascii_uppercase()
}

/// turn_ok:
/// Decide whether it is legal to turn onto the given space.
pub unsafe fn turn_ok(y: i32, x: i32, dungeon: &Dungeon) -> u8 {
    let flags = flat_at(y, x, dungeon) as u8;
    if dungeon.with_level(|level| level.is_door_at(y as usize, x as usize))
        || (flags & (F_REAL as u8 | F_PASS as u8)) == (F_REAL as u8 | F_PASS as u8)
    {
        true as u8
    } else {
        0
    }
}

#[inline]
unsafe fn move_stuff(next_pos: &mut IVec2, fl: u8, dungeon: &Dungeon, random: &mut Random, oldpos: *const IVec2) {
    let hero = dungeon.player.pos();
    crate::draw::write_cell_glyph(hero, floor_at(dungeon) as char);
    if (fl as u8 & F_PASS as u8) != 0
        && dungeon.with_level(|level| level.is_door_at((*oldpos).y as usize, (*oldpos).x as usize))
    {
        draw_leave_room(*next_pos, random);
    }
    dungeon.player.set_pos(*next_pos);
}

/// Applies the trap at the given map cell, returning the trap kind that fired.
///
/// The cell's trap nibble holds the trap number (0-7). If the hero is
/// levitating, no trap effect applies. Uses the C engine helpers (`msg`,
/// `roll`, `spread`, `teleport`, ...) exactly as the legacy `be_trapped` did,
/// but is callable only from Rust.
pub unsafe fn be_trapped(
    pos: IVec2,
    dungeon: &Dungeon,
    random: &mut Random,
    running: *mut u8,
    count: *mut i32,
    no_move: *mut i32,
    no_command: *mut i32,
    group: *mut i32,
    has_hit: *mut u8,
    weap_info: *const [ObjInfo; 10],
    to_death: *const u8,
    terse: *const u8,
    max_stats: &std::sync::RwLock<Stats>,
) -> TrapType {
    let trap =
        dungeon.with_level(|current| current.trap_at(pos.y as usize, pos.x as usize));

    if dungeon.player.has_flag(MonsterFlags::LEVIT) {
        return TrapType::Rust;
    }

    *running = false as u8;
    *count = 0;
    dungeon.with_level_mut(|current| {
        current.reveal_trap(pos.y as usize, pos.x as usize);
    });

    let mut hit = TrapHit::Miss;

    match trap {
        TrapType::Door => {
            dungeon.set_current_depth(dungeon.current_depth() + 1);
            dungeon.new_level();
        }
        TrapType::Bear => {
            *no_move += spread(3, random);
        }
        TrapType::Mystery => {}
        TrapType::Sleep => {
            *no_command += spread(5, random);
            dungeon.player.remove_flag(MonsterFlags::RUN);
        }
        TrapType::Arrow => {
            let stats = dungeon.player.stats();
            let hit_roll = swing(stats.level - 1, stats.armor, 1, random);
            if hit_roll != 0 {
                let damage = random.roll(1, 6);
                dungeon.player.with_stats_mut(|stats| {
                    stats.hit_points -= damage;
                });
                hit = if dungeon.player.stats().hit_points <= 0 {
                    TrapHit::Kill
                } else {
                    TrapHit::Hit
                };
            } else {
                let arrow = new_item_id(&dungeon.objects);
                init_weapon(
                    arrow,
                    ARROW,
                    random,
                    dungeon,
                    group,
                );
                dungeon.objects.with_object_mut(arrow, |o| {
                    o.o_count = 1;
                    o.o_pos = dungeon.player.pos();
                });
                let weap_info = (*weap_info).clone();
                fall(
                    arrow,
                    false,
                    has_hit,
                    random,
                    dungeon,
                    &weap_info,
                );
                hit = TrapHit::Miss;
            }
        }
        TrapType::Teleport => {
            teleport(random);
        }
        TrapType::Dart => {
            let stats = dungeon.player.stats();
            let hit_roll = swing(stats.level + 1, stats.armor, 1, random);
            if hit_roll == 0 {
                hit = TrapHit::Miss;
            } else {
                let damage = random.roll(1, 4);
                dungeon.player.with_stats_mut(|stats| {
                    stats.hit_points -= damage;
                });
                if dungeon.player.stats().hit_points <= 0 {
                    hit = TrapHit::Kill;
                } else {
                    if !dungeon.player.wearing_ring(RingType::SustainStrength) && save(VS_POISON, random, dungeon) == 0 {
                        chg_str(-1, dungeon, max_stats);
                    }
                    hit = TrapHit::Hit;
                }
            }
        }
        TrapType::Rust => {
            let message = trap.msg(hit, random);
            if let Some(msg) = message {
                msg_str(&msg);
            }
            rust_armor_id(
                dungeon.player.equipment().armor_id(),
                dungeon,
                *to_death,
                *terse,
            );
        }
    }

    // Send the message after the effect for every trap except `Rust`, which
    // prints its message before applying the effect above.
    if trap != TrapType::Rust {
        let message = trap.msg(hit, random);
        if let Some(msg) = message {
            msg_str(&msg);
        }
    }

    if hit == TrapHit::Kill {
        death(if trap == TrapType::Arrow {
            b'a' as u8
        } else {
            b'd' as u8
        });
        if crate::startup::exit_requested() {
            return trap;
        }
    }

    flush_type();
    trap
}

#[inline]
unsafe fn try_passgo_turn(
    dy: &mut i32, dx: &mut i32, dungeon: &Dungeon,
    passgo: *const u8, running: *const u8, runch: *mut crate::direction::Direction,
) -> bool {
    let current_room = dungeon.player.room();
    if *passgo == 0
        || *running == 0
        || current_room.is_none()
        || !dungeon.with_level(|level| level.room_gone(current_room))
        || player_has(MonsterFlags::BLIND, dungeon)
    {
        return false;
    }

    let hero = dungeon.player.pos();
    if *runch == crate::direction::Direction::West || *runch == crate::direction::Direction::East {
        let b1 = hero.y != 1 && turn_ok(hero.y - 1, hero.x, dungeon) != 0;
        let b2 = hero.y != GameConfig::SCREEN_LINES - 2 && turn_ok(hero.y + 1, hero.x, dungeon) != 0;
        if !(b1 ^ b2) {
            return false;
        }
        if b1 {
            *runch = crate::direction::Direction::North;
            *dy = -1;
        } else {
            *runch = crate::direction::Direction::South;
            *dy = 1;
        }
        *dx = 0;
        draw_turnref();
        true
    } else if *runch == crate::direction::Direction::South
        || *runch == crate::direction::Direction::North
    {
        let b1 = hero.x != 0 && turn_ok(hero.y, hero.x - 1, dungeon) != 0;
        let b2 = hero.x != GameConfig::SCREEN_COLS - 1 && turn_ok(hero.y, hero.x + 1, dungeon) != 0;
        if !(b1 ^ b2) {
            return false;
        }
        if b1 {
            *runch = crate::direction::Direction::West;
            *dx = -1;
        } else {
            *runch = crate::direction::Direction::East;
            *dx = 1;
        }
        *dy = 0;
        draw_turnref();
        true
    } else {
        false
    }
}

/// Global "next hero position" used by the save/load subsystem (state.c).
///
/// Re-exported from [`crate::game::globals`], the single owner of process-wide
/// game state.
pub use crate::game::globals::nh;

/// do_run:
/// Start the hero running in the chosen direction.
pub unsafe fn do_run(
    direction: crate::direction::Direction,
    running: *mut u8,
    after: *mut u8,
    runch: *mut crate::direction::Direction,
) {
    *running = true as u8;
    *after = false as u8;
    *runch = direction;
}

/// do_move:
/// Check to see that a move is legal. If it is, handle the consequences.
pub unsafe fn do_move(
    dy: i32,
    dx: i32,
    dungeon: &Dungeon,
    random: &mut Random,
    firstmove: *mut u8,
    no_move: *mut i32,
    after: *mut u8,
    running: *mut u8,
    to_death: *mut u8,
    passgo: *const u8,
    runch: *mut crate::direction::Direction,
    oldpos: *const IVec2,
    take: *mut u8,
    count: *mut i32,
    no_command: *mut i32,
    group: *mut i32,
    has_hit: *mut u8,
    weap_info: *const [ObjInfo; 10],
    terse: *const u8,
    max_stats: &std::sync::RwLock<Stats>,
    quiet: &std::sync::atomic::AtomicI32,
    kamikaze: *mut u8,
    vf_hit: *mut i32,
    fight_flush: *const u8,
    monsters: *mut [MonsterInfo; 26],
    e_levels: *const [i32; 21],
) {
    let mut next_pos = IVec2 { x: 0, y: 0 };
    let mut current_dy = dy;
    let mut current_dx = dx;
    let hero = dungeon.player.pos();
    let mut ch: u8;
    let fl: u8;

    *firstmove = false as u8;
    if *no_move != 0 {
        *no_move -= 1;
        msg_str("you are still stuck in the bear trap");
        return;
    }

    if player_has(MonsterFlags::HUH, dungeon) && random.rnd(5) != 0
    {
        next_pos = crate::entity::rndmove::rndmove_from(hero, random, dungeon);
        if coord_eq(next_pos, hero) {
            *after = false as u8;
            *running = false as u8;
            *to_death = false as u8;
            return;
        }
    } else {
        next_pos.y = hero.y + current_dy;
        next_pos.x = hero.x + current_dx;
    }

    loop {
        if next_pos.x < 0
            || next_pos.x >= GameConfig::SCREEN_COLS
            || next_pos.y <= 0
            || next_pos.y >= GameConfig::SCREEN_LINES - 1
        {
            if try_passgo_turn(&mut current_dy, &mut current_dx, dungeon, passgo, running, runch) {
                next_pos.y = hero.y + current_dy;
                next_pos.x = hero.x + current_dx;
                continue;
            }
            *running = false as u8;
            *after = false as u8;
            return;
        }
        break;
    }

    if diag_ok(hero, next_pos, dungeon) == 0 {
        *after = false as u8;
        *running = false as u8;
        return;
    }

    if *running != 0 && coord_eq(hero, next_pos) {
        *running = false as u8;
    }

    fl = flat_at(next_pos.y, next_pos.x, dungeon);
    ch = winat(next_pos.y, next_pos.x, dungeon);

    if (fl as u8 & F_REAL as u8) == 0 && ch == FLOOR {
        if !player_has(MonsterFlags::LEVIT, dungeon) {
            dungeon.with_level_mut(|level| {
                level.reveal_trap(next_pos.y as usize, next_pos.x as usize);
            });
            ch = TRAP;
        }
    } else if player_has(MonsterFlags::HELD, dungeon) && ch != b'F' as u8 {
        msg_str("you are being held");
        return;
    }
    match ch {
        SPACE | H_WALL | V_WALL => {
            *running = false as u8;
            *after = false as u8;
        }
        DOOR => {
            *running = false as u8;
            if (flat_at(hero.y, hero.x, dungeon) as u8 & F_PASS as u8) != 0 {
                draw_enter_room(next_pos, random);
            }
            move_stuff(&mut next_pos, fl, dungeon, random, oldpos);
        }
        TRAP => {
            let trap = be_trapped(next_pos, dungeon, random, running, count, no_move,
                no_command, group, has_hit, weap_info, to_death, terse, max_stats);
            if trap == TrapType::Door || trap == TrapType::Teleport {
                return;
            }
            move_stuff(&mut next_pos, fl, dungeon, random, oldpos);
        }
        PASSAGE => {
            dungeon.player.set_room(roomin(hero, dungeon));
            move_stuff(&mut next_pos, fl, dungeon, random, oldpos);
        }
        FLOOR => {
            if (fl as u8 & F_REAL as u8) == 0 {
                be_trapped(dungeon.player.pos(), dungeon, random, running, count, no_move,
                    no_command, group, has_hit, weap_info, to_death, terse, max_stats);
            }
            move_stuff(&mut next_pos, fl, dungeon, random, oldpos);
        }
        STAIRS => {
            dungeon.set_seen_stairs(true);
            *running = false as u8;
            if is_upper(ch) || dungeon.monster_map.at(next_pos.y as usize, next_pos.x as usize).is_some() {
                fight(next_pos, dungeon.player.weapon_id(), false as u8, random, dungeon,
                    count, quiet, has_hit, to_death, kamikaze, vf_hit, fight_flush,
                    monsters, weap_info, e_levels);
            } else {
                *take = ch;
                move_stuff(&mut next_pos, fl, dungeon, random, oldpos);
            }
        }
        _ => {
            *running = false as u8;
            if is_upper(ch) || dungeon.monster_map.at(next_pos.y as usize, next_pos.x as usize).is_some() {
                fight(next_pos, dungeon.player.weapon_id(), false as u8, random, dungeon,
                    count, quiet, has_hit, to_death, kamikaze, vf_hit, fight_flush,
                    monsters, weap_info, e_levels);
            } else {
                if ch != STAIRS {
                    *take = ch;
                }
                move_stuff(&mut next_pos, fl, dungeon, random, oldpos);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::arena::{allocated_count, new_item_id, new_object_id};

    /// Retained test serialization; all model state below is dungeon-local.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Attaching then detaching a pack handle preserves the remaining order.
    #[test]
    fn attach_detach_pack_preserves_order() {
        let _guard = serial();
        let dungeon = crate::dungeon::Dungeon::empty();
        let owner = dungeon.monster_list.spawn_actor();
        let a = new_object_id(&dungeon.objects);
        let b = new_object_id(&dungeon.objects);

        attach_pack_id(owner, a, &dungeon);
        attach_pack_id(owner, b, &dungeon);
        assert_eq!(monster_pack(owner, &dungeon), vec![b, a]);

        detach_pack_id(owner, b, &dungeon);
        assert_eq!(monster_pack(owner, &dungeon), vec![a]);

        free_pack_id(owner, &dungeon);
        assert!(monster_pack(owner, &dungeon).is_empty());
        assert!(!dungeon.objects.contains(a));
        assert!(dungeon.objects.contains(b));
        let _ = dungeon.monster_list.remove(owner);
    }

    #[test]
    fn destinations_and_packs_are_dungeon_local() {
        let first = Dungeon::empty();
        let second = Dungeon::empty();
        let a = first.monster_list.spawn_actor();
        let b = second.monster_list.spawn_actor();
        let item = new_object_id(&first.objects);

        set_monster_dest_hero(a, &first);
        attach_pack_id(a, item, &first);
        assert!(is_monster_dest_hero(a, &first));
        assert_eq!(monster_dest(b, &second), DestRef::None);
        assert!(monster_pack(b, &second).is_empty());

        set_monster_dest(b, DestRef::RoomGold(2), &second);
        set_monster_pack(b, Vec::new(), &second);
        free_pack_id(a, &first);
        discard_monster(a, &first);
        assert_eq!(monster_dest(b, &second), DestRef::RoomGold(2));
        assert_eq!(monster_pack(b, &second), Vec::<ThingId>::new());
        assert!(first.monster_list.ids().is_empty());
        assert_eq!(second.monster_list.ids(), vec![b]);
    }

    #[test]
    fn running_and_strength_use_supplied_state() {
        let first = Dungeon::empty();
        let second = Dungeon::empty();
        first.player.with_stats_mut(|stats| stats.strength = 10);
        second.player.with_stats_mut(|stats| stats.strength = 19);
        let maximum = std::sync::RwLock::new(Stats::default());
        let mut running = 0;
        let mut after = 1;
        let mut direction = crate::direction::Direction::West;
        unsafe {
            do_run(crate::direction::Direction::North, &raw mut running, &raw mut after, &raw mut direction);
            chg_str(30, &first, &maximum);
        }
        assert_eq!(running, 1);
        assert_eq!(after, 0);
        assert!(direction == crate::direction::Direction::North);
        assert_eq!(first.player.stats().strength, 31);
        assert_eq!(maximum.read().unwrap().strength, 31);
        assert_eq!(second.player.stats().strength, 19);
        unsafe { chg_str(-50, &first, &maximum); }
        assert_eq!(first.player.stats().strength, 3);
        assert_eq!(maximum.read().unwrap().strength, 31);
    }

    #[test]
    fn strength_update_recovers_poisoned_maximum() {
        let dungeon = Dungeon::empty();
        dungeon.player.with_stats_mut(|stats| stats.strength = 10);
        let maximum = std::sync::RwLock::new(Stats::default());
        let result = std::panic::catch_unwind(|| {
            let mut stats = maximum.write().unwrap();
            stats.strength = 12;
            panic!("poison maximum stats for recovery test");
        });
        assert!(result.is_err());
        assert!(maximum.is_poisoned());

        unsafe { chg_str(5, &dungeon, &maximum); }
        assert_eq!(dungeon.player.stats().strength, 15);
        assert_eq!(maximum.read().unwrap_or_else(|poison| poison.into_inner()).strength, 15);

        unsafe { chg_str(-5, &dungeon, &maximum); }
        assert_eq!(dungeon.player.stats().strength, 10);
        assert_eq!(maximum.read().unwrap_or_else(|poison| poison.into_inner()).strength, 15);
    }

    #[test]
    fn zero_strength_change_does_not_lock_maximum() {
        let dungeon = Dungeon::empty();
        let maximum = std::sync::RwLock::new(Stats::default());
        let _guard = maximum.write().unwrap();
        unsafe { chg_str(0, &dungeon, &maximum); }
    }

    /// Allocating and discarding objects keeps the tracked count balanced.
    #[test]
    fn allocation_count_is_balanced() {
        let _guard = serial();
        let dungeon = crate::dungeon::Dungeon::empty();
        let before = allocated_count(&dungeon.objects);
        let a = new_item_id(&dungeon.objects);
        let b = new_item_id(&dungeon.objects);
        assert_eq!(allocated_count(&dungeon.objects), before + 2);
        let _ = dungeon.objects.remove(a);
        let _ = dungeon.objects.remove(b);
        assert_eq!(allocated_count(&dungeon.objects), before);
    }
}
