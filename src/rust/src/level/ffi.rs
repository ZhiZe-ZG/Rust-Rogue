//! Legacy FFI entry points for level generation.
//!
//! Wires the focused submodules together around the C engine's lifecycle:
//! [`new_level`] resets the level, digs/populates it, and draws the screen;
//! [`door_open`] illuminates a room. All raw C symbols live in
//! [`super::symbols`], map mirroring in [`super::mirror`], redraw in
//! [`super::redraw`], and level population in [`super::presence`].

use std::os::raw::c_int;

use glam::IVec2;

use crate::draw::place_at;
use crate::player::{CPlace, CRoom, CThing};

use super::level::current_level_mut;
use super::mirror::{
    apply_room_to_c, copy_flags_to_c, draw_map_ascii, read_c_room_data, sync_passages_to_c,
    sync_rooms_to_c,
};
use super::passages::SCREEN_COLS;
use super::presence::populate_level;
use super::redraw::winat;
use super::rooms::Room;
use super::structure::Structure;
use super::symbols::{
    ISGONE, ISHELD, MAXROOMS, _free_list, clear, level, lvl_obj, max_level, mlist, no_food,
    places, player, thing_t, wake_monster,
};
use super::tile::Tile;

/// Generate this depth's room grid, models, and room-to-room connections.
///
/// Reads the room state from C and asks the current level to generate room
/// models and connections. The result is written back to C and drawn to
/// ncurses/places by [`write_rust_data_back_to_c_and_ncurses`].
unsafe fn generate_rooms_and_connections() -> [Room; MAXROOMS] {
    let current = current_level_mut();
    // Step 1: Read room state from C into Rust-owned data.
    let c_rooms = read_c_room_data();
    // Step 2: Ask Level to generate room grid/models and room connections.
    let bsze = IVec2::new(SCREEN_COLS / 3, 24 / 3);
    current.generate_rooms_and_connections(c_rooms, bsze)
}

/// Write the generated room models back to C and draw the whole map.
///
/// Mirrors room geometry/flags to the C `rooms` array, draws each tile into
/// the C `places` grid, and syncs entry points, passages, and flat flags.
/// This must happen before placing gold and monsters, because `find_floor`
/// looks for floor cells already drawn in `places`.
unsafe fn write_rust_data_back_to_c_and_ncurses(generated: &[Room; MAXROOMS]) {
    use super::symbols::{rooms as c_rooms};

    for i in 0..MAXROOMS {
        let rp = (&raw mut c_rooms[i]) as *mut CRoom;
        apply_room_to_c(&generated[i], rp);
    }

    draw_map_ascii();

    // Mirror the Rust-side rooms, passage components, and flag grids into the
    // C arrays now that rooms, doors, and passages are fully laid out.
    sync_rooms_to_c(current_level_mut());
    sync_passages_to_c(current_level_mut());
    copy_flags_to_c(current_level_mut());
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
    let current = current_level_mut();
    current.depth = level;
    current.map = Structure::new(24, SCREEN_COLS as usize, Tile::Empty);
    current.rooms.clear();
    current.room_graph.reset();
    current.passages.clear();
    current.passage_links.clear();

    (*thing_t(&raw mut player)).t_flags &= !ISHELD; /* unhold when you go down just in case */
    if level > max_level {
        max_level = level;
    }

    // Reset the Rust-side per-cell flags for the fresh depth.
    current.reset_flags();

    // Clean things off from last level.
    for y in 0..32 {
        for x in 0..SCREEN_COLS {
            let pp = place_at((&raw mut places) as *mut CPlace, y, x);
            (*pp).p_ch = b' ' as _;
            (*pp).p_monst = std::ptr::null_mut();
        }
    }
    clear();
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
        _free_list((&raw mut (*thing_t(tp)).t_pack) as *mut *mut CThing);
        tp = next_tp;
    }
    _free_list((&raw mut mlist) as *mut *mut CThing);

    // Throw away stuff left on the previous level (if anything).
    _free_list((&raw mut lvl_obj) as *mut *mut CThing);
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
    current_level_mut().do_passages(); /* Draw passages */
    write_rust_data_back_to_c_and_ncurses(&generated);

    no_food += 1;

    populate_level();
}