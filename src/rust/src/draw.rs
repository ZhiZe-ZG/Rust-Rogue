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
