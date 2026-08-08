use std::os::raw::{c_char, c_int};

use crate::player::CPlace;

unsafe extern "C" {
	static mut places: [CPlace; 32 * 80];
}

#[inline]
unsafe fn place_at(y: c_int, x: c_int) -> *mut CPlace {
	places.as_mut_ptr().add(((x as usize) << 5) + (y as usize))
}

#[inline]
pub(crate) unsafe fn fill_area_with_char(y: c_int, x: c_int, ch: c_char) {
	(*place_at(y, x)).p_ch = ch;
}
