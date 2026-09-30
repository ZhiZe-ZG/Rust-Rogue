//! Dungeon-level lifecycle orchestration.
//!
//! [`new_level`] resets the current level, generates its rooms and passages,
//! and populates it. The UI render daemon observes the updated current level
//! and redraws it. [`door_open`] wakes monsters in a room when it becomes
//! visible.

use glam::IVec2;

use crate::config::GameConfig;
use crate::draw::winat;
use crate::entity::monsters::wake_monster;
use crate::entity::player::MonsterFlags;
use crate::dungeon::DUNGEON;
use crate::game::{self, clear_level, with_current_level_mut};
use crate::dungeon::{bump_no_food, record_max_depth};
use crate::structure::Room;

use super::presence::populate_level;

unsafe fn reset_level() {
    let depth = with_current_level_mut(|current| current.reset_for_new_level());

    game::player_remove_flag(MonsterFlags::HELD);
    record_max_depth(depth);

    clear_level();
}

unsafe fn clear_previous_level_items() {
    for id in DUNGEON.monster_list.ids() {
        crate::entity::player::free_pack_id(id);
    }
    DUNGEON.monster_list.clear();
    with_current_level_mut(|current| current.clear_items());
}

/// Wake monsters in a room when it becomes visible.
pub unsafe fn door_open(room: Option<usize>) {
    if crate::game::room_gone(room) {
        return;
    }

    let (pos, size) = match crate::game::room_bounds(room) {
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
pub unsafe fn new_level() {
    reset_level();
    clear_previous_level_items();

    // Lay out and dig the rooms and passages for this level.
    let rooms = std::array::from_fn(|_| Room::new(IVec2::ZERO, IVec2::ZERO));
    let room_size = IVec2::new(GameConfig::SCREEN_COLS / 3, GameConfig::SCREEN_LINES / 3);
    with_current_level_mut(|current| {
        let _ = current.generate_rooms_and_connections(rooms, room_size);
        current.do_passages();
    });

    bump_no_food();
    populate_level();
}