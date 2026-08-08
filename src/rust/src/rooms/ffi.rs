use std::os::raw::{c_char, c_int, c_short};

use crate::draw::{place_at, set_tile_char};
use crate::player::{CCoord, CPlace, CThing, CThingObject, CRoom};

use super::{build_maze_structure, build_room_structure, tile_to_ascii, Room};
use glam::IVec2;

const ISGONE: c_short = 0o000002;
const ISMAZE: c_short = 0o000004;

unsafe extern "C" {
	static mut places: [CPlace; 32 * 80];

	fn wake_monster(y: c_int, x: c_int);
	fn putpass(cp: *mut CCoord);
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
	tp as *mut CThingObject
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

unsafe fn build_room_model(rp: *const CRoom) -> Option<Room> {
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
	Some(Room::new(position, size, structure))
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
	Some(Room::new(position, size, structure))
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
			set_tile_char(
				room.position.y + local_y as c_int,
				room.position.x + local_x as c_int,
				ch,
			);
		}
	}
}

unsafe fn draw_maze_ascii(room: &Room) {
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
			if !matches!(tile, crate::tile::Tile::Passage) {
				continue;
			}
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

#[no_mangle]
pub unsafe extern "C" fn draw_room(rp: *mut CRoom) {
	if rp.is_null() {
		return;
	}

	if ((*rp).r_flags & ISGONE) != 0 {
		return;
	}

	if ((*rp).r_flags & ISMAZE) != 0 {
		rogue_do_maze(rp);
		return;
	}

	if let Some(room) = build_room_model(rp) {
		draw_room_ascii(&room);
	}
}

#[no_mangle]
pub unsafe extern "C" fn rogue_do_maze(rp: *mut CRoom) {
	if rp.is_null() {
		return;
	}

	if let Some(room) = build_maze_model(rp) {
		draw_maze_ascii(&room);
	}
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