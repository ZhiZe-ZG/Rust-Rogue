use std::os::raw::{c_char, c_int, c_short};

use crate::player::{CCoord, CPlace, CThing, CThingObject, CRoom};

const ISGONE: c_short = 0o000002;

unsafe extern "C" {
    static mut places: [CPlace; 32 * 80];

    fn wake_monster(y: c_int, x: c_int);
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

#[inline]
unsafe fn place_at(y: c_int, x: c_int) -> *mut CPlace {
    places.as_mut_ptr().add(((x as usize) << 5) + (y as usize))
}

#[inline]
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    (*place_at(y, x)).p_ch
}

#[inline]
unsafe fn winat(y: c_int, x: c_int) -> c_char {
    let tp = (*place_at(y, x)).p_monst;
    if tp.is_null() {
        chat_at(y, x)
    } else {
        (*thing_o(tp)).o_packch
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
