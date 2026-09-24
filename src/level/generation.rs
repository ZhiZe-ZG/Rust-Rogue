//! Dungeon-level lifecycle orchestration.
//!
//! [`new_level`] resets the current level, generates its rooms and passages,
//! populates it, and prepares the screen. [`door_open`] wakes monsters in a
//! room when it becomes visible.

use glam::IVec2;

use crate::config::GameConfig;
use crate::draw::winat;
use crate::entity::monster_list::MLIST;
use crate::entity::monsters::wake_monster;
use crate::entity::player::{Thing, ThingMonster, MonsterFlags};
use crate::game::{clear_level, with_current_level_mut};
use crate::globals::{max_level, no_food};
use crate::ui::output;

use super::presence::populate_level;
use super::structure::Room;

/// Interpret `tp` as a monster (`CThingMonster`).
#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

unsafe fn reset_level() {
    let depth = with_current_level_mut(|current| current.reset_for_new_level());

    (*thing_t(crate::game::player_ptr()))
        .t_flags
        .remove(MonsterFlags::HELD);
    if depth > max_level {
        max_level = depth;
    }

    clear_level();
    output::clear_screen();
}

unsafe fn clear_previous_level_items() {
    let mut monster = MLIST.head();
    while !monster.is_null() {
        let next = crate::entity::player::thing_next(monster);
        crate::item::thing_list::free_pack(monster);
        monster = next;
    }
    MLIST.free_list();
    with_current_level_mut(|current| current.items.clear());
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
            if (winat(y, x) as u8).is_ascii_uppercase() {
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

    no_food += 1;
    populate_level();
}