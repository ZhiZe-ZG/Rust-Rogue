//! Populating a generated level: gold, monsters, objects, traps, stairs, and
//! the hero spawn.
//!
//! Room selection, geometry, and candidate-cell validation go through the Rust
//! `Level` model (`rnd_room`, `rnd_pos`) and [`Dungeon::find_floor`], plus the
//! safe per-cell monster occupancy grid. Items are addressed by arena
//! [`ThingId`] handles and monsters by [`MonsterId`] handles. This replaces the
//! legacy process-wide `static mut` game globals (`amulet`, `ntraps`,
//! `seenstairs`) with counters on the [`Dungeon`] itself; only the item/monster
//! stores remain externally owned.

use glam::IVec2;

use crate::config::GameConfig;
use crate::entity::chase::roomin;
use crate::entity::monsters::{give_pack_id, new_monster_id, randmonster};
use crate::entity::player::{MonsterFlags, ObjectFlags, Thing};
use crate::item::arena::{new_item_id, ThingId, OBJECTS};
use crate::item::item_type::ItemType;
use crate::item::things::new_thing_id;
use crate::level::LevelFlags;
use crate::rnd::rnd;
use crate::tile::{Tile, TrapType};

use super::Dungeon;

// -- Glyphs --
const GOLDGRP: i32 = 1;

impl Dungeon {
    /// Link an already-allocated floor object into the level's item list.
    fn attach_floor(&self, id: ThingId) {
        self.with_level_mut(|current| current.add_item(id));
    }

    /// Allocate a floor object at `pos` and link it into the level's item list.
    unsafe fn spawn_object_at(&self, pos: IVec2) -> ThingId {
        let id = new_thing_id();
        OBJECTS.with_object_mut(id, |o| o.o_pos = pos);
        self.attach_floor(id);
        id
    }

    /// Fill one treasure room with `MIN..MAX` objects and monsters.
    unsafe fn treas_room(&self) {
        let idx = self.with_level(|current| current.rnd_room());
        let mut spots = self.with_level(|current| {
            let room = &current.rooms[idx];
            (room.size.y - 2) * (room.size.x - 2) - GameConfig::MIN_TREASURES
        });

        if spots > (GameConfig::MAX_TREASURES - GameConfig::MIN_TREASURES) {
            spots = GameConfig::MAX_TREASURES - GameConfig::MIN_TREASURES;
        }

        let mut nm = rnd(spots) + GameConfig::MIN_TREASURES;
        let num_monst = nm;
        while nm > 0 {
            if let Some(pos) = self.find_floor(Some(idx), 2 * GameConfig::MAX_PLACEMENT_ATTEMPTS, false)
            {
                self.spawn_object_at(pos);
            }
            nm -= 1;
        }

        nm = rnd(spots) + GameConfig::MIN_TREASURES;
        if nm < num_monst + 2 {
            nm = num_monst + 2;
        }
        spots = self.with_level(|current| {
            let room = &current.rooms[idx];
            (room.size.y - 2) * (room.size.x - 2)
        });
        if nm > spots {
            nm = spots;
        }

        let depth = self.current_depth();
        self.set_current_depth(depth + 1);
        while nm > 0 {
            if let Some(pos) = self.find_floor(Some(idx), GameConfig::MAX_PLACEMENT_ATTEMPTS, true) {
                let id = self.monster_list.spawn_actor();
                new_monster_id(id, randmonster(false), pos);
                self.monster_list.with_mut(id, |t| {
                    if let Thing::Monster { data } = t {
                        data.t_flags.insert(MonsterFlags::MEAN);
                    }
                });
                give_pack_id(id);
            }
            nm -= 1;
        }
        self.set_current_depth(depth);
    }

    /// Scatter gold and monsters through every active room.
    ///
    /// Each room may hold a gold stash (value `rnd(50 + 10*level) + 2`) and has
    /// a chance of a monster guarding it (higher when the room has gold).
    unsafe fn place_room_contents(&self) {
        let level = self.current_depth();
        let max_level = self.max_depth();
        let has_amulet = self.has_amulet();

        for i in 0..GameConfig::MAX_ROOMS {
            let gone = self.with_level(|current| current.rooms[i].gone);
            if gone {
                continue;
            }

            if rnd(2) == 0 && (!has_amulet || level >= max_level) {
                let gold = new_item_id();
                let goldval = rnd(50 + 10 * level) + 2;
                let gold_pos = self.find_floor(Some(i), 0, false).unwrap_or(IVec2::ZERO);
                OBJECTS.with_object_mut(gold, |og| {
                    og.o_arm = goldval;
                    og.o_pos = gold_pos;
                    og.o_flags = ObjectFlags::MANY;
                    og.o_group = GOLDGRP;
                    og.o_type = ItemType::Gold;
                });
                self.with_level_mut(|current| {
                    current.rooms[i].gold = gold_pos;
                    current.rooms[i].goldval = goldval;
                });
                self.attach_floor(gold);
            }

            let goldval = self.with_level(|current| current.rooms[i].goldval);
            if rnd(100) < if goldval > 0 { 80 } else { 25 } {
                let id = self.monster_list.spawn_actor();
                if let Some(pos) = self.find_floor(Some(i), 0, true) {
                    new_monster_id(id, randmonster(false), pos);
                    give_pack_id(id);
                }
            }
        }
    }

    /// Put potions and scrolls (and, deep enough, the Amulet of Yendor) on this
    /// level.
    unsafe fn put_things(&self) {
        let level = self.current_depth();

        // Once you have found the amulet, the only way to get new stuff is
        // go down into the dungeon.
        if self.has_amulet() && level < self.max_depth() {
            return;
        }

        // Check for treasure rooms, and if so, put it in.
        if rnd(GameConfig::TREASURE_ROOM_CHANCE) == 0 {
            self.treas_room();
        }

        // Do MAXOBJ attempts to put things on a level.
        for _ in 0..GameConfig::MAX_OBJECTS {
            if rnd(100) < 36 {
                // Pick a new object and link it in the list.
                if let Some(pos) = self.find_floor(None, 0, false) {
                    self.spawn_object_at(pos);
                }
            }
        }

        // If he is really deep in the dungeon and he hasn't found the amulet
        // yet, put it somewhere on the ground.
        if level >= GameConfig::AMULET_LEVEL && !self.has_amulet() {
            if let Some(pos) = self.find_floor(None, 0, false) {
                let obj = new_item_id();
                OBJECTS.with_object_mut(obj, |og| {
                    og.o_hplus = 0;
                    og.o_dplus = 0;
                    og.o_damage = [b'0', b'x', b'0', 0, 0, 0, 0, 0];
                    og.o_hurldmg = [b'0', b'x', b'0', 0, 0, 0, 0, 0];
                    og.o_arm = 11;
                    og.o_type = ItemType::Amulet;
                    og.o_pos = pos;
                });
                self.attach_floor(obj);
            }
        }
    }

    /// Scatter traps (scaled by depth) on floor cells.
    unsafe fn place_traps(&self) {
        let level = self.current_depth();

        if rnd(10) >= level {
            return;
        }

        let mut ntraps = rnd(level / 4) + 1;
        if ntraps > GameConfig::MAX_TRAPS {
            ntraps = GameConfig::MAX_TRAPS;
        }
        self.set_ntraps(ntraps);

        let mut i = ntraps;
        while i > 0 {
            let stairs = loop {
                match self.find_floor(None, 0, false) {
                    Some(pos) => {
                        if self.with_level(|current| {
                            current.tile_at(pos.y as usize, pos.x as usize)
                        }) == Tile::Floor
                        {
                            break pos;
                        }
                    }
                    None => break IVec2::ZERO,
                }
            };

            self.with_level_mut(|current| {
                let idx = LevelFlags::flag_idx(stairs.y as usize, stairs.x as usize);
                let trap = TrapType::from_raw(rnd(GameConfig::TRAP_KIND_COUNT) as u8);
                current.map.set(stairs, Tile::Trap(trap));
                current.flags.real[idx] = false;
            });
            i -= 1;
        }
    }

    /// Place the down staircase on a floor cell.
    unsafe fn place_stairs(&self) {
        let stairs = self.find_floor(None, 0, false).unwrap_or(IVec2::ZERO);
        // The staircase is a tile in the level map; it renders `%` via draw.
        self.with_level_mut(|current| {
            current.stairs = stairs;
            current.map.set(stairs, Tile::Stairs);
        });
        self.set_seen_stairs(false);
    }

    /// Link every monster on the level to the room its position falls in.
    unsafe fn link_monsters_to_rooms(&self) {
        let ids = self.monster_list.ids();
        for id in ids {
            self.monster_list.with_mut(id, |t| {
                if let Thing::Monster { data } = t {
                    data.t_room = roomin(data.t_pos);
                }
            });
        }
    }

    /// Place the hero on an open floor cell.
    fn place_hero(&self) {
        if let Some(pos) = self.find_floor(None, 0, true) {
            crate::game::PLAYER.set_pos(pos);
        }
    }

    /// Run the full population pass: gold/monsters, objects, traps, stairs, and
    /// the hero.
    ///
    /// Called by [`Dungeon::new_level`] after the map is generated.
    pub(crate) unsafe fn populate_level(&self) {
        self.place_room_contents();
        self.put_things(); /* Place objects (if any) */
        self.place_traps();
        self.place_stairs();
        self.link_monsters_to_rooms();
        self.place_hero();
    }
}
