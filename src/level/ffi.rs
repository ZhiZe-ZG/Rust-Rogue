//! Legacy FFI entry points for level generation.
//!
//! Wires the focused submodules together around the C engine's lifecycle:
//! [`new_level`] resets the level, digs/populates it, and draws the screen;
//! [`door_open`] illuminates a room. All raw C symbols live in
//! [`super::symbols`], map mirroring in [`super::mirror`], redraw in
//! [`super::redraw`], and level population in [`super::presence`].

use glam::IVec2;

use crate::curses as cur;

use crate::game::{clear_level, with_current_level_mut};
use crate::player::{CRoom, CThing};

use super::mirror::{apply_room_to_c, read_c_room_data, sync_passages_to_c, sync_rooms_to_c};
use super::passages::SCREEN_COLS;
use super::presence::populate_level;
use super::rooms::Room;
use super::structure::Structure;
use super::symbols::{
    free_list, level, lvl_obj, max_level, mlist, no_food, player, thing_t, wake_monster, ISGONE,
    ISHELD, MAXROOMS,
};
use super::tile::Tile;
use crate::draw::winat;

/// Generate this depth's room grid, models, and room-to-room connections.
///
/// Reads the room state from C and asks the current level to generate room
/// models and connections. The result is written back to C and drawn to
/// ncurses/places by [`write_rust_data_back_to_c_and_ncurses`].
unsafe fn generate_rooms_and_connections() -> [Room; MAXROOMS] {
    // Step 1: Read room state from C into Rust-owned data.
    let c_rooms = read_c_room_data();
    // Step 2: Ask Level to generate room grid/models and room connections.
    let bsze = IVec2::new(SCREEN_COLS / 3, 24 / 3);
    with_current_level_mut(|current| current.generate_rooms_and_connections(c_rooms, bsze))
}

/// Write the generated room models back to C and draw the whole map.
///
/// Mirrors room geometry/flags to the C `rooms` array, draws each tile into
/// the C `places` grid, and syncs entry points, passages, and flat flags.
/// This must happen before placing gold and monsters, because `find_floor`
/// looks for floor cells already drawn in `places`.
unsafe fn write_rust_data_back_to_c_and_ncurses(generated: &[Room; MAXROOMS]) {
    use super::symbols::rooms as c_rooms;

    for i in 0..MAXROOMS {
        let rp = (&raw mut c_rooms[i]) as *mut CRoom;
        apply_room_to_c(&generated[i], rp);
    }

    // Mirror the Rust-side room and passage components into the C arrays now
    // that rooms, doors, and passages are fully laid out. Per-cell display
    // glyphs and flat flags are computed on the fly by `crate::draw`.
    with_current_level_mut(|current| sync_rooms_to_c(current));
    with_current_level_mut(|current| sync_passages_to_c(current));
}

/// Reset the in-memory level and the C screen for a fresh dungeon depth.
///
/// Stores the current depth, clears the Rust-side level state (rooms, room
/// graph, passages, and the full map), blanks the C `places` grid, and
/// unholds the hero.
///
/// ```text
/// Uses globals: level, max_level, places, player.
/// ```
unsafe fn begin_new_level() {
    with_current_level_mut(|current| {
        current.depth = level;
        current.map = Structure::new(24, SCREEN_COLS as usize, Tile::Empty);
        current.rooms.clear();
        current.room_graph.reset();
        current.passages.clear();
        current.passage_links.clear();
        current.reset_flags();
    });

    (*thing_t(&raw mut player)).t_flags &= !ISHELD; /* unhold when you go down just in case */
    if level > max_level {
        max_level = level;
    }

    // Reset the Rust-owned places grid and monster map.
    clear_level();
    cur::clear();
}

/// Release the monsters and objects left on the previous level.
///
/// Frees every monster's pack, then the monster list itself, and finally
/// the level-object list.
///
/// ```text
/// Uses globals: mlist, lvl_obj.
/// ```
unsafe fn clear_previous_level_items() {
    // Free up the monsters on the last level.
    let mut tp = mlist;
    while !tp.is_null() {
        let next_tp = (*thing_t(tp)).l_next;
        free_list((&raw mut (*thing_t(tp)).t_pack) as *mut *mut CThing);
        tp = next_tp;
    }
    free_list((&raw mut mlist) as *mut *mut CThing);

    // Throw away stuff left on the previous level (if anything).
    free_list((&raw mut lvl_obj) as *mut *mut CThing);
}

/// door_open:
/// Called to illuminate a room. If it is dark, wake anything that might move.
pub unsafe fn door_open(rp: *mut CRoom) {
    if ((*rp).r_flags & ISGONE) != 0 {
        return;
    }
    let y0 = (*rp).r_pos.y;
    let x0 = (*rp).r_pos.x;
    let y_end = y0 + (*rp).r_max.y;
    let x_end = x0 + (*rp).r_max.x;
    let mut y = y0;
    while y < y_end {
        let mut x = x0;
        while x < x_end {
            if (winat(y, x) as u8).is_ascii_uppercase() {
                wake_monster(y, x);
            }
            x += 1;
        }
        y += 1;
    }
}

/// new_level:
/// Dig and draw a new level.
///
/// Called whenever the hero enters a new dungeon depth.  It clears the
/// previous level's map, monsters, and objects; digs the rooms and
/// passages; places objects, traps, and the down staircase; and then
/// moves the hero to a random open floor and draws the new screen.
///
/// ```text
/// Uses globals: player, level, max_level, places, mlist, lvl_obj,
/// no_food, ntraps, stairs, seenstairs, rooms, passages.
/// ```
#[no_mangle]
pub unsafe extern "C" fn new_level() {
    begin_new_level();
    clear_previous_level_items();
    let generated = generate_rooms_and_connections();

    // Dig corridors for the room-connection plan generated by Level and mirror
    // the resulting tiles/flags back to the C `places` grid.
    with_current_level_mut(|current| current.do_passages()); /* Draw passages */
    write_rust_data_back_to_c_and_ncurses(&generated);

    no_food += 1;

    populate_level();
}
