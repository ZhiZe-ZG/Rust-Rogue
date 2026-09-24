//! Player movement, running, and the player/monster `THING` types.
//!
//! Ported from `src/c/move.c` to Rust, together with the shared `THING`,
//! `PLACE`, and `COORD` layouts the rest of the port relies on.
use crate::config::GameConfig;
use crate::draw::{
    enter_room as draw_enter_room, flat_at, leave_room as draw_leave_room,
    turnref as draw_turnref, winat,
};
use crate::entity::chase::{diag_ok, roomin};
use crate::entity::fight::{fight, swing};
use crate::entity::monsters::save;
use crate::entity::rndmove::rndmove;
use crate::game;
use crate::game::EQUIPMENT;
use crate::item::armor::rust_armor;
use crate::item::pack::floor_at;
use crate::item::rings::RingType;
use crate::item::thing_list::new_item;
use crate::item::weapons::{fall, init_weapon};
use crate::level::{new_level, Trap, TrapHit};
use crate::machdep::flush_type;
use crate::misc::{chg_str, spread};
use crate::rip::death;
use crate::rnd::rnd;
use crate::startup::roll;
use crate::ui::output;
use crate::ui::output::msg_str;
use crate::wizard::teleport;
use glam::IVec2;
use std::os::raw::{c_char, c_int, c_short, c_uchar};

pub use crate::entity::stats::Stats;

const DOOR: c_char = b'+' as c_char;
const FLOOR: c_char = b'.' as c_char;
const PASSAGE: c_char = b'#' as c_char;
const TRAP: c_char = b'^' as c_char;
const STAIRS: c_char = b'%' as c_char;
const SPACE: c_char = b' ' as c_char;
const H_WALL: c_char = b'-' as c_char;
const V_WALL: c_char = b'|' as c_char;

const ISBLIND: c_short = 0o0000004;
const ISHELD: c_short = 0o0000400;
const ISHUH: c_short = 0o0001000;
const ISLEVIT: c_short = 0o0000010;
const ISRUN: c_short = 0o020000;

const F_PASS: c_char = 0x80u8 as c_char;
const F_REAL: c_char = 0x10u8 as c_char;

const ARROW: c_int = 3;
const VS_POISON: c_int = 0;

/// Monster/player (actor) data for a [`CThing`], using native Rust types.
#[derive(Copy, Clone)]
pub struct CThingMonster {
    pub t_pos: IVec2,
    pub t_turn: bool,
    pub t_type: u8,
    pub t_disguise: u8,
    pub t_oldch: u8,
    pub t_dest: *mut IVec2,
    pub t_flags: i16,
    pub t_stats: Stats,
    pub t_room: Option<usize>,
    pub t_pack: *mut CThing,
    pub t_reserved: i32,
}

/// Object (item) data for a [`CThing`], using native Rust types. The `o_text`
/// and `o_label` string fields stay raw pointers because objects are still
/// referenced through raw pointers that alias the owning arena.
#[derive(Copy, Clone)]
pub struct CThingObject {
    pub o_type: i32,
    pub o_pos: IVec2,
    pub o_text: *mut c_char,
    pub o_launch: i32,
    pub o_packch: u8,
    pub o_damage: [u8; 8],
    pub o_hurldmg: [u8; 8],
    pub o_count: i32,
    pub o_which: i32,
    pub o_hplus: i32,
    pub o_dplus: i32,
    pub o_arm: i32,
    pub o_flags: i32,
    pub o_group: i32,
    pub o_label: *mut c_char,
}

/// Intrusive doubly-linked list header shared by every [`CThing`], independent
/// of whether the thing is an actor (monster/player) or an object (item).
#[derive(Copy, Clone)]
pub struct ThingLink {
    pub l_next: *mut CThing,
    pub l_prev: *mut CThing,
}

impl ThingLink {
    pub const fn empty() -> Self {
        ThingLink {
            l_next: std::ptr::null_mut(),
            l_prev: std::ptr::null_mut(),
        }
    }
}

/// A game thing: either an actor (monster/player) or an object (item), carrying
/// a shared intrusive list header. This is a pure Rust enum (`union`/C ABI has
/// been removed).
#[derive(Copy, Clone)]
pub enum CThing {
    Monster { link: ThingLink, data: CThingMonster },
    Object { link: ThingLink, data: CThingObject },
}

impl CThing {
    /// Build an actor thing with an empty list header.
    pub const fn actor(data: CThingMonster) -> Self {
        CThing::Monster {
            link: ThingLink::empty(),
            data,
        }
    }

    /// Build an object thing with an empty list header.
    pub const fn object(data: CThingObject) -> Self {
        CThing::Object {
            link: ThingLink::empty(),
            data,
        }
    }
}

impl Default for CThingMonster {
    fn default() -> Self {
        CThingMonster {
            t_pos: IVec2 { x: 0, y: 0 },
            t_turn: false,
            t_type: 0,
            t_disguise: 0,
            t_oldch: 0,
            t_dest: std::ptr::null_mut(),
            t_flags: 0,
            t_stats: Stats::default(),
            t_room: None,
            t_pack: std::ptr::null_mut(),
            t_reserved: 0,
        }
    }
}

impl Default for CThingObject {
    fn default() -> Self {
        CThingObject {
            o_type: 0,
            o_pos: IVec2 { x: 0, y: 0 },
            o_text: std::ptr::null_mut(),
            o_launch: 0,
            o_packch: 0,
            o_damage: [0; 8],
            o_hurldmg: [0; 8],
            o_count: 0,
            o_which: 0,
            o_hplus: 0,
            o_dplus: 0,
            o_arm: 0,
            o_flags: 0,
            o_group: 0,
            o_label: std::ptr::null_mut(),
        }
    }
}

/// Read the next-list pointer of `tp` (null if `tp` is null).
#[inline]
pub unsafe fn thing_next(tp: *mut CThing) -> *mut CThing {
    if tp.is_null() {
        std::ptr::null_mut()
    } else {
        (*thing_link(tp)).l_next
    }
}

/// Set the next-list pointer of `tp`.
#[inline]
pub unsafe fn set_thing_next(tp: *mut CThing, value: *mut CThing) {
    (*thing_link(tp)).l_next = value;
}

/// Read the prev-list pointer of `tp` (null if `tp` is null).
#[inline]
pub unsafe fn thing_prev(tp: *mut CThing) -> *mut CThing {
    if tp.is_null() {
        std::ptr::null_mut()
    } else {
        (*thing_link(tp)).l_prev
    }
}

/// Set the prev-list pointer of `tp`.
#[inline]
pub unsafe fn set_thing_prev(tp: *mut CThing, value: *mut CThing) {
    (*thing_link(tp)).l_prev = value;
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
    static mut player: CThing;
    static mut runch: c_char;

}

/// Borrow the actor payload of `tp` (null when `tp` is an object).
#[inline]
pub unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    match &mut *tp {
        CThing::Monster { data, .. } => data as *mut CThingMonster,
        CThing::Object { .. } => std::ptr::null_mut(),
    }
}

/// Borrow the object payload of `tp` (null when `tp` is an actor).
#[inline]
pub unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    match &mut *tp {
        CThing::Object { data, .. } => data as *mut CThingObject,
        CThing::Monster { .. } => std::ptr::null_mut(),
    }
}

/// Borrow the shared list header of `tp`.
#[inline]
pub unsafe fn thing_link(tp: *mut CThing) -> *mut ThingLink {
    match &mut *tp {
        CThing::Monster { link, .. } => link as *mut ThingLink,
        CThing::Object { link, .. } => link as *mut ThingLink,
    }
}

#[inline]
unsafe fn hero_ptr() -> *mut IVec2 {
    &mut (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn hero_pos() -> IVec2 {
    (*thing_t(&raw mut player)).t_pos
}

#[inline]
unsafe fn ring_is(ring: *mut CThing, ring_type: RingType) -> bool {
    !ring.is_null() && RingType::from_raw((*thing_o(ring)).o_which) == Some(ring_type)
}

#[inline]
unsafe fn player_has(flag: c_short) -> bool {
    ((*thing_t(&raw mut player)).t_flags & flag) != 0
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
    let hero = hero_pos();
    output::write_glyph_at(IVec2::new(hero.x, hero.y), (floor_at() as u8) as char);
    if (fl as u8 & F_PASS as u8) != 0 && crate::game::is_door_at(oldpos.y, oldpos.x) {
        draw_leave_room(next_pos);
    }
    *hero_ptr() = *next_pos;
}

/// Applies the trap at the given map cell, returning the trap kind that fired.
///
/// The cell's trap nibble holds the trap number (0-7). If the hero is
/// levitating, no trap effect applies. Uses the C engine helpers (`msg`,
/// `roll`, `spread`, `teleport`, ...) exactly as the legacy `be_trapped` did,
/// but is callable only from Rust.
pub unsafe fn be_trapped(pos: IVec2) -> Trap {
    let trap =
        crate::level::with_current_level(|current| current.trap_at(pos.y as usize, pos.x as usize));

    if ((*thing_t(&raw mut player)).t_flags & ISLEVIT) != 0 {
        return Trap::Rust;
    }

    running = false as c_uchar;
    count = 0;
    crate::level::with_current_level_mut(|current| {
        current.reveal_trap(pos.y as usize, pos.x as usize);
    });

    let mut hit = TrapHit::Miss;

    match trap {
        Trap::Door => {
            crate::game::set_current_depth(crate::game::current_depth() + 1);
            new_level();
        }
        Trap::Bear => {
            no_move += spread(3);
        }
        Trap::Mystery => {}
        Trap::Sleep => {
            no_command += spread(5);
            (*thing_t(&raw mut player)).t_flags &= !ISRUN;
        }
        Trap::Arrow => {
            let stats = &mut (*thing_t(&raw mut player)).t_stats;
            if swing(stats.level - 1, stats.armor, 1) != 0 {
                stats.hit_points -= roll(1, 6);
                hit = if stats.hit_points <= 0 {
                    TrapHit::Kill
                } else {
                    TrapHit::Hit
                };
            } else {
                let arrow = new_item();
                init_weapon(arrow, ARROW);
                (*thing_o(arrow)).o_count = 1;
                (*thing_o(arrow)).o_pos = hero_pos();
                fall(arrow, false as c_uchar);
                hit = TrapHit::Miss;
            }
        }
        Trap::Teleport => {
            teleport();
        }
        Trap::Dart => {
            let stats = &mut (*thing_t(&raw mut player)).t_stats;
            if swing(stats.level + 1, stats.armor, 1) == 0 {
                hit = TrapHit::Miss;
            } else {
                stats.hit_points -= roll(1, 4);
                if stats.hit_points <= 0 {
                    hit = TrapHit::Kill;
                } else {
                    if !ring_is(EQUIPMENT.left_ring(), RingType::SustainStrength)
                        && !ring_is(EQUIPMENT.right_ring(), RingType::SustainStrength)
                        && save(VS_POISON) == 0
                    {
                        chg_str(-1);
                    }
                    hit = TrapHit::Hit;
                }
            }
        }
        Trap::Rust => {
            if let Some(msg) = trap.msg(hit) {
                msg_str(&msg);
            }
            rust_armor(EQUIPMENT.armor());
        }
    }

    // Send the message after the effect for every trap except `Rust`, which
    // prints its message before applying the effect above.
    if trap != Trap::Rust {
        if let Some(msg) = trap.msg(hit) {
            msg_str(&msg);
        }
    }

    if hit == TrapHit::Kill {
        death(if trap == Trap::Arrow {
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
    let current_room = (*thing_t(&raw mut player)).t_room;
    if passgo == 0
        || running == 0
        || current_room.is_none()
        || !crate::game::room_gone(current_room)
        || player_has(ISBLIND)
    {
        return false;
    }

    let hero = hero_pos();
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
    let hero = hero_pos();
    let mut ch: c_char;
    let fl: c_char;

    firstmove = false as c_uchar;
    if no_move != 0 {
        no_move -= 1;
        msg_str("you are still stuck in the bear trap");
        return;
    }

    if player_has(ISHUH) && rnd(5) != 0 {
        next_pos = *rndmove(&raw mut player);
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

    if diag_ok(hero_ptr(), &mut next_pos) == 0 {
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
        if !player_has(ISLEVIT) {
            crate::level::with_current_level_mut(|level| {
                level.reveal_trap(next_pos.y as usize, next_pos.x as usize);
            });
            ch = TRAP;
        }
    } else if player_has(ISHELD) && ch != b'F' as c_char {
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
            if trap == Trap::Door || trap == Trap::Teleport {
                return;
            }
            move_stuff(&mut next_pos, fl);
        }
        PASSAGE => {
            (*thing_t(&raw mut player)).t_room = roomin(hero_ptr());
            move_stuff(&mut next_pos, fl);
        }
        FLOOR => {
            if (fl as u8 & F_REAL as u8) == 0 {
                be_trapped(hero_pos());
            }
            move_stuff(&mut next_pos, fl);
        }
        STAIRS => {
            seenstairs = true as c_uchar;
            running = false as c_uchar;
            if is_upper(ch) || !game::monster_at(next_pos.y, next_pos.x).is_null() {
                fight(&mut next_pos, game::EQUIPMENT.weapon(), false as c_uchar);
            } else {
                take = ch;
                move_stuff(&mut next_pos, fl);
            }
        }
        _ => {
            running = false as c_uchar;
            if is_upper(ch) || !game::monster_at(next_pos.y, next_pos.x).is_null() {
                fight(&mut next_pos, game::EQUIPMENT.weapon(), false as c_uchar);
            } else {
                if ch != STAIRS {
                    take = ch;
                }
                move_stuff(&mut next_pos, fl);
            }
        }
    }
}
