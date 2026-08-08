use std::os::raw::{c_char, c_int, c_short};

use crate::draw::set_chat;
use crate::player::{CCoord, CPlace, CThing, CThingObject, CRoom};
use crate::rnd::rnd;

const ISGONE: c_short = 0o000002;
const ISMAZE: c_short = 0o000004;
const NUMLINES: usize = 24;
const NUMCOLS: usize = 80;
const F_PASS: u8 = 0x80;
const FLOOR: c_char = b'.' as c_char;
const H_WALL: c_char = b'-' as c_char;
const V_WALL: c_char = b'|' as c_char;

#[repr(C)]
#[derive(Copy, Clone)]
struct CSpot {
    nexits: c_int,
    exits: [CCoord; 4],
    used: c_int,
}

const ZERO_COORD: CCoord = CCoord { x: 0, y: 0 };
const ZERO_SPOT: CSpot = CSpot {
    nexits: 0,
    exits: [ZERO_COORD; 4],
    used: 0,
};

unsafe extern "C" {
    static mut places: [CPlace; 32 * 80];

    fn wake_monster(y: c_int, x: c_int);
    fn putpass(cp: *mut CCoord);
}

static mut MAXY: c_int = 0;
static mut MAXX: c_int = 0;
static mut STARTY: c_int = 0;
static mut STARTX: c_int = 0;
static mut MAZE: [[CSpot; (NUMCOLS / 3) + 1]; (NUMLINES / 3) + 1] =
    [[ZERO_SPOT; (NUMCOLS / 3) + 1]; (NUMLINES / 3) + 1];

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

#[inline]
unsafe fn maze_spot(y: c_int, x: c_int) -> *mut CSpot {
    &raw mut MAZE[y as usize][x as usize]
}

#[inline]
unsafe fn pass_present(maze_y: c_int, maze_x: c_int) -> bool {
    let tile = place_at(maze_y + STARTY, maze_x + STARTX);
    (((*tile).p_flags as u8) & F_PASS) != 0
}

unsafe fn accnt_maze_local(y: c_int, x: c_int, ny: c_int, nx: c_int) {
    let sp = maze_spot(y, x);
    let mut i = 0;
    while i < (*sp).nexits {
        let cp = &(*sp).exits[i as usize];
        if cp.y == ny && cp.x == nx {
            return;
        }
        i += 1;
    }
    if (*sp).nexits < 4 {
        let slot = (*sp).nexits as usize;
        (*sp).exits[slot].y = ny;
        (*sp).exits[slot].x = nx;
    }
}

unsafe fn dig_local(y: c_int, x: c_int) {
    let del = [
        CCoord { y: 2, x: 0 },
        CCoord { y: -2, x: 0 },
        CCoord { y: 0, x: 2 },
        CCoord { y: 0, x: -2 },
    ];

    loop {
        let mut cnt = 0;
        let mut nexty = 0;
        let mut nextx = 0;

        for cp in &del {
            let newy = y + cp.y;
            let newx = x + cp.x;

            if newy < 0 || newy > MAXY || newx < 0 || newx > MAXX {
                continue;
            }
            if pass_present(newy, newx) {
                continue;
            }

            cnt += 1;
            if rnd(cnt) == 0 {
                nexty = newy;
                nextx = newx;
            }
        }

        if cnt == 0 {
            return;
        }

        accnt_maze_local(y, x, nexty, nextx);
        accnt_maze_local(nexty, nextx, y, x);

        let mut pos = CCoord { y: 0, x: 0 };
        if nexty == y {
            pos.y = y + STARTY;
            if (nextx - x) < 0 {
                pos.x = nextx + STARTX + 1;
            } else {
                pos.x = nextx + STARTX - 1;
            }
        } else {
            pos.x = x + STARTX;
            if (nexty - y) < 0 {
                pos.y = nexty + STARTY + 1;
            } else {
                pos.y = nexty + STARTY - 1;
            }
        }
        putpass(&mut pos);

        pos.y = nexty + STARTY;
        pos.x = nextx + STARTX;
        putpass(&mut pos);

        dig_local(nexty, nextx);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rogue_draw_room(rp: *mut CRoom) {
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

    rogue_vert(rp, (*rp).r_pos.x);
    rogue_vert(rp, (*rp).r_pos.x + (*rp).r_max.x - 1);
    rogue_horiz(rp, (*rp).r_pos.y);
    rogue_horiz(rp, (*rp).r_pos.y + (*rp).r_max.y - 1);

    let mut y = (*rp).r_pos.y + 1;
    while y < (*rp).r_pos.y + (*rp).r_max.y - 1 {
        let mut x = (*rp).r_pos.x + 1;
        while x < (*rp).r_pos.x + (*rp).r_max.x - 1 {
            set_chat(y, x, FLOOR);
            x += 1;
        }
        y += 1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn rogue_vert(rp: *mut CRoom, startx: c_int) {
    if rp.is_null() {
        return;
    }

    let mut y = (*rp).r_pos.y + 1;
    let end = (*rp).r_pos.y + (*rp).r_max.y - 1;
    while y <= end {
        set_chat(y, startx, V_WALL);
        y += 1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn rogue_horiz(rp: *mut CRoom, starty: c_int) {
    if rp.is_null() {
        return;
    }

    let mut x = (*rp).r_pos.x;
    let end = (*rp).r_pos.x + (*rp).r_max.x - 1;
    while x <= end {
        set_chat(starty, x, H_WALL);
        x += 1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn rogue_do_maze(rp: *mut CRoom) {
    if rp.is_null() {
        return;
    }

    let mut my = 0;
    while my <= (NUMLINES / 3) as c_int {
        let mut mx = 0;
        while mx <= (NUMCOLS / 3) as c_int {
            let sp = maze_spot(my, mx);
            (*sp).used = 0;
            (*sp).nexits = 0;
            mx += 1;
        }
        my += 1;
    }

    MAXY = (*rp).r_max.y;
    MAXX = (*rp).r_max.x;
    STARTY = (*rp).r_pos.y;
    STARTX = (*rp).r_pos.x;

    let starty = (rnd((*rp).r_max.y) / 2) * 2;
    let startx = (rnd((*rp).r_max.x) / 2) * 2;

    let mut pos = CCoord {
        y: starty + STARTY,
        x: startx + STARTX,
    };
    putpass(&mut pos);
    dig_local(starty, startx);
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
