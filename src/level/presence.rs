//! Populating a generated level: gold, monsters, objects, traps, stairs, and
//! the hero spawn.
//!
//! Room selection, geometry, and candidate-cell validation go through the Rust
//! `Level` model (`Level::rnd_room`/`Level::rnd_pos` through scoped level
//! access) and the safe per-cell monster occupancy grid, so [`find_floor`] is
//! entirely safe. The remaining unsafe comes only from the raw `*mut Thing`
//! allocation handles produced by the item/monster stores, which
//! [`super::generation::new_level`] triggers after the rooms/passages have been
//! dug.

use glam::IVec2;

use crate::config::GameConfig;
use crate::daemons::visuals;
use crate::draw::enter_room;
use crate::entity::chase::roomin;
use crate::entity::monsters::{give_pack, new_monster, randmonster};
use crate::entity::player::{MonsterFlags, ObjectFlags, Thing, ThingMonster, ThingObject};
use crate::game::MONSTER_LIST;
use crate::game::{self, with_current_level, with_current_level_mut};
use crate::globals::{amulet, max_level, ntraps, seenstairs};
use crate::item::potions::turn_see;
use crate::item::thing_list::{new_actor, new_item};
use crate::item::things::new_thing;
use crate::rnd::rnd;
use crate::ui::output;

use super::level::LevelFlags;
use crate::tile::{Tile, TrapType};

// -- Glyphs --
const AMULET: u8 = b',';
const GOLD: u8 = b'*';
const PLAYER: u8 = b'@';

const GOLDGRP: i32 = 1;

/// Interpret `tp` as an object (`ThingObject`).
#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

/// Interpret `tp` as a monster (`ThingMonster`).
#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

/// Find a floor cell to place something, optionally avoiding monsters.
///
/// If `room_idx` is `None` a random room slot is tried each iteration via
/// `Level::rnd_room`; otherwise the cell is chosen inside that room. Room
/// selection and geometry come from the Rust `Level` model (`Level::rnd_pos`),
/// while the candidate cell is validated against the level tile map and the
/// per-cell monster occupancy grid. Returns the chosen cell, or `None` when
/// `limit` (if nonzero) attempts are exhausted.
pub(crate) fn find_floor(room_idx: Option<usize>, limit: i32, monst: bool) -> Option<IVec2> {
    let mut cnt = limit;
    // Safety bound: unlimited scans must eventually give up rather than hang
    // level generation on a packed level.
    let mut guard = 0u32;
    loop {
        if limit != 0 {
            if cnt == 0 {
                return None;
            }
            cnt -= 1;
        }
        guard += 1;
        if guard > 1_000_000 {
            return None;
        }

        let (expected_tile, pos) = with_current_level(|current| {
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

        // `find_floor` validates the map tile directly; an object overlay does
        // not count as a free floor cell.
        let tile = with_current_level(|current| current.tile_at(pos.y as usize, pos.x as usize));

        if monst {
            let occupied = game::MONSTER_MAP
                .at(pos.y as usize, pos.x as usize)
                .is_some();
            if !occupied && tile.is_walkable() {
                return Some(pos);
            }
        } else if tile == expected_tile {
            return Some(pos);
        }
    }
}

/// Allocate a floor object at `pos` and link it into the level's item list.
unsafe fn spawn_object_at(pos: IVec2) -> *mut Thing {
    let obj = new_thing();
    (*thing_o(obj)).o_pos = pos;
    with_current_level_mut(|current| current.items.attach(obj));
    obj
}

/// Fill one treasure room with `MIN..MAX` objects and monsters.
unsafe fn treas_room() {
    let idx = with_current_level(|current| current.rnd_room());
    let mut spots = with_current_level(|current| {
        let room = &current.rooms[idx];
        (room.size.y - 2) * (room.size.x - 2) - GameConfig::MIN_TREASURES
    });

    if spots > (GameConfig::MAX_TREASURES - GameConfig::MIN_TREASURES) {
        spots = GameConfig::MAX_TREASURES - GameConfig::MIN_TREASURES;
    }

    let mut nm = rnd(spots) + GameConfig::MIN_TREASURES;
    let num_monst = nm;
    while nm > 0 {
        if let Some(pos) = find_floor(Some(idx), 2 * GameConfig::MAX_PLACEMENT_ATTEMPTS, false) {
            spawn_object_at(pos);
        }
        nm -= 1;
    }

    nm = rnd(spots) + GameConfig::MIN_TREASURES;
    if nm < num_monst + 2 {
        nm = num_monst + 2;
    }
    spots = with_current_level(|current| {
        let room = &current.rooms[idx];
        (room.size.y - 2) * (room.size.x - 2)
    });
    if nm > spots {
        nm = spots;
    }

    let depth = game::current_depth();
    game::set_current_depth(depth + 1);
    while nm > 0 {
        if let Some(mut pos) = find_floor(Some(idx), GameConfig::MAX_PLACEMENT_ATTEMPTS, true) {
            let tp = new_actor();
            new_monster(tp, randmonster(false), &mut pos);
            (*thing_t(tp)).t_flags.insert(MonsterFlags::MEAN);
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
    let level = game::current_depth();

    for i in 0..GameConfig::MAX_ROOMS {
        let gone = with_current_level(|current| current.rooms[i].gone);
        if gone {
            continue;
        }

        if rnd(2) == 0 && (!amulet || level >= max_level) {
            let gold = new_item();

            if !gold.is_null() {
                let og = thing_o(gold);

                (*og).o_arm = rnd(50 + 10 * level) + 2;
                let gold_pos = find_floor(Some(i), 0, false).unwrap_or(IVec2::ZERO);
                with_current_level_mut(|current| {
                    current.rooms[i].gold = gold_pos;
                    current.rooms[i].goldval = (*og).o_arm;
                });
                (*og).o_pos = gold_pos;
                (*og).o_flags = ObjectFlags::MANY;
                (*og).o_group = GOLDGRP;
                (*og).o_type = GOLD as i32;
                with_current_level_mut(|current| current.items.attach(gold));
            }
        }

        let goldval = with_current_level(|current| current.rooms[i].goldval);
        if rnd(100) < if goldval > 0 { 80 } else { 25 } {
            let tp = new_actor();
            if !tp.is_null() {
                if let Some(mut pos) = find_floor(Some(i), 0, true) {
                    new_monster(tp, randmonster(false), &mut pos);
                    give_pack(tp);
                }
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
            if let Some(pos) = find_floor(None, 0, false) {
                spawn_object_at(pos);
            }
        }
    }

    // If he is really deep in the dungeon and he hasn't found the amulet
    // yet, put it somewhere on the ground.
    if level >= GameConfig::AMULET_LEVEL && !amulet {
        if let Some(pos) = find_floor(None, 0, false) {
            let obj = new_item();
            let og = thing_o(obj);
            (*og).o_hplus = 0;
            (*og).o_dplus = 0;
            (*og).o_damage = [b'0', b'x', b'0', 0, 0, 0, 0, 0];
            (*og).o_hurldmg = [b'0', b'x', b'0', 0, 0, 0, 0, 0];
            (*og).o_arm = 11;
            (*og).o_type = AMULET as i32;
            (*og).o_pos = pos;
            with_current_level_mut(|current| current.items.attach(obj));
        }
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
    while i > 0 {
        let stairs = loop {
            match find_floor(None, 0, false) {
                Some(pos) => {
                    if with_current_level(|current| current.tile_at(pos.y as usize, pos.x as usize))
                        == Tile::Floor
                    {
                        break pos;
                    }
                }
                None => break IVec2::ZERO,
            }
        };

        with_current_level_mut(|current| {
            let idx = LevelFlags::flag_idx(stairs.y as usize, stairs.x as usize);
            let trap = TrapType::from_raw(rnd(GameConfig::TRAP_KIND_COUNT) as u8);
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
    let stairs = find_floor(None, 0, false).unwrap_or(IVec2::ZERO);
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
    for id in MONSTER_LIST.ids() {
        if let Some(tp) = MONSTER_LIST.handle(id) {
            let t = thing_t(tp);
            (*t).t_room = roomin(&raw mut (*t).t_pos);
        }
    }
}

/// Place the hero on an open floor cell and finalize the screen.
unsafe fn place_hero() {
    if let Some(pos) = find_floor(None, 0, true) {
        crate::game::PLAYER.set_pos(pos);
    }

    let mut hero_pos = crate::game::PLAYER.pos();
    enter_room(&raw mut hero_pos);
    let hero_pos = crate::game::PLAYER.pos();
    output::write_glyph_at(IVec2::new(hero_pos.x, hero_pos.y), PLAYER as char);
    if crate::game::PLAYER.has_flag(MonsterFlags::SEEMONST) {
        turn_see(false as u8);
    }
    if crate::game::PLAYER.has_flag(MonsterFlags::HALU) {
        visuals();
    }
}

/// Run the full population pass: gold/monsters, objects, traps, stairs, and
/// the hero.
///
/// Called by [`super::generation::new_level`] after the map is generated.
pub(crate) unsafe fn populate_level() {
    place_room_contents();
    put_things(); /* Place objects (if any) */
    place_traps();
    place_stairs();
    link_monsters_to_rooms();
    place_hero();
}
