use std::os::raw::{c_char, c_int};

use crate::player::CPlace;

unsafe extern "C" {
	static mut places: [CPlace; 32 * 80];
}

#[inline]
pub(crate) unsafe fn place_at<T>(base: *mut T, y: c_int, x: c_int) -> *mut T {
	base.add(((x as usize) << 5) + (y as usize))
}

#[inline]
pub(crate) unsafe fn set_tile_char(y: c_int, x: c_int, ch: c_char) {
	(*place_at((&mut places) as *mut CPlace, y, x)).p_ch = ch;
}

/// Clear `flag` from the flat flags of the `places` cell at `(y, x)`.
#[inline]
pub(crate) unsafe fn clear_tile_flag(y: c_int, x: c_int, flag: c_char) {
	let pp = place_at((&mut places) as *mut CPlace, y, x);
	(*pp).p_flags = (((*pp).p_flags as u8) & !(flag as u8)) as c_char;
}

