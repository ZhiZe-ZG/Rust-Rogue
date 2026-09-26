//! Port of `src/c/chase.c` — one creature chasing another.
//!
//! All functions are exported with the same C ABI as the original, so the
//! C object files can call them directly.  Constants that in C live behind
//! `#ifdef MASTER` (the `debug`/`abort` debug helpers) are replaced by plain
//! Rust `const` values so the debug behavior is always available and there is
//! no need for preprocessor conditionals in Rust.


use crate::config::GameConfig;
use crate::entity::fight::attack;
use crate::entity::player::{set_thing_dest, set_thing_dest_hero, thing_dest};
use crate::entity::player::{MonsterFlags, Thing, ThingMonster, ThingObject};
use crate::entity::rndmove::rndmove;
use crate::game::MONSTER_LIST;
use crate::globals::monsters;
use crate::item::scrolls::ScrollType;
use crate::item::sticks::fire_bolt;
use crate::entity::player::attach_pack;
use crate::misc::sign;
use crate::rnd::rnd;
use crate::ui::output;
use crate::ui::output::{endmsg, msg_str};
use glam::IVec2;

const DRAGONSHOT: i32 = 5; // one chance in DRAGONSHOT that a dragon will flame

const F_PASS: u8 = 0x80u8 as u8;
const F_PNUM: u8 = 0x0fu8 as u8;

const DOOR: u8 = b'+' as u8;
const FLOOR: u8 = b'.' as u8;
const PASSAGE: u8 = b'#' as u8;
const SCROLL: u8 = b'?' as u8;
const BOLT_LENGTH: i32 = 6;
const LAMPDIST: i32 = 3;

/// `#ifdef MASTER` helper: replaced by a plain `const` so the preprocessor
/// conditional disappears.  The C build is compiled without `-DMASTER`, so the
/// debug-only `msg`/`abort` paths are disabled here too; flip to `true` to
/// enable the wizard/debug diagnostics.
const MASTER: bool = false;

/// Where chasing takes you (persistent return slot, mirrors C's `static coord ch_ret`).
static mut CH_RET: IVec2 = IVec2 { x: 0, y: 0 };
/// Temporary destination for chaser (mirrors C's `static coord this`).
static mut THIS: IVec2 = IVec2 { x: 0, y: 0 };
/// Temporary try position (mirrors C's `static coord tryp`).
static mut TRYP: IVec2 = IVec2 { x: 0, y: 0 };
/// Temporary coord for cansee (mirrors C's `static coord tp`).
static mut CANSEE_TP: IVec2 = IVec2 { x: 0, y: 0 };

use crate::globals::{count, delta, has_hit, kamikaze, quiet, running, see_floor, to_death};


#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

#[inline]
fn hero_pos() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    crate::game::PLAYER.has_flag(flag)
}

/// The effective chase destination of `th`: the hero's live position when the
/// actor is chasing the hero, otherwise its stored coordinate.
#[inline]
unsafe fn dest_coord(th: *mut Thing) -> IVec2 {
    if crate::entity::player::is_thing_dest_hero(th) {
        return hero_pos();
    }
    let d = thing_dest(th);
    if d.is_null() {
        hero_pos()
    } else {
        *d
    }
}

#[inline]
unsafe fn monster_has(tp: *mut Thing, flag: MonsterFlags) -> bool {
    (*thing_t(tp)).t_flags.contains(flag)
}

#[inline]
unsafe fn coord_eq(a: IVec2, b: IVec2) -> bool {
    a.x == b.x && a.y == b.y
}

#[inline]
unsafe fn flat_at(y: i32, x: i32) -> u8 {
    crate::draw::flat_at(y, x)
}

#[inline]
unsafe fn moat_at(y: i32, x: i32) -> *mut Thing {
    crate::game::monster_at(y, x)
}

#[inline]
unsafe fn set_moat_at(y: i32, x: i32, tp: *mut Thing) {
    crate::game::set_monster(y, x, tp);
}

/// runners:
/// Make all the running monsters move.
///
/// Uses globals: mlist, hero, to_death, has_hit.
pub unsafe fn runners() {
    for id in MONSTER_LIST.ids() {
        if let Some(tp) = MONSTER_LIST.handle(id) {
            if !monster_has(tp, MonsterFlags::HELD) && monster_has(tp, MonsterFlags::RUN) {
                let orig_pos = (*thing_t(tp)).t_pos;
                let wastarget = monster_has(tp, MonsterFlags::TARGET);
                if move_monst(tp) == -1 {
                    continue;
                }
                let hero = hero_pos();
                if monster_has(tp, MonsterFlags::FLY)
                    && dist(
                        hero.y,
                        hero.x,
                        (*thing_t(tp)).t_pos.y,
                        (*thing_t(tp)).t_pos.x,
                    ) >= 3
                {
                    move_monst(tp);
                }
                if wastarget && !coord_eq(orig_pos, (*thing_t(tp)).t_pos) {
                    (*thing_t(tp)).t_flags.remove(MonsterFlags::TARGET);
                    to_death = false as u8;
                }
            }
        }
    }
    if has_hit != 0 {
        endmsg();
        has_hit = false as u8;
    }
}

/// move_monst:
/// Execute a single turn of running for a monster
pub unsafe fn move_monst(tp: *mut Thing) -> i32 {
    if !monster_has(tp, MonsterFlags::SLOW) || (*thing_t(tp)).t_turn {
        if do_chase(tp) == -1 {
            return -1;
        }
    }
    if monster_has(tp, MonsterFlags::HASTE) {
        if do_chase(tp) == -1 {
            return -1;
        }
    }
    (*thing_t(tp)).t_turn = !(*thing_t(tp)).t_turn;
    0
}

/// relocate:
/// Make the monster's new location be the specified one, updating
/// all the relevant state.
///
/// Uses globals: places (via moat), player, see_monst (function).
pub unsafe fn relocate(th: *mut Thing, new_loc: *mut IVec2) {
    if new_loc.is_null() {
        return;
    }
    if !coord_eq(*new_loc, (*thing_t(th)).t_pos) {
        output::write_glyph_at(
            IVec2::new((*thing_t(th)).t_pos.x, (*thing_t(th)).t_pos.y),
            ((*thing_t(th)).t_oldch as u8) as char,
        );
        (*thing_t(th)).t_room = roomin(new_loc);
        set_oldch(th, new_loc);
        let oroom = (*thing_t(th)).t_room;
        set_moat_at(
            (*thing_t(th)).t_pos.y,
            (*thing_t(th)).t_pos.x,
            std::ptr::null_mut(),
        );

        if oroom != (*thing_t(th)).t_room {
            update_dest(th);
        }
        (*thing_t(th)).t_pos = *new_loc;
        set_moat_at((*new_loc).y, (*new_loc).x, th);
    }
    output::move_cursor(IVec2::new((*new_loc).x, (*new_loc).y));
    if see_monst(th) != false as u8 {
        output::write_glyph(((*thing_t(th)).t_disguise as u8) as char);
    } else if player_has(MonsterFlags::SEEMONST) {
        output::set_standout(true);
        output::write_glyph(((*thing_t(th)).t_type as u8) as char);
        output::set_standout(false);
    }
}

/// do_chase:
/// Make one thing chase another.
///
/// Uses globals: hero, proom, passages, places (via flat/chat/moat),
/// delta, running, count, quiet, to_death, kamikaze, lvl_obj.
pub unsafe fn do_chase(th: *mut Thing) -> i32 {
    let mut mindist: i32 = 32767;
    let mut curdist: i32;
    let mut stoprun = false; // true as u8 means we are there
    let door: bool;
    let mut obj: *mut Thing;

    let rer = (*thing_t(th)).t_room; // Find room of chaser
    if monster_has(th, MonsterFlags::GREED) && crate::game::room_goldval(rer) == 0 {
        set_thing_dest_hero(th); // If gold has been taken, run after hero
    }
    let ree = if crate::entity::player::is_thing_dest_hero(th) {
        // Find room of chasee
        crate::game::PLAYER.room()
    } else {
        roomin(thing_dest(th))
    };
    // We don't count doors as inside rooms for this routine
    door = crate::game::is_door_at((*thing_t(th)).t_pos.y, (*thing_t(th)).t_pos.x);
    // If the object of our desire is in a different room,
    // and we are not in a corridor, run to the door nearest to
    // our goal.
    let mut loop_rer = rer;
    let mut loop_door = door;
    let loop_ree = ree;
    loop {
        if loop_rer != loop_ree {
            let dest = dest_coord(th);
            let exits = crate::game::room_exits(loop_rer);
            for cp in exits.iter() {
                curdist = dist(dest.y, dest.x, cp.y, cp.x);
                if curdist < mindist {
                    THIS = *cp;
                    mindist = curdist;
                }
            }
            if loop_door {
                let pnum =
                    (flat_at((*thing_t(th)).t_pos.y, (*thing_t(th)).t_pos.x) & F_PNUM) as usize;
                let passage_exits = crate::game::passage_exits(Some(pnum));
                for cp in passage_exits.iter() {
                    curdist = dist(dest.y, dest.x, cp.y, cp.x);
                    if curdist < mindist {
                        THIS = *cp;
                        mindist = curdist;
                    }
                }
                loop_rer = loop_ree;
                loop_door = false;
                continue;
            }
        } else {
            THIS = dest_coord(th);
            // For dragons check and see if (a) the hero is on a straight
            // line from it, and (b) that it is within shooting distance,
            // but outside of striking range.
            let hero = hero_pos();
            if (*thing_t(th)).t_type == b'D'
                && ((*thing_t(th)).t_pos.y == hero.y
                    || (*thing_t(th)).t_pos.x == hero.x
                    || ((*thing_t(th)).t_pos.y - hero.y).abs()
                        == ((*thing_t(th)).t_pos.x - hero.x).abs())
                && dist(
                    (*thing_t(th)).t_pos.y,
                    (*thing_t(th)).t_pos.x,
                    hero.y,
                    hero.x,
                ) <= BOLT_LENGTH * BOLT_LENGTH
                && !monster_has(th, MonsterFlags::CANCELLED)
                && rnd(DRAGONSHOT) == 0
            {
                let dy = hero.y - (*thing_t(th)).t_pos.y;
                let dx = hero.x - (*thing_t(th)).t_pos.x;
                delta.y = sign(dy);
                delta.x = sign(dx);
                if has_hit != 0 {
                    endmsg();
                }
                fire_bolt((*thing_t(th)).t_pos, delta, "flame");
                running = false as u8;
                count = 0;
                quiet = 0;
                if to_death != 0 && !monster_has(th, MonsterFlags::TARGET) {
                    to_death = false as u8;
                    kamikaze = false as u8;
                }
                return 0;
            }
        }
        break;
    }

    // This now contains what we want to run to this time
    // so we run to it.  If we hit it we either want to fight it
    // or stop running.
    if chase(th, &raw mut THIS) == false as u8 {
        if coord_eq(THIS, hero_pos()) {
            return attack(th);
        } else if coord_eq(THIS, dest_coord(th)) {
            obj = crate::game::with_current_level(|level| level.items.head());
            while !obj.is_null() {
                if thing_dest(th) == &raw mut (*thing_o(obj)).o_pos {
                    crate::game::with_current_level_mut(|level| level.items.detach(obj));
                    attach_pack(th, obj);
                    // Objects render from the `lvl_obj` list; the floor glyph
                    // under a picked-up object is then the terrain char.
                    update_dest(th);
                    break;
                }
                obj = crate::entity::player::thing_next(obj);
            }
            if (*thing_t(th)).t_type != b'F' {
                stoprun = true;
            }
        }
    } else if (*thing_t(th)).t_type == b'F' {
        return 0;
    }
    relocate(th, &raw mut CH_RET);
    // And stop running if need be
    if stoprun && coord_eq((*thing_t(th)).t_pos, dest_coord(th)) {
        (*thing_t(th)).t_flags.remove(MonsterFlags::RUN);
    }
    0
}

/// set_oldch:
/// Set the oldch character for the monster
///
/// Uses globals: player, hero, see_floor, places (via chat).
pub unsafe fn set_oldch(tp: *mut Thing, cp: *mut IVec2) {
    if coord_eq((*thing_t(tp)).t_pos, *cp) {
        return;
    }

    let sch = (*thing_t(tp)).t_oldch;
    (*thing_t(tp)).t_oldch = output::glyph_at(IVec2::new((*cp).x, (*cp).y)) as u8 & 0x7f;
    if !player_has(MonsterFlags::BLIND) {
        if (sch == FLOOR as u8 || (*thing_t(tp)).t_oldch == FLOOR as u8)
            && crate::game::room_dark((*thing_t(tp)).t_room)
        {
            (*thing_t(tp)).t_oldch = b' ';
        } else if dist((*cp).y, (*cp).x, hero_pos().y, hero_pos().x) <= LAMPDIST && see_floor != 0 {
            (*thing_t(tp)).t_oldch = crate::draw::cell_glyph((*cp).y, (*cp).x) as u8;
        }
    }
}

/// see_monst:
/// Return true as u8 if the hero can see the monster
///
/// Uses globals: player, hero, proom, places (via chat).
pub unsafe fn see_monst(mp: *mut Thing) -> u8 {
    if player_has(MonsterFlags::BLIND) {
        return false as u8;
    }
    if monster_has(mp, MonsterFlags::INVIS) && !player_has(MonsterFlags::CANSEE) {
        return false as u8;
    }
    let y = (*thing_t(mp)).t_pos.y;
    let x = (*thing_t(mp)).t_pos.x;
    if dist(y, x, hero_pos().y, hero_pos().x) < LAMPDIST {
        if y != hero_pos().y
            && x != hero_pos().x
            && !crate::game::tile_at(y, hero_pos().x).is_walkable()
            && !crate::game::tile_at(hero_pos().y, x).is_walkable()
        {
            return false as u8;
        }
        return true as u8;
    }
    if (*thing_t(mp)).t_room != crate::game::PLAYER.room() {
        return false as u8;
    }
    if crate::game::room_dark((*thing_t(mp)).t_room) {
        false as u8
    } else {
        true as u8
    }
}

/// runto:
/// Set a monster running after the hero.
///
/// Uses globals: places (via moat).
pub unsafe fn runto(runner: *mut IVec2) {
    // If we couldn't find him, something is funny.
    // (C guarded this with `#ifdef MASTER`; always report in the Rust port.)
    let tp = moat_at((*runner).y, (*runner).x);
    if MASTER && tp.is_null() {
        msg_str(&format!(
            "couldn't find monster in runto at ({},{})",
            (*runner).y,
            (*runner).x
        ));
    }
    if tp.is_null() {
        return;
    }
    // Start the beastie running
    (*thing_t(tp)).t_flags.insert(MonsterFlags::RUN);
    (*thing_t(tp)).t_flags.remove(MonsterFlags::HELD);
    update_dest(tp);
}

/// chase:
/// Find the spot for the chaser(er) to move closer to the
/// chasee(ee).  Returns true as u8 if we want to keep on chasing later
/// false as u8 if we reach the goal.
///
/// Uses globals: hero, lvl_obj, places (via moat/chat/winat).
pub unsafe fn chase(tp: *mut Thing, ee: *mut IVec2) -> u8 {
    let mut curdist: i32;
    let mut thisdist: i32;
    let er = &raw mut (*thing_t(tp)).t_pos;
    let mut plcnt = 1;

    // If the thing is confused, let it move randomly. Invisible
    // Stalkers are slightly confused all of the time, and bats are
    // quite confused all the time.
    if (monster_has(tp, MonsterFlags::HUH) && rnd(5) != 0)
        || ((*thing_t(tp)).t_type == b'P' && rnd(5) == 0)
        || ((*thing_t(tp)).t_type == b'B' && rnd(2) == 0)
    {
        // get a valid random move
        CH_RET = *rndmove(tp);
        curdist = dist_cp(&raw mut CH_RET, ee);
        // Small chance that it will become un-confused
        if rnd(20) == 0 {
            (*thing_t(tp)).t_flags.remove(MonsterFlags::HUH);
        }
    }
    // Otherwise, find the empty spot next to the chaser that is
    // closest to the chasee.
    else {
        // This will eventually hold where we move to get closer.
        // If we can't find an empty spot, we stay where we are.
        curdist = dist_cp(er, ee);
        CH_RET = *er;

        let mut ey = (*er).y + 1;
        if ey >= GameConfig::SCREEN_LINES - 1 {
            ey = GameConfig::SCREEN_LINES - 2;
        }
        let mut ex = (*er).x + 1;
        if ex >= GameConfig::SCREEN_COLS {
            ex = GameConfig::SCREEN_COLS - 1;
        }

        let mut x = (*er).x - 1;
        while x <= ex {
            if x >= 0 {
                TRYP.x = x;
                let mut y = (*er).y - 1;
                while y <= ey {
                    TRYP.y = y;
                    if diag_ok(er, &raw mut TRYP) == false as u8 {
                        y += 1;
                        continue;
                    }
                    if crate::game::cell_is_walkable(y, x) {
                        // If it is a scroll, it might be a scare monster scroll
                        // so we need to look it up to see what type it is.
                        let mut obj = crate::game::with_current_level(|level| level.items.head());
                        while !obj.is_null() {
                            if y == (*thing_o(obj)).o_pos.y && x == (*thing_o(obj)).o_pos.x {
                                break;
                            }
                            obj = crate::entity::player::thing_next(obj);
                        }
                        if !obj.is_null() && (*thing_o(obj)).o_which == ScrollType::Scare as i32 {
                            y += 1;
                            continue;
                        }
                        // It can also be a Xeroc, which we shouldn't step on.
                        let obj = moat_at(y, x);
                        if !obj.is_null() && (*thing_t(obj)).t_type == b'X' {
                            y += 1;
                            continue;
                        }
                        // If we didn't find any scrolls at this place or it
                        // wasn't a scare scroll, then this place counts.
                        thisdist = dist(y, x, (*ee).y, (*ee).x);
                        if thisdist < curdist {
                            plcnt = 1;
                            CH_RET = TRYP;
                            curdist = thisdist;
                        } else if thisdist == curdist && rnd(plcnt + 1) == 0 {
                            // C's rnd(++plcnt) bumps plcnt then draws in [0, plcnt).
                            plcnt += 1;
                            CH_RET = TRYP;
                            curdist = thisdist;
                        }
                    }
                    y += 1;
                }
            }
            x += 1;
        }
    }
    if curdist != 0 && !coord_eq(CH_RET, hero_pos()) {
        true as u8
    } else {
        false as u8
    }
}

/// roomin:
/// Find what room some coordinates are in. NULL means they aren't
/// in any room.
///
/// Uses globals: places (via flat), passages, rooms, msg.
pub unsafe fn roomin(cp: *mut IVec2) -> Option<usize> {
    if cp.is_null() {
        return None;
    }
    let room = crate::game::with_current_level(|level| level.room_at((*cp).y, (*cp).x));
    if room.is_some() {
        return room;
    }

    msg_str(&format!("in some bizarre place ({}, {})", (*cp).y, (*cp).x));
    if MASTER {
        std::process::abort();
    }
    None
}

/// diag_ok:
/// Check to see if the move is legal if it is diagonal
///
/// Uses globals: places (via chat).
pub unsafe fn diag_ok(sp: *mut IVec2, ep: *mut IVec2) -> u8 {
    if (*ep).x < 0
        || (*ep).x >= GameConfig::SCREEN_COLS
        || (*ep).y <= 0
        || (*ep).y >= GameConfig::SCREEN_LINES - 1
    {
        return false as u8;
    }
    if (*ep).x == (*sp).x || (*ep).y == (*sp).y {
        return true as u8;
    }
    if crate::game::tile_at((*ep).y, (*sp).x).is_walkable()
        && crate::game::tile_at((*sp).y, (*ep).x).is_walkable()
    {
        true as u8
    } else {
        false as u8
    }
}

/// cansee:
/// Returns true if the hero can see a certain coordinate.
///
/// Uses globals: player, hero, proom, places (via flat/chat).
pub unsafe fn cansee(y: i32, x: i32) -> u8 {
    if player_has(MonsterFlags::BLIND) {
        return false as u8;
    }
    if dist(y, x, hero_pos().y, hero_pos().x) < LAMPDIST {
        if (flat_at(y, x) & F_PASS) != 0 {
            if y != hero_pos().y
                && x != hero_pos().x
                && !crate::game::tile_at(y, hero_pos().x).is_walkable()
                && !crate::game::tile_at(hero_pos().y, x).is_walkable()
            {
                return false as u8;
            }
        }
        return true as u8;
    }
    // We can only see if the hero in the same room as
    // the coordinate and the room is lit or if it is close.
    CANSEE_TP.y = y;
    CANSEE_TP.x = x;
    let rer = roomin(&raw mut CANSEE_TP);
    if rer == crate::game::PLAYER.room() && !crate::game::room_dark(rer) {
        true as u8
    } else {
        false as u8
    }
}

/// update_dest:
/// Set the proper destination for the monster.
///
/// Chases the hero (recorded symbolically, never as a raw pointer) when the
/// monster carries nothing, is in the hero's room, or can see the hero;
/// otherwise it may target a nearby floor object it wants to pick up.
///
/// Uses globals: monsters, hero, proom, lvl_obj, mlist.
pub unsafe fn update_dest(tp: *mut Thing) {
    let prob = monsters[((*thing_t(tp)).t_type as i32 - 'A' as i32) as usize].m_carry;
    if prob <= 0
        || (*thing_t(tp)).t_room == crate::game::PLAYER.room()
        || see_monst(tp) != false as u8
    {
        set_thing_dest_hero(tp);
        return;
    }
    let mut obj = crate::game::with_current_level(|level| level.items.head());
    while !obj.is_null() {
        if (*thing_o(obj)).o_type == SCROLL as i32
            && (*thing_o(obj)).o_which == ScrollType::Scare as i32
        {
            obj = crate::entity::player::thing_next(obj);
            continue;
        }
        if roomin(&raw mut (*thing_o(obj)).o_pos) == (*thing_t(tp)).t_room && rnd(100) < prob {
            let obj_pos_ptr = &raw mut (*thing_o(obj)).o_pos;
            let mut taken = false;
            for mid in MONSTER_LIST.ids() {
                if let Some(m) = MONSTER_LIST.handle(mid) {
                    if thing_dest(m) == obj_pos_ptr {
                        taken = true;
                        break;
                    }
                }
            }
            if !taken {
                set_thing_dest(tp, obj_pos_ptr);
                return;
            }
        }
        obj = crate::entity::player::thing_next(obj);
    }
    set_thing_dest_hero(tp);
}

/// dist:
/// Calculate the "distance" between to points.  Actually,
/// this calculates d^2, not d, but that's good enough for
/// our purposes, since it's only used comparitively.
pub unsafe fn dist(y1: i32, x1: i32, y2: i32, x2: i32) -> i32 {
    (x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)
}

/// dist_cp:
/// Call dist() with appropriate arguments for coord pointers
pub unsafe fn dist_cp(c1: *mut IVec2, c2: *mut IVec2) -> i32 {
    dist((*c1).y, (*c1).x, (*c2).y, (*c2).x)
}
