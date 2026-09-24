//! Populating a generated level: gold, monsters, objects, traps, stairs, and
//! the hero spawn.
//!
//! Room selection and geometry go through the Rust `Level` model
//! (`Level::rnd_room`/`Level::rnd_pos` through scoped level access); the
//! remaining C `places`/`player` globals are touched via the raw `extern` C
//! symbols declared at the top of this module. [`super::generation::new_level`]
//! calls these after the rooms/passages have been dug and mirrored.

use glam::IVec2;

use crate::config::GameConfig;
use crate::daemons::visuals;
use crate::draw::enter_room;
use crate::entity::chase::roomin;
use crate::entity::monster_list::MLIST;
use crate::entity::monsters::{give_pack, new_monster, randmonster};
use crate::entity::player::{CThing, CThingMonster, CThingObject};
use crate::game;
use crate::globals::{amulet, max_level, ntraps, player, seenstairs};
use crate::item::potions::turn_see;
use crate::item::thing_list::{new_actor, new_item};
use crate::item::things::new_thing;
use crate::rnd::rnd;
use crate::ui::output;

use super::level::{with_current_level_mut, LevelFlags};
use super::tile::{Tile, Trap};

// -- Object/thing flags --
const ISMANY: i32 = 0o0000010;
const ISMEAN: i16 = 0o0004000;
const SEEMONST: i16 = 0o040000;
const ISHALU: i16 = 0o0004000;

// -- Glyphs --
const AMULET: u8 = b',';
const GOLD: u8 = b'*';
const PLAYER: u8 = b'@';

const GOLDGRP: i32 = 1;

/// Interpret `tp` as an object (`CThingObject`).
#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    crate::entity::player::thing_o(tp)
}

/// Interpret `tp` as a monster (`CThingMonster`).
#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    crate::entity::player::thing_t(tp)
}

/// Find a floor cell to place something, optionally avoiding monsters.
///
/// If `room_idx` is `None` a random room slot is tried each iteration via
/// `Level::rnd_room`; otherwise the cell is chosen inside that room. Room
/// selection and geometry come from the Rust `Level` model (`Level::rnd_pos`),
/// while the candidate cell is validated against the C `places` grid. Returns
/// `true` and stores the chosen cell into `cp` on success; `false` when
/// `limit` (if nonzero) attempts are exhausted.
pub unsafe fn find_floor(
    room_idx: Option<usize>,
    cp: *mut IVec2,
    limit: i32,
    monst: bool,
) -> bool {
    if cp.is_null() {
        return false;
    }

    let mut cnt = limit;
    let mut guard = 0u32;
    loop {
        if limit != 0 {
            if cnt == 0 {
                return false;
            }
            cnt -= 1;
        }
        // Safety bound: unlimited scans must eventually give up rather than
        // hang level generation on a packed level.
        guard += 1;
        if guard > 1_000_000 {
            return false;
        }

        let (expected_tile, pos) = with_current_level_mut(|current| {
            let idx = match room_idx {
                Some(idx) => idx,
                None => current.rnd_room(),
            };
            let room = &current.rooms[idx];
            let expected_tile = if room.is_maze() {
                Tile::Passage
            } else {
                Tile::Floor
            };
            (expected_tile, current.rnd_pos(room))
        });

        (*cp).x = pos.x;
        (*cp).y = pos.y;

        // `find_floor` validates the map tile directly; an object overlay does
        // not count as a free floor cell.
        let tile = game::tile_at((*cp).y, (*cp).x);

        if monst {
            if game::monster_at((*cp).y, (*cp).x).is_null() && tile.is_walkable() {
                return true;
            }
        } else if tile == expected_tile {
            return true;
        }
    }
}

/// Fill one treasure room with `MIN..MAX` objects and monsters.
unsafe fn treas_room() {
    let mut mp = IVec2::ZERO;
    let (idx, mut spots) = with_current_level_mut(|current| {
        let idx = current.rnd_room();
        let room = &current.rooms[idx];
        let spots = (room.size.y - 2) * (room.size.x - 2) - GameConfig::MIN_TREASURES;
        (idx, spots)
    });

    if spots > (GameConfig::MAX_TREASURES - GameConfig::MIN_TREASURES) {
        spots = GameConfig::MAX_TREASURES - GameConfig::MIN_TREASURES;
    }

    let mut nm = rnd(spots) + GameConfig::MIN_TREASURES;
    let num_monst = nm;
    while nm > 0 {
        find_floor(
            Some(idx),
            &mut mp,
            2 * GameConfig::MAX_PLACEMENT_ATTEMPTS,
            false,
        );
        let tp = new_thing();
        (*thing_o(tp)).o_pos = mp;
        // Objects render from the `lvl_obj` list; no glyph write needed.
        with_current_level_mut(|current| current.items.attach(tp));
        nm -= 1;
    }

    nm = rnd(spots) + GameConfig::MIN_TREASURES;
    if nm < num_monst + 2 {
        nm = num_monst + 2;
    }
    spots = with_current_level_mut(|current| {
        let room = &current.rooms[idx];
        (room.size.y - 2) * (room.size.x - 2)
    });
    if nm > spots {
        nm = spots;
    }

    let depth = game::current_depth();
    game::set_current_depth(depth + 1);
    while nm > 0 {
        if find_floor(Some(idx), &mut mp, GameConfig::MAX_PLACEMENT_ATTEMPTS, true) {
            let tp = new_actor();
            new_monster(tp, randmonster(false), &mut mp);
            (*thing_t(tp)).t_flags |= ISMEAN;
            give_pack(tp);
        }
        nm -= 1;
    }
    game::set_current_depth(depth);
}

/// Scatter gold and monsters through every active room.
///
/// Each room may hold a gold stash (value `rnd(50 + 10*level) + 2`) and has a
/// chance of a monster guarding it (higher when the room has gold).
unsafe fn place_room_contents() {
    let mut mp = IVec2::ZERO;
    let level = game::current_depth();

    for i in 0..GameConfig::MAX_ROOMS {
        let gone = with_current_level_mut(|current| current.rooms[i].gone);
        if gone {
            continue;
        }

        if rnd(2) == 0 && (!amulet || level >= max_level) {
            let gold = new_item();

            if !gold.is_null() {
                let og = thing_o(gold);

                (*og).o_arm = rnd(50 + 10 * level) + 2;
                let mut gold_pos = IVec2::ZERO;
                find_floor(Some(i), &mut gold_pos, 0, false);
                with_current_level_mut(|current| {
                    current.rooms[i].gold = gold_pos;
                    current.rooms[i].goldval = (*og).o_arm;
                });
                (*og).o_pos = gold_pos;
                (*og).o_flags = ISMANY;
                (*og).o_group = GOLDGRP;
                (*og).o_type = GOLD as i32;
                with_current_level_mut(|current| current.items.attach(gold));
            }
        }

        let goldval = with_current_level_mut(|current| current.rooms[i].goldval);
        if rnd(100) < if goldval > 0 { 80 } else { 25 } {
            let tp = new_actor();
            if !tp.is_null() {
                find_floor(Some(i), &mut mp, 0, true);
                new_monster(tp, randmonster(false), &mut mp);
                give_pack(tp);
            }
        }
    }
}

/// Put potions and scrolls (and, deep enough, the Amulet of Yendor) on this level.
unsafe fn put_things() {
    let level = game::current_depth();

    // Once you have found the amulet, the only way to get new stuff is
    // go down into the dungeon.
    if amulet && level < max_level {
        return;
    }

    // Check for treasure rooms, and if so, put it in.
    if rnd(GameConfig::TREASURE_ROOM_CHANCE) == 0 {
        treas_room();
    }

    // Do MAXOBJ attempts to put things on a level.
    for _ in 0..GameConfig::MAX_OBJECTS {
        if rnd(100) < 36 {
            // Pick a new object and link it in the list.
            let obj = new_thing();
            with_current_level_mut(|current| current.items.attach(obj));
            // Put it somewhere.
            let og = thing_o(obj);
            let pos = &raw mut (*og).o_pos;
            find_floor(None, pos, 0, false);
        }
    }

    // If he is really deep in the dungeon and he hasn't found the amulet
    // yet, put it somewhere on the ground.
    if level >= GameConfig::AMULET_LEVEL && !amulet {
        let obj = new_item();
        with_current_level_mut(|current| current.items.attach(obj));
        let og = thing_o(obj);
        (*og).o_hplus = 0;
        (*og).o_dplus = 0;
        (*og).o_damage = [
            b'0',
            b'x',
            b'0',
            0,
            0,
            0,
            0,
            0,
        ];
        (*og).o_hurldmg = [
            b'0',
            b'x',
            b'0',
            0,
            0,
            0,
            0,
            0,
        ];
        (*og).o_arm = 11;
        (*og).o_type = AMULET as i32;
        let pos = &raw mut (*og).o_pos;
        find_floor(None, pos, 0, false);
    }
}

/// Scatter traps (scaled by depth) on floor cells.
unsafe fn place_traps() {
    let level = game::current_depth();

    if rnd(10) >= level {
        return;
    }

    ntraps = rnd(level / 4) + 1;
    if ntraps > GameConfig::MAX_TRAPS {
        ntraps = GameConfig::MAX_TRAPS;
    }

    let mut i = ntraps;
    let mut stairs = IVec2::ZERO;
    while i > 0 {
        loop {
            find_floor(None, &raw mut stairs, 0, false);
            if game::tile_at(stairs.y, stairs.x) == Tile::Floor {
                break;
            }
        }

        with_current_level_mut(|current| {
            let idx = LevelFlags::flag_idx(stairs.y as usize, stairs.x as usize);
            let trap = Trap::from_raw(rnd(GameConfig::TRAP_KIND_COUNT) as u8);
            current
                .map
                .set(stairs.y as usize, stairs.x as usize, Tile::Trap(trap));
            current.flags.real[idx] = false;
        });
        i -= 1;
    }
}

/// Place the down staircase on a floor cell.
unsafe fn place_stairs() {
    let mut stairs = IVec2::ZERO;
    find_floor(None, &raw mut stairs, 0, false);
    // The staircase is a tile in the level map; it renders `%` via draw.
    with_current_level_mut(|current| {
        current.stairs = stairs;
        current
            .map
            .set(stairs.y as usize, stairs.x as usize, Tile::Stairs);
    });
    seenstairs = false as u8;
}

/// Link every monster on the level to the room its position falls in.
pub(crate) unsafe fn link_monsters_to_rooms() {
    let mut tp = MLIST.head();
    while !tp.is_null() {
        let t = thing_t(tp);
        (*t).t_room = roomin(&raw mut (*t).t_pos);
        tp = crate::entity::player::thing_next(tp);
    }
}

/// Place the hero on an open floor cell and finalize the screen.
unsafe fn place_hero() {
    find_floor(None, &raw mut (*thing_t(&raw mut player)).t_pos, 0, true);
    enter_room(&raw mut (*thing_t(&raw mut player)).t_pos);
    output::write_glyph_at(
            IVec2::new(
                (*thing_t(&raw mut player)).t_pos.x,
                (*thing_t(&raw mut player)).t_pos.y,
            ),
        PLAYER as char,
    );
    if ((*thing_t(&raw mut player)).t_flags & SEEMONST) != 0 {
        turn_see(false as u8);
    }
    if ((*thing_t(&raw mut player)).t_flags & ISHALU) != 0 {
        visuals();
    }
}

/// Run the full population pass: gold/monsters, objects, traps, stairs, and
/// the hero.
///
/// Called by [`super::generation::new_level`] after the map is generated and mirrored.
pub(crate) unsafe fn populate_level() {
    place_room_contents();
    put_things(); /* Place objects (if any) */
    place_traps();
    place_stairs();
    link_monsters_to_rooms();
    place_hero();
}
