//! Dungeon-level lifecycle orchestration.
//!
//! [`Dungeon::new_level`] resets the current level, generates its rooms and
//! passages, and populates it. The UI render daemon observes the updated current
//! level and redraws it. [`Dungeon::door_open`] wakes monsters in a room when it
//! becomes visible.

use glam::IVec2;

use crate::config::GameConfig;
use crate::draw::winat;
use crate::entity::monsters::wake_monster;
use crate::entity::player::MonsterFlags;
use crate::game;
use crate::structure::Room;

use super::Dungeon;

impl Dungeon {
    /// Reset the live level in preparation for a fresh generation pass.
    unsafe fn reset_level(&self) {
        let depth = self.with_level_mut(|current| current.reset_for_new_level());

        game::player_remove_flag(MonsterFlags::HELD);
        self.record_max_depth(depth);

        self.monster_map.clear();
    }

    /// Free the monsters and floor items of the previous level.
    unsafe fn clear_previous_level_items(&self) {
        for id in self.monster_list.ids() {
            crate::entity::player::free_pack_id(id);
        }
        self.monster_list.clear();
        self.with_level_mut(|current| current.clear_items());
    }

    /// Wake monsters in a room when it becomes visible.
    pub unsafe fn door_open(&self, room: Option<usize>) {
        if game::room_gone(room) {
            return;
        }

        let (pos, size) = match game::room_bounds(room) {
            Some(b) => b,
            None => return,
        };
        let y_end = pos.y + size.y;
        let x_end = pos.x + size.x;
        for y in pos.y..y_end {
            for x in pos.x..x_end {
                if winat(y, x).is_ascii_uppercase() {
                    wake_monster(y, x);
                }
            }
        }
    }

    /// Build and populate a fresh dungeon level at the current depth.
    pub unsafe fn new_level(&self) {
        self.reset_level();
        self.clear_previous_level_items();

        // Lay out and dig the rooms and passages for this level. A single
        // mutable borrow covers both passes (see `rebuild_rooms_and_passages`).
        let rooms = std::array::from_fn(|_| Room::new(IVec2::ZERO, IVec2::ZERO));
        let room_size = IVec2::new(GameConfig::SCREEN_COLS / 3, GameConfig::SCREEN_LINES / 3);
        self.with_level_mut(|current| {
            let _ = current.rebuild_rooms_and_passages(rooms, room_size);
        });

        self.bump_no_food();
        self.populate_level();
    }
}