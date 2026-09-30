//! Port of `src/c/chase.c` — one creature chasing another.
//!
//! Monsters are addressed by [`MonsterId`] and reached through scoped
//! [`MONSTER_LIST`] access, never raw pointers. Functions that need several
//! fields snapshot the monster into a small local value, compute with that,
//! and write the result back through `MONSTER_LIST.with_mut` — which keeps the
//! monster lock unheld while other locked state (e.g. the level) is read.

use crate::config::GameConfig;
use crate::entity::fight::attack;
use crate::entity::monsters::MonsterType;
use crate::entity::player::{DestRef, MonsterFlags, Thing};
use crate::entity::rndmove::rndmove;
use crate::game::globals::monsters;
use crate::game::{MonsterId, MONSTER_LIST};
use crate::item::item_type::ItemType;
use crate::item::scrolls::ScrollType;
use crate::item::sticks::fire_bolt;
use crate::misc::sign;
use crate::rnd::rnd;
use crate::ui::output::{endmsg, msg_str};
use glam::IVec2;

const DRAGONSHOT: i32 = 5; // one chance in DRAGONSHOT that a dragon will flame

const F_PASS: u8 = 0x80u8 as u8;
const F_PNUM: u8 = 0x0fu8 as u8;

const DOOR: u8 = b'+' as u8;
const FLOOR: u8 = b'.' as u8;
const PASSAGE: u8 = b'#' as u8;
const BOLT_LENGTH: i32 = 6;
const LAMPDIST: i32 = 3;

/// `#ifdef MASTER` helper: replaced by a plain `const` so the preprocessor
/// conditional disappears.  The C build is compiled without `-DMASTER`, so the
/// debug-only `msg`/`abort` paths are disabled here too; flip to `true` to
/// enable the wizard/debug diagnostics.
const MASTER: bool = false;

use crate::game::globals::{count, delta, has_hit, kamikaze, quiet, running, see_floor, to_death};

/// A snapshot of the actor fields `chase.c` reads and writes.
#[derive(Clone)]
struct Mon {
    pos: IVec2,
    turn: bool,
    typ: Option<MonsterType>,
    oldch: u8,
    dest: DestRef,
    flags: MonsterFlags,
    room: Option<usize>,
}

impl Mon {
    /// Read the monster `id` into a snapshot (or `None` when it is gone).
    fn get(id: MonsterId) -> Option<Mon> {
        MONSTER_LIST
            .with(id, |t| match t {
                Thing::Monster { data } => Some(Mon {
                    pos: data.t_pos,
                    turn: data.t_turn,
                    typ: data.t_type,
                    oldch: data.t_oldch,
                    dest: data.t_dest,
                    flags: data.t_flags,
                    room: data.t_room,
                }),
                Thing::Object { .. } => None,
            })
            .flatten()
    }

    /// Whether `flag` is set.
    #[inline]
    fn has(&self, flag: MonsterFlags) -> bool {
        self.flags.contains(flag)
    }

    /// Write the snapshot's mutable fields back onto the monster `id`.
    fn set(&self, id: MonsterId) {
        MONSTER_LIST.with_mut(id, |t| {
            if let Thing::Monster { data } = t {
                data.t_pos = self.pos;
                data.t_turn = self.turn;
                data.t_oldch = self.oldch;
                data.t_dest = self.dest;
                data.t_flags = self.flags;
                data.t_room = self.room;
            }
        });
    }
}

#[inline]
fn hero_pos() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    crate::game::PLAYER.has_flag(flag)
}

/// Resolve a stored [`DestRef`] to the live coordinate it names.
///
/// Every handle is looked up in its owning container; an unresolvable handle
/// (freed monster/item, out-of-range room) falls back to the hero's position.
#[inline]
fn resolve_dest(dest: DestRef) -> IVec2 {
    match dest {
        DestRef::None | DestRef::Hero => hero_pos(),
        DestRef::Monster(id) => MONSTER_LIST
            .with(id, |t| match t {
                Thing::Monster { data, .. } => data.t_pos,
                Thing::Object { .. } => hero_pos(),
            })
            .unwrap_or_else(hero_pos),
        DestRef::Object(id) => crate::item::arena::OBJECTS
            .with(id, |t| match t {
                Thing::Object { data, .. } => data.o_pos,
                Thing::Monster { .. } => hero_pos(),
            })
            .unwrap_or_else(hero_pos),
        DestRef::RoomGold(r) => {
            unsafe { crate::game::room_gold_pos(Some(r)) }.unwrap_or_else(hero_pos)
        }
    }
}

#[inline]
fn coord_eq(a: IVec2, b: IVec2) -> bool {
    a.x == b.x && a.y == b.y
}

#[inline]
unsafe fn flat_at(y: i32, x: i32) -> u8 {
    crate::draw::flat_at(y, x)
}

/// runners:
/// Make all the running monsters move.
///
/// Uses globals: mlist, hero, to_death, has_hit.
pub unsafe fn runners() {
    for id in MONSTER_LIST.ids() {
        let Some(m) = Mon::get(id) else {
            continue;
        };
        if !m.has(MonsterFlags::HELD) && m.has(MonsterFlags::RUN) {
            let orig_pos = m.pos;
            let wastarget = m.has(MonsterFlags::TARGET);
            if move_monst(id) == -1 {
                continue;
            }
            let m = Mon::get(id).unwrap_or(m);
            let hero = hero_pos();
            if m.has(MonsterFlags::FLY) && dist(hero.y, hero.x, m.pos.y, m.pos.x) >= 3 {
                move_monst(id);
            }
            let m = Mon::get(id).unwrap_or(m);
            if wastarget && !coord_eq(orig_pos, m.pos) {
                MONSTER_LIST.with_mut(id, |t| {
                    if let Thing::Monster { data } = t {
                        data.t_flags.remove(MonsterFlags::TARGET);
                    }
                });
                to_death = false as u8;
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
pub unsafe fn move_monst(id: MonsterId) -> i32 {
    let Some(m) = Mon::get(id) else {
        return 0;
    };
    if !m.has(MonsterFlags::SLOW) || m.turn {
        if do_chase(id) == -1 {
            return -1;
        }
    }
    let Some(m) = Mon::get(id) else {
        return 0;
    };
    if m.has(MonsterFlags::HASTE) {
        if do_chase(id) == -1 {
            return -1;
        }
    }
    MONSTER_LIST.with_mut(id, |t| {
        if let Thing::Monster { data } = t {
            data.t_turn = !data.t_turn;
        }
    });
    0
}

/// relocate:
/// Make the monster's new location be the specified one, updating
/// all the relevant state.
///
/// Uses globals: places (via moat), player, see_monst (function).
pub unsafe fn relocate(id: MonsterId, new_loc: IVec2) {
    let Some(mut m) = Mon::get(id) else {
        return;
    };
    if !coord_eq(new_loc, m.pos) {
        crate::draw::write_cell_glyph(m.pos, m.oldch as char);
        m.room = roomin(new_loc);
        set_oldch(id, new_loc);
        m.oldch = Mon::get(id).map(|x| x.oldch).unwrap_or(m.oldch);
        let oroom = m.room;
        crate::game::clear_monster(m.pos.y, m.pos.x);

        if oroom != m.room {
            update_dest(id);
            m.dest = Mon::get(id).map(|x| x.dest).unwrap_or(m.dest);
        }
        m.pos = new_loc;
        m.set(id);
        crate::game::set_monster_id(new_loc.y, new_loc.x, Some(id));
    }
    if see_monst(id) != false as u8 {
        crate::draw::write_cell_glyph(new_loc, crate::draw::monster_glyph(id));
    } else if player_has(MonsterFlags::SEEMONST) {
        crate::draw::write_reverse_video_cell_glyph(new_loc, crate::draw::monster_type_glyph(id));
    }
}

/// do_chase:
/// Make one thing chase another.
///
/// Uses globals: hero, proom, passages, places (via flat/chat/moat),
/// delta, running, count, quiet, to_death, kamikaze, lvl_obj.
pub unsafe fn do_chase(id: MonsterId) -> i32 {
    let Some(mut m) = Mon::get(id) else {
        return 0;
    };
    let mut mindist: i32 = 32767;
    let mut curdist: i32;
    let mut stoprun = false; // true means we are there
    let door: bool;

    let rer = m.room; // Find room of chaser
    if m.has(MonsterFlags::GREED) && crate::game::room_goldval(rer) == 0 {
        m.dest = DestRef::Hero; // If gold has been taken, run after hero
        m.set(id);
    }
    let ree = if m.dest == DestRef::Hero {
        // Find room of chasee
        crate::game::PLAYER.room()
    } else {
        roomin(resolve_dest(m.dest))
    };
    // We don't count doors as inside rooms for this routine
    door = crate::game::is_door_at(m.pos.y, m.pos.x);
    // If the object of our desire is in a different room,
    // and we are not in a corridor, run to the door nearest to
    // our goal.
    let mut loop_rer = rer;
    let mut loop_door = door;
    let loop_ree = ree;
    // The chosen destination for the chaser this turn (C's `static coord this`).
    let mut this = IVec2::ZERO;
    loop {
        if loop_rer != loop_ree {
            let dest = resolve_dest(m.dest);
            let exits = crate::game::room_exits(loop_rer);
            for cp in exits.iter() {
                curdist = dist(dest.y, dest.x, cp.y, cp.x);
                if curdist < mindist {
                    this = *cp;
                    mindist = curdist;
                }
            }
            if loop_door {
                let pnum = (flat_at(m.pos.y, m.pos.x) & F_PNUM) as usize;
                let passage_exits = crate::game::passage_exits(Some(pnum));
                for cp in passage_exits.iter() {
                    curdist = dist(dest.y, dest.x, cp.y, cp.x);
                    if curdist < mindist {
                        this = *cp;
                        mindist = curdist;
                    }
                }
                loop_rer = loop_ree;
                loop_door = false;
                continue;
            }
        } else {
            this = resolve_dest(m.dest);
            // For dragons check and see if (a) the hero is on a straight
            // line from it, and (b) that it is within shooting distance,
            // but outside of striking range.
            let hero = hero_pos();
            if m.typ == Some(MonsterType::Dragon)
                && (m.pos.y == hero.y
                    || m.pos.x == hero.x
                    || (m.pos.y - hero.y).abs() == (m.pos.x - hero.x).abs())
                && dist(m.pos.y, m.pos.x, hero.y, hero.x) <= BOLT_LENGTH * BOLT_LENGTH
                && !m.has(MonsterFlags::CANCELLED)
                && rnd(DRAGONSHOT) == 0
            {
                let dy = hero.y - m.pos.y;
                let dx = hero.x - m.pos.x;
                delta.y = sign(dy);
                delta.x = sign(dx);
                if has_hit != 0 {
                    endmsg();
                }
                fire_bolt(m.pos, delta, "flame");
                running = false as u8;
                count = 0;
                quiet = 0;
                if to_death != 0 && !m.has(MonsterFlags::TARGET) {
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
    let (keep_chasing, ch_ret) = chase(id, this);
    if keep_chasing == false as u8 {
        if coord_eq(this, hero_pos()) {
            return attack(id);
        } else if coord_eq(this, resolve_dest(m.dest)) {
            for obj_id in crate::game::item_ids() {
                if m.dest == DestRef::Object(obj_id) {
                    crate::game::with_current_level_mut(|level| level.remove_item(obj_id));
                    crate::entity::player::attach_pack_id(id, obj_id);
                    // Objects render from the level item list; the floor glyph
                    // under a picked-up object is then the terrain char.
                    update_dest(id);
                    m.dest = Mon::get(id).map(|x| x.dest).unwrap_or(m.dest);
                    break;
                }
            }
            if m.typ != Some(MonsterType::VenusFlytrap) {
                stoprun = true;
            }
        }
    } else if m.typ == Some(MonsterType::VenusFlytrap) {
        return 0;
    }
    relocate(id, ch_ret);
    // And stop running if need be
    let m = Mon::get(id).unwrap_or(m);
    if stoprun && coord_eq(m.pos, resolve_dest(m.dest)) {
        MONSTER_LIST.with_mut(id, |t| {
            if let Thing::Monster { data } = t {
                data.t_flags.remove(MonsterFlags::RUN);
            }
        });
    }
    0
}

/// set_oldch:
/// Set the oldch character for the monster
///
/// Uses globals: player, hero, see_floor, places (via chat).
pub unsafe fn set_oldch(id: MonsterId, cp: IVec2) {
    let Some(m) = Mon::get(id) else {
        return;
    };
    if coord_eq(m.pos, cp) {
        return;
    }

    let sch = m.oldch;
    let mut newch = crate::draw::screen_glyph_at(cp) as u8 & 0x7f;
    if !player_has(MonsterFlags::BLIND) {
        if (sch == FLOOR as u8 || newch == FLOOR as u8) && crate::game::room_dark(m.room) {
            newch = b' ';
        } else if dist(cp.y, cp.x, hero_pos().y, hero_pos().x) <= LAMPDIST && see_floor != 0 {
            newch = crate::draw::cell_glyph(cp.y, cp.x) as u8;
        }
    }
    MONSTER_LIST.with_mut(id, |t| {
        if let Thing::Monster { data } = t {
            data.t_oldch = newch;
        }
    });
}

/// see_monst:
/// Return true as u8 if the hero can see the monster
///
/// Uses globals: player, hero, proom, places (via chat).
pub unsafe fn see_monst(id: MonsterId) -> u8 {
    let Some(m) = Mon::get(id) else {
        return false as u8;
    };
    if player_has(MonsterFlags::BLIND) {
        return false as u8;
    }
    if m.has(MonsterFlags::INVIS) && !player_has(MonsterFlags::CANSEE) {
        return false as u8;
    }
    let y = m.pos.y;
    let x = m.pos.x;
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
    if m.room != crate::game::PLAYER.room() {
        return false as u8;
    }
    if crate::game::room_dark(m.room) {
        false as u8
    } else {
        true as u8
    }
}

/// runto:
/// Set a monster running after the hero.
///
/// Uses globals: places (via moat).
pub unsafe fn runto(runner: IVec2) {
    // If we couldn't find him, something is funny.
    // (C guarded this with `#ifdef MASTER`; always report in the Rust port.)
    let Some(id) = crate::game::monster_id_at(runner.y, runner.x) else {
        if MASTER {
            msg_str(&format!(
                "couldn't find monster in runto at ({},{})",
                runner.y, runner.x
            ));
        }
        return;
    };
    // Start the beastie running
    MONSTER_LIST.with_mut(id, |t| {
        if let Thing::Monster { data } = t {
            data.t_flags.insert(MonsterFlags::RUN);
            data.t_flags.remove(MonsterFlags::HELD);
        }
    });
    update_dest(id);
}

/// chase:
/// Find the spot for the chaser(er) to move closer to the
/// chasee(ee).  Returns `(keep_chasing, new_position)` where the first value is
/// true if we want to keep on chasing later and false if we reach the goal, and
/// the second is the coordinate the chaser should relocate to (C's `ch_ret`).
///
/// Uses globals: hero, lvl_obj, places (via moat/chat/winat).
pub unsafe fn chase(id: MonsterId, ee: IVec2) -> (u8, IVec2) {
    let Some(m) = Mon::get(id) else {
        return (false as u8, IVec2::ZERO);
    };
    let mut curdist: i32;
    let mut thisdist: i32;
    let mut er = m.pos;
    let mut plcnt = 1;
    // The new coordinate for the chaser (C's `static coord ch_ret`).
    let mut ch_ret = IVec2::ZERO;
    // The trial position being tested (C's `static coord tryp`).
    let mut tryp = IVec2::ZERO;

    // If the thing is confused, let it move randomly. Invisible
    // Stalkers are slightly confused all of the time, and bats are
    // quite confused all the time.
    if (m.has(MonsterFlags::HUH) && rnd(5) != 0)
        || (m.typ == Some(MonsterType::Phantom) && rnd(5) == 0)
        || (m.typ == Some(MonsterType::Bat) && rnd(2) == 0)
    {
        // get a valid random move
        ch_ret = rndmove(m.pos);
        curdist = dist_cp(ch_ret, ee);
        // Small chance that it will become un-confused
        if rnd(20) == 0 {
            MONSTER_LIST.with_mut(id, |t| {
                if let Thing::Monster { data } = t {
                    data.t_flags.remove(MonsterFlags::HUH);
                }
            });
        }
    }
    // Otherwise, find the empty spot next to the chaser that is
    // closest to the chasee.
    else {
        // This will eventually hold where we move to get closer.
        // If we can't find an empty spot, we stay where we are.
        curdist = dist_cp(er, ee);
        ch_ret = er;

        let mut ey = er.y + 1;
        if ey >= GameConfig::SCREEN_LINES - 1 {
            ey = GameConfig::SCREEN_LINES - 2;
        }
        let mut ex = er.x + 1;
        if ex >= GameConfig::SCREEN_COLS {
            ex = GameConfig::SCREEN_COLS - 1;
        }

        let mut x = er.x - 1;
        while x <= ex {
            if x >= 0 {
                tryp.x = x;
                let mut y = er.y - 1;
                while y <= ey {
                    tryp.y = y;
                    if diag_ok(er, tryp) == false as u8 {
                        y += 1;
                        continue;
                    }
                    if crate::game::cell_is_walkable(y, x) {
                        // If it is a scroll, it might be a scare monster scroll
                        // so we need to look it up to see what type it is.
                        let mut found_scare = false;
                        for obj_id in crate::game::item_ids() {
                            let at_pos = crate::item::arena::with_object(obj_id, |data| {
                                if data.o_pos.y == y && data.o_pos.x == x {
                                    Some(data.o_type)
                                } else {
                                    None
                                }
                            })
                            .flatten();
                            if let Some(otype) = at_pos {
                                found_scare = matches!(otype, ItemType::Scroll(ScrollType::Scare));
                                break;
                            }
                        }
                        if found_scare {
                            y += 1;
                            continue;
                        }
                        // It can also be a Xeroc, which we shouldn't step on.
                        let xeroc = crate::game::monster_id_at(y, x).is_some_and(|mid| {
                            MONSTER_LIST
                                .with(mid, |t| match t {
                                    Thing::Monster { data } => {
                                        data.t_type == Some(MonsterType::Xeroc)
                                    }
                                    Thing::Object { .. } => false,
                                })
                                .unwrap_or(false)
                        });
                        if xeroc {
                            y += 1;
                            continue;
                        }
                        // If we didn't find any scrolls at this place or it
                        // wasn't a scare scroll, then this place counts.
                        thisdist = dist(y, x, ee.y, ee.x);
                        if thisdist < curdist {
                            plcnt = 1;
                            ch_ret = tryp;
                            curdist = thisdist;
                        } else if thisdist == curdist && rnd(plcnt + 1) == 0 {
                            // C's rnd(++plcnt) bumps plcnt then draws in [0, plcnt).
                            plcnt += 1;
                            ch_ret = tryp;
                            curdist = thisdist;
                        }
                    }
                    y += 1;
                }
            }
            x += 1;
        }
    }
    if curdist != 0 && !coord_eq(ch_ret, hero_pos()) {
        (true as u8, ch_ret)
    } else {
        (false as u8, ch_ret)
    }
}

/// roomin:
/// Find what room some coordinates are in. Passages outside rooms return
/// `None` without reporting an invalid location.
///
/// Uses globals: places (via flat), passages, rooms, msg.
pub unsafe fn roomin(cp: IVec2) -> Option<usize> {
    let (room, is_passage) = crate::game::with_current_level(|level| {
        (
            level.room_at(cp.y, cp.x),
            level.tile_at(cp.y as usize, cp.x as usize) == crate::tile::Tile::Passage,
        )
    });
    if room.is_some() || is_passage {
        return room;
    }

    msg_str(&format!("in some bizarre place ({}, {})", cp.y, cp.x));
    if MASTER {
        std::process::abort();
    }
    None
}

/// diag_ok:
/// Check to see if the move is legal if it is diagonal
///
/// Uses globals: places (via chat).
pub unsafe fn diag_ok(sp: IVec2, ep: IVec2) -> u8 {
    if ep.x < 0
        || ep.x >= GameConfig::SCREEN_COLS
        || ep.y <= 0
        || ep.y >= GameConfig::SCREEN_LINES - 1
    {
        return false as u8;
    }
    if ep.x == sp.x || ep.y == sp.y {
        return true as u8;
    }
    if crate::game::tile_at(ep.y, sp.x).is_walkable()
        && crate::game::tile_at(sp.y, ep.x).is_walkable()
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
    let tp = IVec2 { x, y };
    let rer = roomin(tp);
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
pub unsafe fn update_dest(id: MonsterId) {
    let Some(m) = Mon::get(id) else {
        return;
    };
    let prob = monsters[m.typ.map_or(0, |k| k.index())].m_carry;
    if prob <= 0 || m.room == crate::game::PLAYER.room() || see_monst(id) != false as u8 {
        crate::entity::player::set_monster_dest(id, DestRef::Hero);
        return;
    }
    for obj_id in crate::game::item_ids() {
        let info = crate::item::arena::with_object(obj_id, |data| (data.o_type, data.o_pos));
        let Some((otype, opos)) = info else {
            continue;
        };
        if matches!(otype, ItemType::Scroll(ScrollType::Scare)) {
            continue;
        }
        if roomin(opos) == m.room && rnd(100) < prob {
            let obj_dest = DestRef::Object(obj_id);
            let mut taken = false;
            for mid in MONSTER_LIST.ids() {
                if crate::entity::player::monster_dest(mid) == obj_dest {
                    taken = true;
                    break;
                }
            }
            if !taken {
                crate::entity::player::set_monster_dest(id, obj_dest);
                return;
            }
        }
    }
    crate::entity::player::set_monster_dest(id, DestRef::Hero);
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
pub unsafe fn dist_cp(c1: IVec2, c2: IVec2) -> i32 {
    dist(c1.y, c1.x, c2.y, c2.x)
}
