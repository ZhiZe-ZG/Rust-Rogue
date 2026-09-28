//! Player movement, running, and the player/monster `THING` types.
//!
//! Ported from `src/c/move.c` to Rust, together with the shared `THING`,
//! `PLACE`, and `COORD` layouts the rest of the port relies on.
use crate::config::GameConfig;
use crate::entity::monsters::MonsterType;
use crate::draw::{
    enter_room as draw_enter_room, flat_at, leave_room as draw_leave_room, turnref as draw_turnref,
    winat,
};
use crate::entity::chase::{diag_ok, roomin};
use crate::entity::fight::{fight, swing};
use crate::entity::monsters::save;
use crate::game;
use crate::game::PLAYER;
use crate::item::armor::rust_armor;
use crate::item::pack::floor_at;
use crate::item::rings::RingType;
use crate::item::arena::new_item;
use crate::item::weapons::{fall, init_weapon};
use crate::level::new_level;
use crate::machdep::flush_type;
use crate::misc::{chg_str, spread};
use crate::rip::death;
use crate::rnd::rnd;
use crate::startup::roll;
use crate::tile::{TrapHit, TrapType};
use crate::ui::output;
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
    /// Chasing the hero (position read live from [`crate::game::PLAYER`]).
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

/// Read the actor's chase destination (a stable [`DestRef`] handle).
#[inline]
pub unsafe fn thing_dest(tp: *mut Thing) -> DestRef {
    (*thing_t(tp)).t_dest
}

/// Set the actor's chase destination.
#[inline]
pub unsafe fn set_thing_dest(tp: *mut Thing, value: DestRef) {
    (*thing_t(tp)).t_dest = value;
}

/// Whether the actor is chasing the hero.
#[inline]
pub unsafe fn is_thing_dest_hero(tp: *mut Thing) -> bool {
    matches!((*thing_t(tp)).t_dest, DestRef::Hero)
}

/// Make the actor chase the hero (clearing any stored destination).
#[inline]
pub unsafe fn set_thing_dest_hero(tp: *mut Thing) {
    (*thing_t(tp)).t_dest = DestRef::Hero;
}

/// The actor's pack as arena handles, head first.
#[inline]
pub unsafe fn thing_pack(tp: *mut Thing) -> Vec<ThingId> {
    (*thing_t(tp)).t_pack.clone()
}

/// Replace the actor's pack with `pack`.
#[inline]
pub unsafe fn set_thing_pack(tp: *mut Thing, pack: Vec<ThingId>) {
    (*thing_t(tp)).t_pack = pack;
}

/// Prepend `item` to the actor `owner`'s pack.
pub unsafe fn attach_pack(owner: *mut Thing, item: *mut Thing) {
    if let Some(id) = crate::item::arena::id_of(item) {
        (*thing_t(owner)).t_pack.insert(0, id);
    }
}

/// Unlink `item` (by handle) from the actor `owner`'s pack.
pub unsafe fn detach_pack(owner: *mut Thing, item: *mut Thing) {
    if let Some(id) = crate::item::arena::id_of(item) {
        (*thing_t(owner)).t_pack.retain(|&x| x != id);
    }
}

/// Drop every item in the actor `owner`'s pack.
pub unsafe fn free_pack(owner: *mut Thing) {
    let pack = std::mem::take(&mut (*thing_t(owner)).t_pack);
    for id in pack {
        let _ = crate::item::arena::OBJECTS.remove(id);
    }
}

/// Discard a thing: monsters are owned by the pointer-free
/// [`crate::game::MONSTER_LIST`], objects by the item arena.
///
/// Safe: it only consults the two safe containers and never dereferences the
/// handle itself.
pub fn discard(item: *mut Thing) {
    if let Some(id) = crate::game::MONSTER_LIST.find(item) {
        let _ = crate::game::MONSTER_LIST.remove(id);
        return;
    }
    let _ = crate::item::arena::OBJECTS.discard(item);
}

use crate::game::globals::{after, count, delta, door_stop, firstmove, jump, move_on, no_command, no_move, oldpos, passgo, runch, running, seenstairs, take, to_death};


/// Borrow the actor payload of `tp` (null when `tp` is an object).
#[inline]
pub unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    match &mut *tp {
        Thing::Monster { data, .. } => data as *mut ThingMonster,
        Thing::Object { .. } => std::ptr::null_mut(),
    }
}

/// Borrow the object payload of `tp` (null when `tp` is an actor).
#[inline]
pub unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    match &mut *tp {
        Thing::Object { data, .. } => data as *mut ThingObject,
        Thing::Monster { .. } => std::ptr::null_mut(),
    }
}

#[inline]
unsafe fn ring_is(ring: *mut Thing, ring_type: RingType) -> bool {
    !ring.is_null() && RingType::from_raw((*thing_o(ring)).o_which) == Some(ring_type)
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    PLAYER.has_flag(flag)
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
pub unsafe fn turn_ok(y: i32, x: i32) -> u8 {
    let flags = flat_at(y, x) as u8;
    if crate::game::is_door_at(y, x)
        || (flags & (F_REAL as u8 | F_PASS as u8)) == (F_REAL as u8 | F_PASS as u8)
    {
        true as u8
    } else {
        0
    }
}

#[inline]
unsafe fn move_stuff(next_pos: &mut IVec2, fl: u8) {
    let hero = PLAYER.pos();
    output::write_glyph_at(IVec2::new(hero.x, hero.y), (floor_at() as u8) as char);
    if (fl as u8 & F_PASS as u8) != 0 && crate::game::is_door_at(oldpos.y, oldpos.x) {
        draw_leave_room(*next_pos);
    }
    PLAYER.set_pos(*next_pos);
}

/// Applies the trap at the given map cell, returning the trap kind that fired.
///
/// The cell's trap nibble holds the trap number (0-7). If the hero is
/// levitating, no trap effect applies. Uses the C engine helpers (`msg`,
/// `roll`, `spread`, `teleport`, ...) exactly as the legacy `be_trapped` did,
/// but is callable only from Rust.
pub unsafe fn be_trapped(pos: IVec2) -> TrapType {
    let trap =
        crate::level::with_current_level(|current| current.trap_at(pos.y as usize, pos.x as usize));

    if PLAYER.has_flag(MonsterFlags::LEVIT) {
        return TrapType::Rust;
    }

    running = false as u8;
    count = 0;
    crate::level::with_current_level_mut(|current| {
        current.reveal_trap(pos.y as usize, pos.x as usize);
    });

    let mut hit = TrapHit::Miss;

    match trap {
        TrapType::Door => {
            crate::game::set_current_depth(crate::game::current_depth() + 1);
            new_level();
        }
        TrapType::Bear => {
            no_move += spread(3);
        }
        TrapType::Mystery => {}
        TrapType::Sleep => {
            no_command += spread(5);
            PLAYER.remove_flag(MonsterFlags::RUN);
        }
        TrapType::Arrow => {
            let stats = PLAYER.stats();
            if swing(stats.level - 1, stats.armor, 1) != 0 {
                PLAYER.with_stats_mut(|stats| stats.hit_points -= roll(1, 6));
                hit = if PLAYER.stats().hit_points <= 0 {
                    TrapHit::Kill
                } else {
                    TrapHit::Hit
                };
            } else {
                let arrow = new_item();
                init_weapon(arrow, ARROW);
                (*thing_o(arrow)).o_count = 1;
                (*thing_o(arrow)).o_pos = PLAYER.pos();
                fall(arrow, false as u8);
                hit = TrapHit::Miss;
            }
        }
        TrapType::Teleport => {
            teleport();
        }
        TrapType::Dart => {
            let stats = PLAYER.stats();
            if swing(stats.level + 1, stats.armor, 1) == 0 {
                hit = TrapHit::Miss;
            } else {
                PLAYER.with_stats_mut(|stats| stats.hit_points -= roll(1, 4));
                if PLAYER.stats().hit_points <= 0 {
                    hit = TrapHit::Kill;
                } else {
                    if !ring_is(PLAYER.left_ring(), RingType::SustainStrength)
                        && !ring_is(PLAYER.right_ring(), RingType::SustainStrength)
                        && save(VS_POISON) == 0
                    {
                        chg_str(-1);
                    }
                    hit = TrapHit::Hit;
                }
            }
        }
        TrapType::Rust => {
            if let Some(msg) = trap.msg(hit) {
                msg_str(&msg);
            }
            rust_armor(PLAYER.armor());
        }
    }

    // Send the message after the effect for every trap except `Rust`, which
    // prints its message before applying the effect above.
    if trap != TrapType::Rust {
        if let Some(msg) = trap.msg(hit) {
            msg_str(&msg);
        }
    }

    if hit == TrapHit::Kill {
        death(if trap == TrapType::Arrow {
            b'a' as u8
        } else {
            b'd' as u8
        });
    }

    flush_type();
    trap
}

#[inline]
unsafe fn try_passgo_turn(dy: &mut i32, dx: &mut i32) -> bool {
    let current_room = PLAYER.room();
    if passgo == 0
        || running == 0
        || current_room.is_none()
        || !crate::game::room_gone(current_room)
        || player_has(MonsterFlags::BLIND)
    {
        return false;
    }

    let hero = PLAYER.pos();
    if runch == b'h' as u8 || runch == b'l' as u8 {
        let b1 = hero.y != 1 && turn_ok(hero.y - 1, hero.x) != 0;
        let b2 = hero.y != GameConfig::SCREEN_LINES - 2 && turn_ok(hero.y + 1, hero.x) != 0;
        if !(b1 ^ b2) {
            return false;
        }
        if b1 {
            runch = b'k' as u8;
            *dy = -1;
        } else {
            runch = b'j' as u8;
            *dy = 1;
        }
        *dx = 0;
        draw_turnref();
        true
    } else if runch == b'j' as u8 || runch == b'k' as u8 {
        let b1 = hero.x != 0 && turn_ok(hero.y, hero.x - 1) != 0;
        let b2 = hero.x != GameConfig::SCREEN_COLS - 1 && turn_ok(hero.y, hero.x + 1) != 0;
        if !(b1 ^ b2) {
            return false;
        }
        if b1 {
            runch = b'h' as u8;
            *dx = -1;
        } else {
            runch = b'l' as u8;
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
pub unsafe fn do_run(ch: u8) {
    running = true as u8;
    after = false as u8;
    runch = ch;
}

/// do_move:
/// Check to see that a move is legal. If it is, handle the consequences.
pub unsafe fn do_move(dy: i32, dx: i32) {
    let mut next_pos = IVec2 { x: 0, y: 0 };
    let mut current_dy = dy;
    let mut current_dx = dx;
    let hero = PLAYER.pos();
    let mut ch: u8;
    let fl: u8;

    firstmove = false as u8;
    if no_move != 0 {
        no_move -= 1;
        msg_str("you are still stuck in the bear trap");
        return;
    }

    if player_has(MonsterFlags::HUH) && rnd(5) != 0 {
        next_pos = crate::entity::rndmove::rndmove_from(hero);
        if coord_eq(next_pos, hero) {
            after = false as u8;
            running = false as u8;
            to_death = false as u8;
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
            if try_passgo_turn(&mut current_dy, &mut current_dx) {
                next_pos.y = hero.y + current_dy;
                next_pos.x = hero.x + current_dx;
                continue;
            }
            running = false as u8;
            after = false as u8;
            return;
        }
        break;
    }

    if diag_ok(hero, next_pos) == 0 {
        after = false as u8;
        running = false as u8;
        return;
    }

    if running != 0 && coord_eq(hero, next_pos) {
        running = false as u8;
    }

    fl = flat_at(next_pos.y, next_pos.x);
    ch = winat(next_pos.y, next_pos.x);

    if (fl as u8 & F_REAL as u8) == 0 && ch == FLOOR {
        if !player_has(MonsterFlags::LEVIT) {
            crate::level::with_current_level_mut(|level| {
                level.reveal_trap(next_pos.y as usize, next_pos.x as usize);
            });
            ch = TRAP;
        }
    } else if player_has(MonsterFlags::HELD) && ch != b'F' as u8 {
        msg_str("you are being held");
        return;
    }
    match ch {
        SPACE | H_WALL | V_WALL => {
            running = false as u8;
            after = false as u8;
        }
        DOOR => {
            running = false as u8;
            if (flat_at(hero.y, hero.x) as u8 & F_PASS as u8) != 0 {
                draw_enter_room(next_pos);
            }
            move_stuff(&mut next_pos, fl);
        }
        TRAP => {
            let trap = be_trapped(next_pos);
            if trap == TrapType::Door || trap == TrapType::Teleport {
                return;
            }
            move_stuff(&mut next_pos, fl);
        }
        PASSAGE => {
            PLAYER.set_room(roomin(hero));
            move_stuff(&mut next_pos, fl);
        }
        FLOOR => {
            if (fl as u8 & F_REAL as u8) == 0 {
                be_trapped(PLAYER.pos());
            }
            move_stuff(&mut next_pos, fl);
        }
        STAIRS => {
            seenstairs = true as u8;
            running = false as u8;
            if is_upper(ch) || game::monster_here(next_pos.y, next_pos.x) {
                fight(next_pos, game::PLAYER.weapon(), false as u8);
            } else {
                take = ch;
                move_stuff(&mut next_pos, fl);
            }
        }
        _ => {
            running = false as u8;
            if is_upper(ch) || game::monster_here(next_pos.y, next_pos.x) {
                fight(next_pos, game::PLAYER.weapon(), false as u8);
            } else {
                if ch != STAIRS {
                    take = ch;
                }
                move_stuff(&mut next_pos, fl);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::arena::{allocated_count, new_item_id, new_object_id};

    /// Serialise the arena assertions; the arena and its counter are process
    /// globals, so the tests must not interleave allocation/free from several
    /// threads.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Attaching then detaching a pack handle preserves the remaining order.
    #[test]
    fn attach_detach_pack_preserves_order() {
        let _guard = serial();
        let mut owner = Thing::actor(ThingMonster::default());
        let a = new_object_id();
        let b = new_object_id();

        unsafe {
            attach_pack(&mut owner, crate::item::arena::ptr_of(a));
            attach_pack(&mut owner, crate::item::arena::ptr_of(b));
        }
        let pack = unsafe { thing_pack(&mut owner) };
        assert_eq!(pack, vec![b, a]);

        unsafe {
            detach_pack(&mut owner, crate::item::arena::ptr_of(b));
        }
        assert_eq!(unsafe { thing_pack(&mut owner) }, vec![a]);

        unsafe {
            free_pack(&mut owner);
        }
        assert!(unsafe { thing_pack(&mut owner) }.is_empty());
    }

    /// Allocating and discarding objects keeps the tracked count balanced.
    #[test]
    fn allocation_count_is_balanced() {
        let _guard = serial();
        let before = allocated_count();
        let a = new_item_id();
        let b = new_item_id();
        assert_eq!(allocated_count(), before + 2);
        let _ = crate::item::arena::OBJECTS.remove(a);
        let _ = crate::item::arena::OBJECTS.remove(b);
        assert_eq!(allocated_count(), before);
    }
}
