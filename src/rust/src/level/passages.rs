use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::draw::place_at;
use crate::player::{CCoord, CPlace, CRoom, CThingMonster};

const MAXROOMS: usize = 9;
const MAXPASS: usize = 13;
const NUMCOLS: c_int = 80;
const NUMLINES: c_int = 24;

const ISGONE: c_short = 0o000002;
const ISMAZE: c_short = 0o000004;

const PASSAGE: c_char = b'#' as c_char;
const DOOR: c_char = b'+' as c_char;
const F_PASS: c_char = 0x80u8 as c_char;
const F_REAL: c_char = 0x10u8 as c_char;
const F_PNUM: c_char = 0x0fu8 as c_char;
const F_SEEN: c_char = 0x40u8 as c_char;

const FALSE: c_uchar = 0;
const TRUE: c_uchar = 1;

#[repr(C)]
#[derive(Copy, Clone)]
struct RDes {
    conn: [c_uchar; MAXROOMS],
    isconn: [c_uchar; MAXROOMS],
    ingraph: c_uchar,
}

static mut PNUM: c_int = 0;
static mut NEW_PNUM: c_uchar = FALSE;

unsafe extern "C" {
    static mut level: c_int;
    static mut rooms: [CRoom; MAXROOMS];
    static mut passages: [CRoom; MAXPASS];
    static mut places: [CPlace; 32 * 80];

    fn rnd(range: c_int) -> c_int;
    fn msg(fmt: *const c_char, ...);
    fn r#move(y: c_int, x: c_int) -> c_int;
    fn addch(ch: c_uint) -> c_int;
    fn standout() -> c_int;
    fn standend() -> c_int;
}

#[inline]
unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_ch
}

#[inline]
unsafe fn flat_at(y: c_int, x: c_int) -> c_char {
    (*place_at((&raw mut places) as *mut CPlace, y, x)).p_flags
}

#[inline]
unsafe fn clear_flat_flag(y: c_int, x: c_int, flag: c_char) {
    let pp = place_at((&raw mut places) as *mut CPlace, y, x);
    (*pp).p_flags = (((*pp).p_flags as u8) & !(flag as u8)) as c_char;
}

#[inline]
unsafe fn coord_eq(a: CCoord, b: CCoord) -> bool {
    a.x == b.x && a.y == b.y
}

pub(super) unsafe fn do_passages() {
    let mut rdes: [RDes; MAXROOMS] = [
        RDes { conn: [0, 1, 0, 1, 0, 0, 0, 0, 0], isconn: [0; MAXROOMS], ingraph: 0 },
        RDes { conn: [1, 0, 1, 0, 1, 0, 0, 0, 0], isconn: [0; MAXROOMS], ingraph: 0 },
        RDes { conn: [0, 1, 0, 0, 0, 1, 0, 0, 0], isconn: [0; MAXROOMS], ingraph: 0 },
        RDes { conn: [1, 0, 0, 0, 1, 0, 1, 0, 0], isconn: [0; MAXROOMS], ingraph: 0 },
        RDes { conn: [0, 1, 0, 1, 0, 1, 0, 1, 0], isconn: [0; MAXROOMS], ingraph: 0 },
        RDes { conn: [0, 0, 1, 0, 1, 0, 0, 0, 1], isconn: [0; MAXROOMS], ingraph: 0 },
        RDes { conn: [0, 0, 0, 1, 0, 0, 0, 1, 0], isconn: [0; MAXROOMS], ingraph: 0 },
        RDes { conn: [0, 0, 0, 0, 1, 0, 1, 0, 1], isconn: [0; MAXROOMS], ingraph: 0 },
        RDes { conn: [0, 0, 0, 0, 0, 1, 0, 1, 0], isconn: [0; MAXROOMS], ingraph: 0 },
    ];

    for r1 in rdes.iter_mut() {
        for j in 0..MAXROOMS {
            r1.isconn[j] = 0;
        }
        r1.ingraph = 0;
    }

    let mut roomcount = 1;
    let mut r1_idx = rnd(MAXROOMS as c_int) as usize;
    rdes[r1_idx].ingraph = 1;

    loop {
        let mut j = 0;
        let mut r2_idx = None;
        for i in 0..MAXROOMS {
            if rdes[r1_idx].conn[i] != 0 && rdes[i].ingraph == 0 {
                j += 1;
                if rnd(j as c_int) == 0 {
                    r2_idx = Some(i);
                }
            }
        }

        if j == 0 {
            loop {
                r1_idx = rnd(MAXROOMS as c_int) as usize;
                if rdes[r1_idx].ingraph != 0 {
                    break;
                }
            }
        } else {
            let idx = r2_idx.unwrap();
            rdes[idx].ingraph = 1;
            conn(r1_idx as c_int, idx as c_int);
            rdes[r1_idx].isconn[idx] = 1;
            rdes[idx].isconn[r1_idx] = 1;
            roomcount += 1;
        }

        if roomcount >= MAXROOMS as c_int {
            break;
        }
    }

    let mut roomcount = rnd(5);
    while roomcount > 0 {
        r1_idx = rnd(MAXROOMS as c_int) as usize;
        let mut j = 0;
        let mut r2_idx = None;
        for i in 0..MAXROOMS {
            if rdes[r1_idx].conn[i] != 0 && rdes[r1_idx].isconn[i] == 0 {
                j += 1;
                if rnd(j as c_int) == 0 {
                    r2_idx = Some(i);
                }
            }
        }

        if j != 0 {
            let idx = r2_idx.unwrap();
            conn(r1_idx as c_int, idx as c_int);
            rdes[r1_idx].isconn[idx] = 1;
            rdes[idx].isconn[r1_idx] = 1;
        }

        roomcount -= 1;
    }

    passnum();
}

unsafe fn conn(r1: c_int, r2: c_int) {
    let mut rmt: c_int = 0;
    let mut distance = 0;
    let mut turn_spot;
    let mut turn_distance = 0;
    let mut direc = 'd';
    let rm: usize;

    let mut del = CCoord { x: 0, y: 0 };
    let mut curr = CCoord { x: 0, y: 0 };
    let mut turn_delta = CCoord { x: 0, y: 0 };
    let mut spos = CCoord { x: 0, y: 0 };
    let mut epos = CCoord { x: 0, y: 0 };

    if r1 < r2 {
        rm = r1 as usize;
        if r1 + 1 == r2 {
            direc = 'r';
        }
    } else {
        rm = r2 as usize;
        if r2 + 1 == r1 {
            direc = 'r';
        }
    }

    let rpf = &mut rooms[rm];

    if direc == 'd' {
        rmt = rm as c_int + 3;
        let rpt = &mut rooms[rmt as usize];
        del.x = 0;
        del.y = 1;
        spos.x = rpf.r_pos.x;
        spos.y = rpf.r_pos.y;
        epos.x = rpt.r_pos.x;
        epos.y = rpt.r_pos.y;
        if (rpf.r_flags & ISGONE) == 0 {
            loop {
                spos.x = rpf.r_pos.x + rnd(rpf.r_max.x - 2) + 1;
                spos.y = rpf.r_pos.y + rpf.r_max.y - 1;
                if (rpf.r_flags & ISMAZE) == 0 || (flat_at(spos.y, spos.x) as u8 & F_PASS as u8) != 0 {
                    break;
                }
            }
        }
        if (rpt.r_flags & ISGONE) == 0 {
            loop {
                epos.x = rpt.r_pos.x + rnd(rpt.r_max.x - 2) + 1;
                if (rpt.r_flags & ISMAZE) == 0 || (flat_at(epos.y, epos.x) as u8 & F_PASS as u8) != 0 {
                    break;
                }
            }
        }
        distance = (spos.y - epos.y).abs() - 1;
        turn_delta.y = 0;
        turn_delta.x = if spos.x < epos.x { 1 } else { -1 };
        turn_distance = (spos.x - epos.x).abs();
    } else if direc == 'r' {
        rmt = rm as c_int + 1;
        let rpt = &mut rooms[rmt as usize];
        del.x = 1;
        del.y = 0;
        spos.x = rpf.r_pos.x;
        spos.y = rpf.r_pos.y;
        epos.x = rpt.r_pos.x;
        epos.y = rpt.r_pos.y;
        if (rpf.r_flags & ISGONE) == 0 {
            loop {
                spos.x = rpf.r_pos.x + rpf.r_max.x - 1;
                spos.y = rpf.r_pos.y + rnd(rpf.r_max.y - 2) + 1;
                if (rpf.r_flags & ISMAZE) == 0 || (flat_at(spos.y, spos.x) as u8 & F_PASS as u8) != 0 {
                    break;
                }
            }
        }
        if (rpt.r_flags & ISGONE) == 0 {
            loop {
                epos.y = rpt.r_pos.y + rnd(rpt.r_max.y - 2) + 1;
                if (rpt.r_flags & ISMAZE) == 0 || (flat_at(epos.y, epos.x) as u8 & F_PASS as u8) != 0 {
                    break;
                }
            }
        }
        distance = (spos.x - epos.x).abs() - 1;
        turn_delta.y = if spos.y < epos.y { 1 } else { -1 };
        turn_delta.x = 0;
        turn_distance = (spos.y - epos.y).abs();
    }

    if distance > 1 {
        turn_spot = rnd(distance - 1) + 1;
    } else {
        turn_spot = 1;
    }

    if (rpf.r_flags & ISGONE) == 0 {
        door(rpf, &mut spos);
    } else {
        putpass(&mut spos);
    }

    let rpt = &rooms[rmt as usize];
    if (rpt.r_flags & ISGONE) == 0 {
        door(&raw const rooms[rmt as usize] as *mut CRoom, &mut epos);
    } else {
        putpass(&mut epos);
    }

    curr.x = spos.x;
    curr.y = spos.y;
    while distance > 0 {
        curr.x += del.x;
        curr.y += del.y;
        if distance == turn_spot {
            let mut remaining = turn_distance;
            while remaining > 0 {
                putpass(&mut curr);
                curr.x += turn_delta.x;
                curr.y += turn_delta.y;
                remaining -= 1;
            }
        }
        putpass(&mut curr);
        distance -= 1;
    }

    curr.x += del.x;
    curr.y += del.y;
    if !coord_eq(curr, epos) {
        msg(b"warning, connectivity problem on this level\0".as_ptr() as *const c_char);
    }
}

pub(super) unsafe fn putpass(cp: *mut CCoord) {
    if cp.is_null() {
        return;
    }

    let pp = place_at((&raw mut places) as *mut CPlace, (*cp).y, (*cp).x);
    (*pp).p_flags = (((*pp).p_flags as u8) | (F_PASS as u8)) as c_char;
    if rnd(10) + 1 < level && rnd(40) == 0 {
        clear_flat_flag((*cp).y, (*cp).x, F_REAL);
    } else {
        (*pp).p_ch = PASSAGE;
    }
}

unsafe fn door(rm: *mut CRoom, cp: *mut CCoord) {
    if rm.is_null() || cp.is_null() {
        return;
    }

    let rm_ref = &mut *rm;
    rm_ref.r_exit[rm_ref.r_nexits as usize] = *cp;
    rm_ref.r_nexits += 1;

    if (rm_ref.r_flags & ISMAZE) != 0 {
        return;
    }

    let pp = place_at((&raw mut places) as *mut CPlace, (*cp).y, (*cp).x);
    if rnd(10) + 1 < level && rnd(5) == 0 {
        if (*cp).y == rm_ref.r_pos.y || (*cp).y == rm_ref.r_pos.y + rm_ref.r_max.y - 1 {
            (*pp).p_ch = b'-' as c_char;
        } else {
            (*pp).p_ch = b'|' as c_char;
        }
        clear_flat_flag((*cp).y, (*cp).x, F_REAL);
    } else {
        (*pp).p_ch = DOOR;
    }
}

#[no_mangle]
pub unsafe extern "C" fn add_pass() {
    for y in 1..NUMLINES - 1 {
        for x in 0..NUMCOLS {
            let pp = place_at((&raw mut places) as *mut CPlace, y, x);
            let flags = (*pp).p_flags;
            let ch = (*pp).p_ch;
            if (((flags as u8) & (F_PASS as u8)) != 0)
                || ch == DOOR
                || (((flags as u8) & (F_REAL as u8)) == 0 && (ch == b'|' as c_char || ch == b'-' as c_char))
            {
                let mut out_ch = ch;
                if ((flags as u8) & (F_PASS as u8)) != 0 {
                    out_ch = PASSAGE;
                }
                (*pp).p_flags = (((*pp).p_flags as u8) | (F_SEEN as u8)) as c_char;
                r#move(y, x);
                if !(*pp).p_monst.is_null() {
                    let monst = (*pp).p_monst as *mut CThingMonster;
                    (*monst).t_oldch = (*pp).p_ch;
                } else if ((flags as u8) & (F_REAL as u8)) != 0 {
                    addch(out_ch as c_uint);
                } else {
                    standout();
                    addch(if (flags as u8) & (F_PASS as u8) != 0 { PASSAGE as c_uint } else { DOOR as c_uint });
                    standend();
                }
            }
        }
    }
}

unsafe fn passnum() {
    PNUM = 0;
    NEW_PNUM = FALSE;
    for rp in &mut passages[..MAXPASS] {
        rp.r_nexits = 0;
    }
    for rp in &mut rooms[..MAXROOMS] {
        for i in 0..rp.r_nexits as usize {
            NEW_PNUM = TRUE;
            numpass(rp.r_exit[i].y, rp.r_exit[i].x);
        }
    }
}

unsafe fn numpass(y: c_int, x: c_int) {
    if x >= NUMCOLS || x < 0 || y >= NUMLINES || y <= 0 {
        return;
    }

    let pp = place_at((&raw mut places) as *mut CPlace, y, x);
    if ((*pp).p_flags as u8 & F_PNUM as u8) != 0 {
        return;
    }
    if NEW_PNUM != 0 {
        PNUM += 1;
        NEW_PNUM = FALSE;
    }

    let ch = chat_at(y, x);
    if ch == DOOR || (((flat_at(y, x) as u8) & (F_REAL as u8)) == 0 && (ch == b'|' as c_char || ch == b'-' as c_char)) {
        let rp = &mut passages[PNUM as usize];
        rp.r_exit[rp.r_nexits as usize].y = y;
        rp.r_exit[rp.r_nexits as usize].x = x;
        rp.r_nexits += 1;
    } else if ((flat_at(y, x) as u8) & (F_PASS as u8)) == 0 {
        return;
    }

    (*pp).p_flags = (((*pp).p_flags as u8) | (PNUM as u8)) as c_char;
    numpass(y + 1, x);
    numpass(y - 1, x);
    numpass(y, x + 1);
    numpass(y, x - 1);
}
