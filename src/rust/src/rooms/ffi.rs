use std::os::raw::{c_char, c_int, c_short, c_uchar};

use crate::draw::{place_at, set_tile_char};
use crate::passages::putpass;
use crate::player::{CCoord, CPlace, CRoom, CThing, CThingMonster, CThingObject};
use crate::tile::Tile;

use super::{build_maze_structure, build_room_structure, Room};
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
const MAX_ROOM_TRIES: usize = 100;
const GOLDGRP: c_int = 1;
const GOLD: c_char = b'*' as c_char;
const FLOOR: c_char = b'.' as c_char;
const PASSAGE: c_char = b'#' as c_char;
const H_WALL: c_char = b'-' as c_char;
const V_WALL: c_char = b'|' as c_char;
const DOOR: c_char = b'+' as c_char;
const STAIRS: c_char = b'%' as c_char;
const TRAP: c_char = b'^' as c_char;
const FALSE: c_uchar = 0;
const TRUE: c_uchar = 1;

unsafe extern "C" {
	static mut level: c_int;
	static mut max_level: c_int;
	static mut amulet: c_uchar;
	static mut rooms: [CRoom; MAXROOMS];
	static mut lvl_obj: *mut CThing;
	static mut places: [CPlace; 32 * 80];

	fn wake_monster(y: c_int, x: c_int);
	fn rnd(range: c_int) -> c_int;
	fn step_ok(ch: c_int) -> c_int;
	fn new_thing() -> *mut CThing;
	fn new_item() -> *mut CThing;
	fn _attach(list: *mut *mut CThing, item: *mut CThing);
	fn randmonster(wander: c_uchar) -> c_char;
	fn new_monster(tp: *mut CThing, kind: c_char, cp: *mut CCoord);
	fn give_pack(tp: *mut CThing);
}

#[no_mangle]
pub unsafe extern "C" fn rnd_room() -> c_int {
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

unsafe fn build_plain_room_model(rp: *const CRoom) -> Option<Room> {
	if rp.is_null() {
		return None;
	}

	let width = (*rp).r_max.x;
	let height = (*rp).r_max.y;
	if width <= 0 || height <= 0 {
		return None;
	}

	let position = IVec2::new((*rp).r_pos.x, (*rp).r_pos.y);
	let size = IVec2::new(width, height);
	let structure = build_room_structure(height as usize, width as usize);
	Some(Room::new(position, size, Some(structure), None))
}

unsafe fn build_maze_model(rp: *const CRoom) -> Option<Room> {
	if rp.is_null() {
		return None;
	}

	let width = (*rp).r_max.x;
	let height = (*rp).r_max.y;
	if width <= 0 || height <= 0 {
		return None;
	}

	let position = IVec2::new((*rp).r_pos.x, (*rp).r_pos.y);
	let size = IVec2::new(width, height);
	let structure = build_maze_structure(height as usize, width as usize);
	Some(Room::new(position, size, Some(structure), None))
}

fn tile_to_ascii(tile: Tile, local_y: usize, _local_x: usize, height: usize) -> Option<c_char> {
	match tile {
		Tile::Empty => None,
		Tile::Floor => Some(FLOOR),
		Tile::Wall => {
			if local_y == 0 || local_y + 1 == height {
				Some(H_WALL)
			} else {
				Some(V_WALL)
			}
		}
		Tile::Passage => Some(PASSAGE),
		Tile::Door => Some(DOOR),
		Tile::Stairs => Some(STAIRS),
		Tile::Trap => Some(TRAP),
	}
}

#[no_mangle]
pub unsafe extern "C" fn rnd_pos(rp: *mut CRoom, cp: *mut CCoord) {
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

#[no_mangle]
pub unsafe extern "C" fn do_rooms() {
	let mut bsze = CCoord { x: NUMCOLS / 3, y: NUMLINES / 3 };
	let mut mp = CCoord { x: 0, y: 0 };
	let mut room_models: [Option<Room>; MAXROOMS] = std::array::from_fn(|_| None);

	// Reset per-room state before generating the level layout.
	for i in 0..MAXROOMS {
		let rp = (&raw mut rooms[i]) as *mut CRoom;
		(*rp).r_goldval = 0;
		(*rp).r_nexits = 0;
		(*rp).r_flags = 0;
	}

	let left_out = rnd(4);
	// Randomly mark a few rooms as removed for this level.
	for _ in 0..left_out {
		let room_idx = rnd_room() as usize;
		rooms[room_idx].r_flags |= ISGONE;
	}

	// Compute geometry and room models for every room slot.
	for i in 0..MAXROOMS {
		let rp = (&raw mut rooms[i]) as *mut CRoom;
		let top = CCoord {
			x: (i as c_int % 3) * bsze.x + 1,
			y: (i as c_int / 3) * bsze.y,
		};

		if ((*rp).r_flags & ISGONE) != 0 {
			// Keep rerolling until the off-map placeholder position is valid.
			loop {
				(*rp).r_pos.x = top.x + rnd(bsze.x - 2) + 1;
				(*rp).r_pos.y = top.y + rnd(bsze.y - 2) + 1;
				(*rp).r_max.x = -NUMCOLS;
				(*rp).r_max.y = -NUMLINES;
				if (*rp).r_pos.y > 0 && (*rp).r_pos.y < NUMLINES - 1 {
					break;
				}
			}
			continue;
		}

		if rnd(10) < level - 1 {
			(*rp).r_flags |= ISDARK;
			if rnd(15) == 0 {
				(*rp).r_flags = ISMAZE;
			}
		}

		if ((*rp).r_flags & ISMAZE) != 0 {
			(*rp).r_max.x = bsze.x - 1;
			(*rp).r_max.y = bsze.y - 1;
			(*rp).r_pos.x = top.x;
			if (*rp).r_pos.x == 1 {
				(*rp).r_pos.x = 0;
			}
			(*rp).r_pos.y = top.y;
			if (*rp).r_pos.y == 0 {
				(*rp).r_pos.y += 1;
				(*rp).r_max.y -= 1;
			}
		} else {
			let mut placed = false;
			for _ in 0..MAX_ROOM_TRIES {
				(*rp).r_max.x = rnd(bsze.x - 4) + 4;
				(*rp).r_max.y = rnd(bsze.y - 4) + 4;
				(*rp).r_pos.x = top.x + rnd(bsze.x - (*rp).r_max.x);
				(*rp).r_pos.y = top.y + rnd(bsze.y - (*rp).r_max.y);
				if (*rp).r_pos.y != 0 {
					placed = true;
					break;
				}
			}

			if !placed {
				(*rp).r_flags |= ISGONE;
				continue;
			}
		}

		room_models[i] = build_room_model(rp);
	}

	// Draw prebuilt room models, then place gold and monsters.
	for i in 0..MAXROOMS {
		let rp = (&raw mut rooms[i]) as *mut CRoom;
		if let Some(room) = room_models[i].as_ref() {
			draw_room_ascii(&room);
		} else {
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

#[no_mangle]
pub unsafe extern "C" fn treas_room() {
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

pub unsafe fn draw_room_ascii(room: &Room) {
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
			if matches!(tile, crate::tile::Tile::Passage) {
				let mut pos = CCoord { y: abs_y, x: abs_x };
				putpass(&mut pos);
			}
			set_tile_char(abs_y, abs_x, ch);
		}
	}
}

pub unsafe fn build_room_model(rp: *mut CRoom) -> Option<Room> {
	if rp.is_null() {
		return None;
	}

	if ((*rp).r_flags & ISGONE) != 0 {
		return None;
	}

	if ((*rp).r_flags & ISMAZE) != 0 {
		return build_maze_model(rp);
	}

	build_plain_room_model(rp)
}

/// door_open:
/// Called to illuminate a room. If it is dark, wake anything that might move.
#[no_mangle]
pub unsafe extern "C" fn door_open(rp: *mut CRoom) {
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