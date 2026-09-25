//! Player movement, running, and the player/monster `THING` types.
//!
//! Ported from `src/c/move.c` to Rust, together with the shared `THING`,
//! `PLACE`, and `COORD` layouts the rest of the port relies on.
use crate::config::GameConfig;
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
use crate::item::thing_list::new_item;
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
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use std::os::raw::{c_char, c_int, c_uchar};
use std::ptr::NonNull;

pub use crate::entity::stats::Stats;

/// Actor (monster/player) status flags — the typed replacement for the legacy
/// `t_flags` bit field of [`CThingMonster`].
///
/// The original 16-bit pattern is preserved exactly so save files stay
/// byte-compatible; callers use the named constants and bit operations below
/// instead of raw octal literals.
#[derive(Copy, Clone, PartialEq, Eq, Default)]
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
/// field of [`CThingObject`]. The 32-bit pattern is preserved exactly so save
/// files stay byte-compatible.
#[derive(Copy, Clone, PartialEq, Eq, Default)]
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

const DOOR: c_char = b'+' as c_char;
const FLOOR: c_char = b'.' as c_char;
const PASSAGE: c_char = b'#' as c_char;
const TRAP: c_char = b'^' as c_char;
const STAIRS: c_char = b'%' as c_char;
const SPACE: c_char = b' ' as c_char;
const H_WALL: c_char = b'-' as c_char;
const V_WALL: c_char = b'|' as c_char;

const F_PASS: c_char = 0x80u8 as c_char;
const F_REAL: c_char = 0x10u8 as c_char;

const ARROW: c_int = 3;
const VS_POISON: c_int = 0;

/// Monster/player (actor) data for a [`CThing`], using native Rust types.
#[derive(Copy, Clone)]
pub struct ThingMonster {
    pub t_pos: IVec2,
    pub t_turn: bool,
    pub t_type: u8,
    pub t_disguise: u8,
    pub t_oldch: u8,
    /// Chase destination for non-hero targets (monster, item, or room gold);
    /// `None` when the destination is the hero (see [`Self::t_dest_hero`]) or unset.
    pub t_dest: Option<NonNull<IVec2>>,
    /// Whether the actor is chasing the hero rather than a stored [`Self::t_dest`].
    ///
    /// The hero's position is read live through [`crate::game::PLAYER`], so no
    /// raw pointer to the hero is ever stored.
    pub t_dest_hero: bool,
    pub t_flags: MonsterFlags,
    pub t_stats: Stats,
    pub t_room: Option<usize>,
    pub t_pack: Option<NonNull<Thing>>,
    pub t_reserved: i32,
}

/// Object (item) data for a [`CThing`], using native Rust types. The `o_text`
/// and `o_label` string fields are owned Rust `String`s rather than C pointers.
#[derive(Clone)]
pub struct ThingObject {
    pub o_type: i32,
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

/// Intrusive doubly-linked list header shared by every [`CThing`], independent
/// of whether the thing is an actor (monster/player) or an object (item). The
/// links use Rust `NonNull` pointers with a null niche instead of raw `*mut`.
#[derive(Copy, Clone)]
pub struct ThingLink {
    pub l_next: Option<NonNull<Thing>>,
    pub l_prev: Option<NonNull<Thing>>,
}

impl ThingLink {
    pub const fn empty() -> Self {
        ThingLink {
            l_next: None,
            l_prev: None,
        }
    }
}

/// A game thing: either an actor (monster/player) or an object (item), carrying
/// a shared intrusive list header. This is a pure Rust enum (`union`/C ABI has
/// been removed). It is `Clone` but not `Copy` because object string fields are
/// owned values.
#[derive(Clone)]
pub enum Thing {
    Monster { link: ThingLink, data: ThingMonster },
    Object { link: ThingLink, data: ThingObject },
}

// SAFETY: the game is single-threaded. `Thing` embeds `NonNull` handles (list
// links and pack heads) that make it neither `Send` nor `Sync` by default, but
// the game only ever mutates things from one thread. This marker lets the safe
// [`crate::game::MonsterList`] own monsters inside a `Mutex` without raw-pointer
// fields of its own.
unsafe impl Send for Thing {}
unsafe impl Sync for Thing {}

impl Thing {
    /// Build an actor thing with an empty list header.
    pub const fn actor(data: ThingMonster) -> Self {
        Thing::Monster {
            link: ThingLink::empty(),
            data,
        }
    }

    /// Build an object thing with an empty list header.
    pub const fn object(data: ThingObject) -> Self {
        Thing::Object {
            link: ThingLink::empty(),
            data,
        }
    }
}

impl Default for ThingMonster {
    fn default() -> Self {
        ThingMonster {
            t_pos: IVec2 { x: 0, y: 0 },
            t_turn: false,
            t_type: 0,
            t_disguise: 0,
            t_oldch: 0,
            t_dest: None,
            t_dest_hero: false,
            t_flags: MonsterFlags::NONE,
            t_stats: Stats::default(),
            t_room: None,
            t_pack: None,
            t_reserved: 0,
        }
    }
}

impl Default for ThingObject {
    fn default() -> Self {
        ThingObject {
            o_type: 0,
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

/// Read the next-list pointer of `tp` (null if `tp` is null).
#[inline]
pub unsafe fn thing_next(tp: *mut Thing) -> *mut Thing {
    if tp.is_null() {
        std::ptr::null_mut()
    } else {
        (*thing_link(tp))
            .l_next
            .map_or(std::ptr::null_mut(), |p| p.as_ptr())
    }
}

/// Set the next-list pointer of `tp`.
#[inline]
pub unsafe fn set_thing_next(tp: *mut Thing, value: *mut Thing) {
    (*thing_link(tp)).l_next = NonNull::new(value);
}

/// Read the prev-list pointer of `tp` (null if `tp` is null).
#[inline]
pub unsafe fn thing_prev(tp: *mut Thing) -> *mut Thing {
    if tp.is_null() {
        std::ptr::null_mut()
    } else {
        (*thing_link(tp))
            .l_prev
            .map_or(std::ptr::null_mut(), |p| p.as_ptr())
    }
}

/// Set the prev-list pointer of `tp`.
#[inline]
pub unsafe fn set_thing_prev(tp: *mut Thing, value: *mut Thing) {
    (*thing_link(tp)).l_prev = NonNull::new(value);
}

/// Read the actor's chase destination as a raw pointer (null when unset).
#[inline]
pub unsafe fn thing_dest(tp: *mut Thing) -> *mut IVec2 {
    (*thing_t(tp))
        .t_dest
        .map_or(std::ptr::null_mut(), |p| p.as_ptr())
}

/// Set the actor's chase destination from a raw pointer.
///
/// A non-null `value` clears the "chasing the hero" marker; a null value leaves
/// the marker untouched so callers can clear [`set_thing_dest_hero`] separately.
#[inline]
pub unsafe fn set_thing_dest(tp: *mut Thing, value: *mut IVec2) {
    (*thing_t(tp)).t_dest = NonNull::new(value);
    if !value.is_null() {
        (*thing_t(tp)).t_dest_hero = false;
    }
}

/// Whether the actor is chasing the hero.
#[inline]
pub unsafe fn is_thing_dest_hero(tp: *mut Thing) -> bool {
    (*thing_t(tp)).t_dest_hero
}

/// Make the actor chase the hero (clearing any stored coordinate).
#[inline]
pub unsafe fn set_thing_dest_hero(tp: *mut Thing) {
    (*thing_t(tp)).t_dest = None;
    (*thing_t(tp)).t_dest_hero = true;
}

/// Read the actor's pack head as a raw pointer (null when empty).
#[inline]
pub unsafe fn thing_pack(tp: *mut Thing) -> *mut Thing {
    (*thing_t(tp))
        .t_pack
        .map_or(std::ptr::null_mut(), |p| p.as_ptr())
}

/// Set the actor's pack head from a raw pointer.
#[inline]
pub unsafe fn set_thing_pack(tp: *mut Thing, value: *mut Thing) {
    (*thing_t(tp)).t_pack = NonNull::new(value);
}

unsafe extern "C" {
    static mut after: c_uchar;
    static mut count: c_int;
    static mut door_stop: c_uchar;
    static mut firstmove: c_uchar;
    static mut jump: c_uchar;
    static mut move_on: c_uchar;
    static mut no_command: c_int;
    static mut no_move: c_int;
    static mut passgo: c_uchar;
    static mut running: c_uchar;
    static mut seenstairs: c_uchar;
    static mut take: c_char;
    static mut to_death: c_uchar;
    static mut oldpos: IVec2;
    static mut delta: IVec2;
    static mut runch: c_char;

}

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

/// Borrow the shared list header of `tp`.
#[inline]
pub unsafe fn thing_link(tp: *mut Thing) -> *mut ThingLink {
    match &mut *tp {
        Thing::Monster { link, .. } => link as *mut ThingLink,
        Thing::Object { link, .. } => link as *mut ThingLink,
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
unsafe fn is_upper(ch: c_char) -> bool {
    (ch as u8).is_ascii_uppercase()
}

/// turn_ok:
/// Decide whether it is legal to turn onto the given space.
#[no_mangle]
pub unsafe extern "C" fn turn_ok(y: c_int, x: c_int) -> c_uchar {
    let flags = flat_at(y, x) as u8;
    if crate::game::is_door_at(y, x)
        || (flags & (F_REAL as u8 | F_PASS as u8)) == (F_REAL as u8 | F_PASS as u8)
    {
        true as c_uchar
    } else {
        0
    }
}

#[inline]
unsafe fn move_stuff(next_pos: &mut IVec2, fl: c_char) {
    let hero = PLAYER.pos();
    output::write_glyph_at(IVec2::new(hero.x, hero.y), (floor_at() as u8) as char);
    if (fl as u8 & F_PASS as u8) != 0 && crate::game::is_door_at(oldpos.y, oldpos.x) {
        draw_leave_room(next_pos);
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

    running = false as c_uchar;
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
                fall(arrow, false as c_uchar);
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
            b'a' as c_char
        } else {
            b'd' as c_char
        });
    }

    flush_type();
    trap
}

#[inline]
unsafe fn try_passgo_turn(dy: &mut c_int, dx: &mut c_int) -> bool {
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
    if runch == b'h' as c_char || runch == b'l' as c_char {
        let b1 = hero.y != 1 && turn_ok(hero.y - 1, hero.x) != 0;
        let b2 = hero.y != GameConfig::SCREEN_LINES - 2 && turn_ok(hero.y + 1, hero.x) != 0;
        if !(b1 ^ b2) {
            return false;
        }
        if b1 {
            runch = b'k' as c_char;
            *dy = -1;
        } else {
            runch = b'j' as c_char;
            *dy = 1;
        }
        *dx = 0;
        draw_turnref();
        true
    } else if runch == b'j' as c_char || runch == b'k' as c_char {
        let b1 = hero.x != 0 && turn_ok(hero.y, hero.x - 1) != 0;
        let b2 = hero.x != GameConfig::SCREEN_COLS - 1 && turn_ok(hero.y, hero.x + 1) != 0;
        if !(b1 ^ b2) {
            return false;
        }
        if b1 {
            runch = b'h' as c_char;
            *dx = -1;
        } else {
            runch = b'l' as c_char;
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
#[no_mangle]
pub static mut nh: IVec2 = IVec2 { x: 0, y: 0 };

/// do_run:
/// Start the hero running in the chosen direction.
#[no_mangle]
pub unsafe extern "C" fn do_run(ch: c_char) {
    running = true as c_uchar;
    after = false as c_uchar;
    runch = ch;
}

/// do_move:
/// Check to see that a move is legal. If it is, handle the consequences.
#[no_mangle]
pub unsafe extern "C" fn do_move(dy: c_int, dx: c_int) {
    let mut next_pos = IVec2 { x: 0, y: 0 };
    let mut current_dy = dy;
    let mut current_dx = dx;
    let hero = PLAYER.pos();
    let mut ch: c_char;
    let fl: c_char;

    firstmove = false as c_uchar;
    if no_move != 0 {
        no_move -= 1;
        msg_str("you are still stuck in the bear trap");
        return;
    }

    if player_has(MonsterFlags::HUH) && rnd(5) != 0 {
        next_pos = crate::entity::rndmove::rndmove_from(hero);
        if coord_eq(next_pos, hero) {
            after = false as c_uchar;
            running = false as c_uchar;
            to_death = false as c_uchar;
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
            running = false as c_uchar;
            after = false as c_uchar;
            return;
        }
        break;
    }

    let mut hero_copy = hero;
    if diag_ok(&raw mut hero_copy, &mut next_pos) == 0 {
        after = false as c_uchar;
        running = false as c_uchar;
        return;
    }

    if running != 0 && coord_eq(hero, next_pos) {
        running = false as c_uchar;
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
    } else if player_has(MonsterFlags::HELD) && ch != b'F' as c_char {
        msg_str("you are being held");
        return;
    }
    match ch {
        SPACE | H_WALL | V_WALL => {
            running = false as c_uchar;
            after = false as c_uchar;
        }
        DOOR => {
            running = false as c_uchar;
            if (flat_at(hero.y, hero.x) as u8 & F_PASS as u8) != 0 {
                draw_enter_room(&mut next_pos);
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
            let mut hero_copy = hero;
            PLAYER.set_room(roomin(&raw mut hero_copy));
            move_stuff(&mut next_pos, fl);
        }
        FLOOR => {
            if (fl as u8 & F_REAL as u8) == 0 {
                be_trapped(PLAYER.pos());
            }
            move_stuff(&mut next_pos, fl);
        }
        STAIRS => {
            seenstairs = true as c_uchar;
            running = false as c_uchar;
            if is_upper(ch) || !game::monster_at(next_pos.y, next_pos.x).is_null() {
                fight(&mut next_pos, game::PLAYER.weapon(), false as c_uchar);
            } else {
                take = ch;
                move_stuff(&mut next_pos, fl);
            }
        }
        _ => {
            running = false as c_uchar;
            if is_upper(ch) || !game::monster_at(next_pos.y, next_pos.x).is_null() {
                fight(&mut next_pos, game::PLAYER.weapon(), false as c_uchar);
            } else {
                if ch != STAIRS {
                    take = ch;
                }
                move_stuff(&mut next_pos, fl);
            }
        }
    }
}
