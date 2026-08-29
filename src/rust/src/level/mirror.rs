//! C↔Rust translation for rooms, passages, and per-cell flags.
//!
//! Bridges the Rust-side level structures (`Level`, `Room`, tile map, flag
//! grids) and the C `rooms`/`passages`/`places` globals, and draws the merged
//! Rust tile map to the C `places` grid in one pass. Nothing here is a
//! `#[no_mangle]` export; [`super::ffi`] orchestrates these helpers around
//! the legacy engine lifecycle.

use std::os::raw::{c_char, c_int, c_short};

use glam::IVec2;

use crate::player::{CCoord, CRoom};

use super::level::Level;
use super::passages::{MAX_EXITS, MAX_PASSAGES};
use super::rooms::Room;
use super::symbols::{ISDARK, ISGONE, ISMAZE, rooms, passages};
use super::tile::Tile;

/// Convert one C `CRoom` into a Rust [`Room`].
pub(crate) unsafe fn room_from_c(rp: *const CRoom) -> Room {
    let mut room = Room::new(
        IVec2::new((*rp).r_pos.x, (*rp).r_pos.y),
        IVec2::new((*rp).r_max.x, (*rp).r_max.y),
    );
    room.gold = IVec2::new((*rp).r_gold.x, (*rp).r_gold.y);
    room.goldval = (*rp).r_goldval;
    room.gone = ((*rp).r_flags & ISGONE) != 0;
    room.dark = ((*rp).r_flags & ISDARK) != 0;
    room.maze = ((*rp).r_flags & ISMAZE) != 0;
    room.entry_point_count = (*rp).r_nexits;
    room
}

/// Write one Rust [`Room`] back into a C `CRoom`.
pub(crate) unsafe fn apply_room_to_c(state: &Room, rp: *mut CRoom) {
    (*rp).r_pos = CCoord {
        x: state.position.x,
        y: state.position.y,
    };
    (*rp).r_max = CCoord {
        x: state.size.x,
        y: state.size.y,
    };
    (*rp).r_gold = CCoord {
        x: state.gold.x,
        y: state.gold.y,
    };
    (*rp).r_goldval = state.goldval;
    let mut flags: c_short = 0;
    if state.gone {
        flags |= ISGONE;
    }
    if state.dark {
        flags |= ISDARK;
    }
    if state.maze {
        flags |= ISMAZE;
    }
    (*rp).r_flags = flags;
    (*rp).r_nexits = state.entry_point_count;
}

/// Read every C room slot into a Rust [`Room`] array.
pub(crate) unsafe fn read_c_room_data() -> [Room; super::symbols::MAXROOMS] {
    std::array::from_fn(|i| {
        let rp = (&raw mut rooms[i]) as *const CRoom;
        room_from_c(rp)
    })
}

// NOTE: The per-cell display glyph (`p_ch`) and flat flags (`p_flags`) are no
// longer cached in a `places` grid. `crate::draw` computes the glyph for every
// cell on the fly from the `Level` tile map and `LevelFlags` grids, so the
// former `draw_map_ascii` and `copy_flags_to_c` mirrors are obsolete and have
// been removed.

/// Copy `lvl`'s rooms' Rust-side entry points into the C `rooms` array so
/// the engine can follow room exits.
/// Uses globals: `rooms`.
pub(crate) unsafe fn sync_rooms_to_c(lvl: &Level) {
    for (i, room) in lvl.rooms.iter().enumerate() {
        let rp = &raw mut rooms[i];
        (*rp).r_nexits = room.entry_point_count.min(MAX_EXITS as i32);
        for (j, ep) in room.entry_points.iter().take(MAX_EXITS).enumerate() {
            let abs = *ep + room.position;
            (*rp).r_exit[j] = CCoord { x: abs.x, y: abs.y };
        }
    }
}

/// Copy `lvl`'s numbered passage components into the C `passages` array.
///
/// Each Rust `PassageLinks` entry (produced by `Level::number_passages`) is
/// written into the matching `passages[]` slot: `r_nexits` and the absolute
/// coordinates of its doorways.
/// Uses globals: `passages`.
pub(crate) unsafe fn sync_passages_to_c(lvl: &Level) {
    for rp in &mut passages[..MAX_PASSAGES] {
        rp.r_nexits = 0;
    }
    for (i, links) in lvl.passage_links.iter().enumerate().take(MAX_PASSAGES) {
        let rp = &mut passages[i];
        rp.r_nexits = links.exits.len().min(MAX_EXITS) as c_int;
        for (j, exit) in links.exits.iter().take(MAX_EXITS).enumerate() {
            rp.r_exit[j] = CCoord { x: exit.x, y: exit.y };
        }
    }
}