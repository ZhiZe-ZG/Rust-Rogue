//! Dungeon-level lifecycle orchestration.
//!
//! [`new_level`] resets the current level, generates its rooms and passages,
//! populates it, and prepares the screen. [`door_open`] wakes monsters in a
//! room when it becomes visible.

use glam::IVec2;

use crate::config::GameConfig;
use crate::draw::winat;
use crate::entity::player::CThing;
use crate::game::{clear_level, with_current_level_mut};
use crate::ui::output;

use super::presence::populate_level;
use super::rooms::Room;
use super::structure::Structure;
use super::symbols::{
    free_list, lvl_obj, max_level, no_food, player, thing_t, wake_monster, ISHELD, MLIST,
};
use super::tile::Tile;

fn generate_rooms_and_connections() -> [Room; GameConfig::MAX_ROOMS] {
    let rooms = std::array::from_fn(|_| Room::new(IVec2::ZERO, IVec2::ZERO));
    let room_size = IVec2::new(GameConfig::SCREEN_COLS / 3, GameConfig::SCREEN_LINES / 3);
    with_current_level_mut(|current| current.generate_rooms_and_connections(rooms, room_size))
}

unsafe fn reset_level() {
    let depth = with_current_level_mut(|current| {
        current.map = Structure::new(
            GameConfig::SCREEN_LINES as usize,
            GameConfig::SCREEN_COLS as usize,
            Tile::Empty,
        );
        current.rooms.clear();
        current.room_graph.reset();
        current.passages.clear();
        current.passage_links.clear();
        current.reset_flags();
        current.depth
    });

    (*thing_t(&raw mut player)).t_flags &= !ISHELD;
    if depth > max_level {
        max_level = depth;
    }

    clear_level();
    output::clear_screen();
}

unsafe fn clear_previous_level_items() {
    let mut monster = MLIST.head();
    while !monster.is_null() {
        let next = (*thing_t(monster)).l_next;
        free_list((&raw mut (*thing_t(monster)).t_pack) as *mut *mut CThing);
        monster = next;
    }
    MLIST.free_list();
    free_list((&raw mut lvl_obj) as *mut *mut CThing);
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

    let _generated = generate_rooms_and_connections();
    with_current_level_mut(|current| current.do_passages());

    no_food += 1;
    populate_level();
}
