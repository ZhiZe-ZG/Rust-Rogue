use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::draw::{place_at, set_tile_char};
use crate::player::{CCoord, CPlace, CRoom, CThing, CThingMonster, CThingObject};
use crate::rnd::rnd;

use super::ffitools::{tile_to_ascii, FLOOR, PASSAGE, STAIRS};
use super::passages::{do_passages, putpass};
use super::rooms::Room;

use super::{current_level_mut};
use super::tile::Tile;
use glam::IVec2;

const ISGONE: c_short = 0o000002;
const ISMAZE: c_short = 0o000004;
const ISDARK: c_short = 0o000001;
const ISMANY: c_int = 0o0000010;
const ISMEAN: c_short = 0o0004000;
const NUMCOLS: c_int = 80;
const NUMLINES: c_int = 24;
const MAXROOMS: usize = 9;
const MAXTREAS: c_int = 10;
const MINTREAS: c_int = 2;
const MAXTRIES: c_int = 10;
const MAXOBJ: c_int = 9;
const TREAS_ROOM: c_int = 20;
const AMULETLEVEL: c_int = 26;
const GOLDGRP: c_int = 1;
const AMULET: c_char = b',' as c_char;
const GOLD: c_char = b'*' as c_char;
const FALSE: c_uchar = 0;
const TRUE: c_uchar = 1;

const ISHELD: c_short = 0o0000400;
const SEEMONST: c_short = 0o040000;
const ISHALU: c_short = 0o0004000;
const F_REAL: c_char = 0x10u8 as c_char;
const PLAYER: c_char = b'@' as c_char;
const MAXCOLS: c_int = 80;
const MAXLINES: c_int = 32;
const MAXTRAPS: c_int = 10;
const NTRAPS: c_int = 8;

unsafe extern "C" {
	static mut level: c_int;
	static mut max_level: c_int;
	static mut amulet: c_uchar;
	static mut rooms: [CRoom; MAXROOMS];
	static mut lvl_obj: *mut CThing;
	static mut places: [CPlace; 32 * 80];
	static mut player: CThing;
	static mut mlist: *mut CThing;
	static mut no_food: c_int;
	static mut ntraps: c_int;
	static mut stairs: CCoord;
	static mut seenstairs: c_uchar;

	fn wake_monster(y: c_int, x: c_int);
	fn step_ok(ch: c_int) -> c_int;
	fn new_thing() -> *mut CThing;
	fn new_item() -> *mut CThing;
	fn _attach(list: *mut *mut CThing, item: *mut CThing);
	fn randmonster(wander: c_uchar) -> c_char;
	fn new_monster(tp: *mut CThing, kind: c_char, cp: *mut CCoord);
	fn give_pack(tp: *mut CThing);

	fn clear() -> c_int;
	fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int;
	fn enter_room(cp: *mut CCoord);
	fn turn_see(turn_off: c_uchar) -> c_uchar;
	fn _free_list(ptr: *mut *mut CThing);
	fn roomin(cp: *mut CCoord) -> *mut CRoom;
	fn visuals();
}

unsafe fn rnd_room() -> c_int {
	loop {
		let rm = rnd(MAXROOMS as c_int);
		if (rooms[rm as usize].r_flags & ISGONE) == 0 {
			return rm;
		}
	}
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
	tp as *mut CThingObject
}

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
	tp as *mut CThingMonster
}

#[inline]
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
	(*place_at((&raw mut places) as *mut CPlace, y, x)).p_ch
}

#[inline]
unsafe fn winat(y: c_int, x: c_int) -> c_char {
	let tp = (*place_at((&raw mut places) as *mut CPlace, y, x)).p_monst;
	if tp.is_null() {
		chat_at(y, x)
	} else {
		(*thing_o(tp)).o_packch
	}
}

unsafe fn rnd_pos(rp: *mut CRoom, cp: *mut CCoord) {
	if rp.is_null() || cp.is_null() {
		return;
	}
	(*cp).x = (*rp).r_pos.x + rnd((*rp).r_max.x - 2) + 1;
	(*cp).y = (*rp).r_pos.y + rnd((*rp).r_max.y - 2) + 1;
}

#[no_mangle]
pub unsafe extern "C" fn find_floor(rp: *mut CRoom, cp: *mut CCoord, limit: c_int, monst: c_uchar) -> c_uchar {
	if cp.is_null() {
		return FALSE;
	}

	let pickroom = rp.is_null();
	let mut room_ptr = rp;
	let mut compchar: c_char = 0;

	if !pickroom {
		compchar = if ((*room_ptr).r_flags & ISMAZE) != 0 { PASSAGE } else { FLOOR };
	}

	let mut cnt = limit;
	loop {
		if limit != 0 {
			if cnt == 0 {
				return FALSE;
			}
			cnt -= 1;
		}

		if pickroom {
			room_ptr = (&raw mut rooms[rnd_room() as usize]) as *mut CRoom;
			compchar = if ((*room_ptr).r_flags & ISMAZE) != 0 { PASSAGE } else { FLOOR };
		}

		rnd_pos(room_ptr, cp);
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

unsafe fn read_c_room_data() -> [Room; MAXROOMS] {
	std::array::from_fn(|i| {
		let rp = (&raw mut rooms[i]) as *const CRoom;
		room_from_c(rp)
	})
}

unsafe fn write_rust_data_back_to_c_and_ncurses(generated: &[Room; MAXROOMS]) {
	let mut mp = CCoord { x: 0, y: 0 };

	// Draw prebuilt room models, then place gold and monsters.
	for i in 0..MAXROOMS {
		let rp = (&raw mut rooms[i]) as *mut CRoom;
		let room = &generated[i];
		apply_room_to_c(room, rp);

		if room.is_gone() {
			continue;
		}

		draw_room_ascii(room);

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

unsafe fn room_from_c(rp: *const CRoom) -> Room {
	let mut room = Room::new(
		IVec2::new((*rp).r_pos.x, (*rp).r_pos.y),
		IVec2::new((*rp).r_max.x, (*rp).r_max.y),
		None,
		None,
	);
	room.gold = IVec2::new((*rp).r_gold.x, (*rp).r_gold.y);
	room.goldval = (*rp).r_goldval;
	room.gone = ((*rp).r_flags & ISGONE) != 0;
	room.dark = ((*rp).r_flags & ISDARK) != 0;
	room.maze = ((*rp).r_flags & ISMAZE) != 0;
	room.entry_point_count = (*rp).r_nexits;
	room
}

unsafe fn apply_room_to_c(state: &Room, rp: *mut CRoom) {
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

unsafe fn treas_room() {
	let mut mp = CCoord { x: 0, y: 0 };
	let rp = &mut rooms[rnd_room() as usize];

	let mut spots = (rp.r_max.y - 2) * (rp.r_max.x - 2) - MINTREAS;
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
	spots = (rp.r_max.y - 2) * (rp.r_max.x - 2);
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

/// put_things:
/// Put potions and scrolls on this level.
///
/// Places random objects (up to `MAXOBJ`) onto the current level, and if the
/// player is deep enough (at least `AMULETLEVEL`) without having found the
/// Amulet yet, places the Amulet on the floor.
///
/// Uses globals: amulet, level, max_level, lvl_obj, places (via chat).
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

unsafe fn draw_room_ascii(room: &Room) {
	let height = room.size.y;
	let width = room.size.x;
	if height <= 0 || width <= 0 {
		return;
	}

	let height_usize = height as usize;
	let width_usize = width as usize;
	for local_y in 0..height_usize {
		for local_x in 0..width_usize {
			let tile = match room.structure.get(local_y, local_x) {
				Some(tile) => tile,
				None => continue,
			};
			let ch = match tile_to_ascii(tile, local_y, local_x, height_usize) {
				Some(ch) => ch,
				None => continue,
			};
			let abs_y = room.position.y + local_y as c_int;
			let abs_x = room.position.x + local_x as c_int;
			if matches!(tile, Tile::Passage) {
				let mut pos = CCoord { y: abs_y, x: abs_x };
				putpass(&mut pos);
			}
			set_tile_char(abs_y, abs_x, ch);
		}
	}
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
/// Uses globals: player, hero (player.t_pos), level, max_level, places,
/// mlist, lvl_obj, no_food, ntraps, stairs, seenstairs, rooms, passages
/// (via do_passages), player.t_flags (via
/// enter_room / turn_see / visuals).
#[no_mangle]
pub unsafe extern "C" fn new_level() {
	let current = current_level_mut();
	current.depth = level;
	current.map = super::Structure::new(NUMLINES as usize, NUMCOLS as usize, Tile::Empty);
	current.rooms.clear();
	current.room_connections.clear();
	current.passages.clear();

	let mut tp: *mut CThing;
	let mut pp: *mut CPlace;
	let mut sp: *mut c_char;

	(*thing_t(&raw mut player)).t_flags &= !ISHELD; /* unhold when you go down just in case */
	if level > max_level {
		max_level = level;
	}

	// Clean things off from last level.
	for y in 0..MAXLINES {
		for x in 0..MAXCOLS {
			pp = place_at((&raw mut places) as *mut CPlace, y, x);
			(*pp).p_ch = b' ' as c_char;
			(*pp).p_flags = F_REAL;
			(*pp).p_monst = std::ptr::null_mut();
		}
	}
	clear();

	// Free up the monsters on the last level.
	tp = mlist;
	while !tp.is_null() {
		let next_tp = (*thing_t(tp)).l_next;
		_free_list((&raw mut (*thing_t(tp)).t_pack) as *mut *mut CThing);
		tp = next_tp;
	}
	_free_list((&raw mut mlist) as *mut *mut CThing);

	// Throw away stuff left on the previous level (if anything).
	_free_list((&raw mut lvl_obj) as *mut *mut CThing);

	// Step 1: Read room state from C into Rust-owned data.
	let c_rooms = read_c_room_data();
	// Step 2: Ask Level to generate room grid/models and room connections.
	let bsze = IVec2::new(NUMCOLS / 3, NUMLINES / 3);
	let generated = current.generate_rooms_and_connections(c_rooms, bsze);
	// Step 3: Write generated room state back to C and draw to ncurses/places.
	write_rust_data_back_to_c_and_ncurses(&generated);

	// Dig corridors for the room-connection plan generated by Level.
	do_passages(&current.room_connections); /* Draw passages */

	no_food += 1;

	put_things(); /* Place objects (if any) */

	// Place the traps.
	if rnd(10) < level {
		ntraps = rnd(level / 4) + 1;
		if ntraps > MAXTRAPS {
			ntraps = MAXTRAPS;
		}
		let mut i = ntraps;
		while i > 0 {
			/*
			 * Not only wouldn't it be NICE to have traps in mazes
			 * (not that we care about being nice), since the trap
			 * number is stored where the passage number is, we
			 * can't actually do it.
			 */
			loop {
				find_floor(std::ptr::null_mut(), &raw mut stairs, FALSE as c_int, FALSE);
				if chat_at(stairs.y, stairs.x) == FLOOR {
					break;
				}
			}
			sp = &raw mut (*place_at((&raw mut places) as *mut CPlace, stairs.y, stairs.x)).p_flags;
			*sp = ((*sp as u8) & !(F_REAL as u8)) as c_char;
			*sp = ((*sp as u8) | rnd(NTRAPS) as u8) as c_char;
			i -= 1;
		}
	}

	// Place the staircase down.
	find_floor(std::ptr::null_mut(), &raw mut stairs, FALSE as c_int, FALSE);
	*chat_at_mut(stairs.y, stairs.x) = STAIRS;
	seenstairs = FALSE;

	tp = mlist;
	while !tp.is_null() {
		let t = thing_t(tp);
		(*t).t_room = roomin(&raw mut (*t).t_pos);
		tp = (*t).l_next;
	}

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

#[inline]
unsafe fn chat_at_mut(y: c_int, x: c_int) -> *mut c_char {
	&raw mut (*place_at((&raw mut places) as *mut CPlace, y, x)).p_ch
}
