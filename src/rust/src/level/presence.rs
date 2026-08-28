//! Populating a generated level: gold, monsters, objects, traps, stairs, and
//! the hero spawn.
//!
//! Room selection and geometry go through the Rust `Level` model
//! (`Level::rnd_room`/`Level::rnd_pos` on `current_level_mut()`); the
//! remaining C `places`/`player` globals are touched via the raw symbols in
//! [`super::symbols`]. [`super::ffi::new_level`] calls these after the
//! rooms/passages have been dug and mirrored.

use std::mem::size_of;
use std::os::raw::{c_char, c_int, c_uchar, c_uint};

use crate::draw::place_at;
use crate::game::places;
use crate::player::{CCoord, CPlace, CRoom, CThing};
use crate::rnd::rnd;

use super::ffitools::{FLOOR, F_REAL, PASSAGE, STAIRS};
use super::level::{current_level_mut, LEVEL_WIDTH};
use super::redraw::{chat_at, chat_at_mut};
use super::symbols::{
    AMULET, AMULETLEVEL, FALSE, GOLD, GOLDGRP, ISGONE, ISHALU, ISMANY, ISMEAN, MAXOBJ,
    MAXROOMS, MAXTRAPS, MAXTRIES, MAXTREAS, MINTREAS, NTRAPS, PLAYER, SEEMONST, TREAS_ROOM, TRUE,
    _attach, amulet, enter_room, give_pack, level, lvl_obj, max_level, mlist, mvaddch, new_item,
    new_monster, new_thing, ntraps, player, randmonster, roomin, rooms, seenstairs, stairs,
    step_ok, thing_o, thing_t, turn_see, visuals,
};
use super::tile::Tile;

/// Map a pointer into the C `rooms` array back to its Rust room-slot index.
///
/// The pointer is only used for address arithmetic against the C `rooms`
/// global so the caller can look the slot up in the `Level::rooms` member;
/// it is never dereferenced. Returns `None` for a null or out-of-range
/// pointer.
unsafe fn room_slot_of(rp: *mut CRoom) -> Option<usize> {
    if rp.is_null() {
        return None;
    }
    let base = rooms.as_ptr() as usize;
    let idx = (rp as usize).wrapping_sub(base) / size_of::<CRoom>();
    (idx < MAXROOMS).then_some(idx)
}

/// Find a floor cell to place something, optionally avoiding monsters.
///
/// If `rp` is null a random room slot is tried each iteration via
/// `Level::rnd_room`; otherwise `rp` is mapped back to the matching `Level`
/// room slot. Room selection and geometry come from the Rust `Level` model
/// (`Level::rnd_pos`), while the candidate cell is validated against the C
/// `places` grid. Returns `TRUE` and stores the chosen cell into `cp` on
/// success; `FALSE` when `limit` (if nonzero) attempts are exhausted.
#[no_mangle]
pub unsafe extern "C" fn find_floor(rp: *mut CRoom, cp: *mut CCoord, limit: c_int, monst: c_uchar) -> c_uchar {
    if cp.is_null() {
        return FALSE;
    }

    let current = current_level_mut();
    let room_idx = room_slot_of(rp);

    let mut cnt = limit;
    loop {
        if limit != 0 {
            if cnt == 0 {
                return FALSE;
            }
            cnt -= 1;
        }

        let idx = match room_idx {
            Some(idx) => idx,
            None => current.rnd_room(),
        };
        let room = &current.rooms[idx];
        let compchar = if room.is_maze() { PASSAGE } else { FLOOR };
        let pos = current.rnd_pos(room);

        (*cp).x = pos.x;
        (*cp).y = pos.y;
        let pp = place_at((&raw mut places) as *mut CPlace, (*cp).y, (*cp).x);
        if monst != 0 {
            if (*pp).p_monst.is_null() && step_ok((*pp).p_ch as c_int) != 0 {
                return TRUE;
            }
        } else if (*pp).p_ch == compchar {
            return TRUE;
        }
    }
}

/// Fill one treasure room with `MIN..MAX` objects and monsters.
unsafe fn treas_room() {
    let current = current_level_mut();
    let mut mp = CCoord { x: 0, y: 0 };
    let idx = current.rnd_room();
    let room = &current.rooms[idx];
    let rp = &mut rooms[idx];

    let mut spots = (room.size.y - 2) * (room.size.x - 2) - MINTREAS;
    if spots > (MAXTREAS - MINTREAS) {
        spots = MAXTREAS - MINTREAS;
    }

    let mut nm = rnd(spots) + MINTREAS;
    let num_monst = nm;
    while nm > 0 {
        find_floor(rp as *mut CRoom, &mut mp, 2 * MAXTRIES, FALSE);
        let tp = new_thing();
        (*thing_o(tp)).o_pos = mp;
        _attach((&raw mut lvl_obj) as *mut *mut CThing, tp);
        (*place_at((&raw mut places) as *mut CPlace, mp.y, mp.x)).p_ch = (*thing_o(tp)).o_type as c_char;
        nm -= 1;
    }

    nm = rnd(spots) + MINTREAS;
    if nm < num_monst + 2 {
        nm = num_monst + 2;
    }
    spots = (room.size.y - 2) * (room.size.x - 2);
    if nm > spots {
        nm = spots;
    }

    level += 1;
    while nm > 0 {
        if find_floor(rp as *mut CRoom, &mut mp, MAXTRIES, TRUE) != 0 {
            let tp = new_item();
            new_monster(tp, randmonster(FALSE), &mut mp);
            (*thing_t(tp)).t_flags |= ISMEAN;
            give_pack(tp);
        }
        nm -= 1;
    }
    level -= 1;
}

/// Scatter gold and monsters through every active room.
///
/// Each room may hold a gold stash (value `rnd(50 + 10*level) + 2`) and has a
/// chance of a monster guarding it (higher when the room has gold).
///
/// ```text
/// Uses globals: amulet, level, max_level, rooms, lvl_obj, places.
/// ```
unsafe fn place_room_contents() {
    let mut mp = CCoord { x: 0, y: 0 };

    for i in 0..MAXROOMS {
        let rp = (&raw mut rooms[i]) as *mut CRoom;

        if (rooms[i].r_flags & ISGONE) != 0 {
            continue;
        }

        if rnd(2) == 0 && (amulet == 0 || level >= max_level) {
            let gold = new_item();

            if !gold.is_null() {
                let og = thing_o(gold);

                (*og).o_arm = rnd(50 + 10 * level) + 2;
                (*rp).r_goldval = (*og).o_arm;
                find_floor(rp, &mut (*rp).r_gold, FALSE as c_int, FALSE);
                (*og).o_pos = (*rp).r_gold;
                (*place_at((&raw mut places) as *mut CPlace, (*rp).r_gold.y, (*rp).r_gold.x)).p_ch = GOLD;
                (*og).o_flags = ISMANY;
                (*og).o_group = GOLDGRP;
                (*og).o_type = GOLD as c_int;
                _attach((&raw mut lvl_obj) as *mut *mut CThing, gold);
            }
        }

        if rnd(100) < if (*rp).r_goldval > 0 { 80 } else { 25 } {
            let tp = new_item();
            if !tp.is_null() {
                find_floor(rp, &mut mp, FALSE as c_int, TRUE);
                new_monster(tp, randmonster(FALSE), &mut mp);
                give_pack(tp);
            }
        }
    }
}

/// Put potions and scrolls (and, deep enough, the Amulet of Yendor) on this level.
///
/// ```text
/// Uses globals: amulet, level, max_level, lvl_obj, places (via chat).
/// ```
unsafe fn put_things() {
    // Once you have found the amulet, the only way to get new stuff is
    // go down into the dungeon.
    if amulet != 0 && level < max_level {
        return;
    }

    // Check for treasure rooms, and if so, put it in.
    if rnd(TREAS_ROOM as c_int) == 0 {
        treas_room();
    }

    // Do MAXOBJ attempts to put things on a level.
    for _ in 0..MAXOBJ {
        if rnd(100) < 36 {
            // Pick a new object and link it in the list.
            let obj = new_thing();
            _attach((&raw mut lvl_obj) as *mut *mut CThing, obj);
            // Put it somewhere.
            let og = thing_o(obj);
            let pos = &raw mut (*og).o_pos;
            find_floor(std::ptr::null_mut(), pos, FALSE as c_int, FALSE);
            let pp = place_at((&raw mut places) as *mut CPlace, (*og).o_pos.y, (*og).o_pos.x);
            (*pp).p_ch = (*og).o_type as c_char;
        }
    }

    // If he is really deep in the dungeon and he hasn't found the amulet
    // yet, put it somewhere on the ground.
    if level >= AMULETLEVEL && amulet == 0 {
        let obj = new_item();
        _attach((&raw mut lvl_obj) as *mut *mut CThing, obj);
        let og = thing_o(obj);
        (*og).o_hplus = 0;
        (*og).o_dplus = 0;
        // Copy "0x0" into the 8-byte damage strings (zero-padded), matching
        // C's strncpy(obj->o_damage, "0x0", sizeof(obj->o_damage)).
        (*og).o_damage = [b'0' as c_char, b'x' as c_char, b'0' as c_char, 0, 0, 0, 0, 0];
        (*og).o_hurldmg = [b'0' as c_char, b'x' as c_char, b'0' as c_char, 0, 0, 0, 0, 0];
        (*og).o_arm = 11;
        (*og).o_type = AMULET as c_int;
        // Put it somewhere.
        let pos = &raw mut (*og).o_pos;
        find_floor(std::ptr::null_mut(), pos, FALSE as c_int, FALSE);
        let pp = place_at((&raw mut places) as *mut CPlace, (*og).o_pos.y, (*og).o_pos.x);
        (*pp).p_ch = AMULET;
    }
}

/// Scatter traps (scaled by depth) on floor cells.
///
/// ```text
/// Uses globals: level, ntraps, places, stairs.
/// ```
unsafe fn place_traps() {
    if rnd(10) >= level {
        return;
    }

    ntraps = rnd(level / 4) + 1;
    if ntraps > MAXTRAPS {
        ntraps = MAXTRAPS;
    }

    let mut i = ntraps;
    while i > 0 {
        loop {
            find_floor(std::ptr::null_mut(), &raw mut stairs, FALSE as c_int, FALSE);
            if chat_at(stairs.y, stairs.x) == FLOOR {
                break;
            }
        }
        let sp = &raw mut (*place_at((&raw mut places) as *mut CPlace, stairs.y, stairs.x)).p_flags;
        *sp = ((*sp as u8) & !(F_REAL as u8)) as c_char;
        *sp = ((*sp as u8) | rnd(NTRAPS) as u8) as c_char;

        // Keep the Rust level model in sync: the trap occupies a floor cell
        // in the tile map and is non-real (hidden) so it can be revealed.
        let current = current_level_mut();
        current.map.set(stairs.y as usize, stairs.x as usize, Tile::Trap);
        let idx = stairs.y as usize * LEVEL_WIDTH + stairs.x as usize;
        current.flags.real[idx] = false;
        i -= 1;
    }
}

/// Place the down staircase on a floor cell.
///
/// ```text
/// Uses globals: stairs, places, seenstairs.
/// ```
unsafe fn place_stairs() {
    find_floor(std::ptr::null_mut(), &raw mut stairs, FALSE as c_int, FALSE);
    *chat_at_mut(stairs.y, stairs.x) = STAIRS;
    seenstairs = FALSE;
}

/// Link every monster on the level to the room its position falls in.
///
/// ```text
/// Uses globals: mlist.
/// ```
pub(crate) unsafe fn link_monsters_to_rooms() {
    let mut tp = mlist;
    while !tp.is_null() {
        let t = thing_t(tp);
        (*t).t_room = roomin(&raw mut (*t).t_pos);
        tp = (*t).l_next;
    }
}

/// Place the hero on an open floor cell and finalize the screen.
///
/// Moves the hero to a random walkable cell, triggers room entry, draws the
/// hero, and refreshes the display according to the hero's current flags.
///
/// ```text
/// Uses globals: player, places (via find_floor).
/// ```
unsafe fn place_hero() {
    find_floor(std::ptr::null_mut(), &raw mut (*thing_t(&raw mut player)).t_pos, FALSE as c_int, TRUE);
    enter_room(&raw mut (*thing_t(&raw mut player)).t_pos);
    mvaddch(
        (*thing_t(&raw mut player)).t_pos.y,
        (*thing_t(&raw mut player)).t_pos.x,
        PLAYER as c_uint,
    );
    if ((*thing_t(&raw mut player)).t_flags & SEEMONST) != 0 {
        turn_see(FALSE);
    }
    if ((*thing_t(&raw mut player)).t_flags & ISHALU) != 0 {
        visuals();
    }
}

/// Run the full population pass: gold/monsters, objects, traps, stairs, and
/// the hero.
///
/// Called by [`super::ffi::new_level`] after the map is drawn and mirrored.
pub(crate) unsafe fn populate_level() {
    place_room_contents();
    put_things(); /* Place objects (if any) */
    place_traps();
    place_stairs();
    link_monsters_to_rooms();
    place_hero();
}
