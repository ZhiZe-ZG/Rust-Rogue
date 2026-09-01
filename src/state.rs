/*
 * state.rs - Portable Rogue Save State Code
 *
 * Ported from state.c to Rust.
 *
 * Copyright (C) 1999, 2000, 2005 Nicholas J. Kisseberth
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 * 3. Neither the name(s) of the author(s) nor the names of other contributors
 *    may be used to endorse or promote products derived from this software
 *    without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR(S) AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR(S) OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 */

use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint, c_ushort, c_void};

use crate::chase::runners;
use crate::daemon::CDelayedAction;
use crate::daemons::{doctor, nohaste, rollwand, sight, stomach, swander, unconfuse, unsee};
use crate::player::{CCoord, CPlace, CRoom, CStats, CThing, CThingMonster, CThingObject};
use crate::things::CObjInfo;

// ─── Constants ───────────────────────────────────────────────────────────────

const RSID_STATS: c_int = 0xABCD0001u32 as c_int;
const RSID_THING: c_int = 0xABCD0002u32 as c_int;
const RSID_THING_NULL: c_int = 0xDEAD0002u32 as c_int;
const RSID_OBJECT: c_int = 0xABCD0003u32 as c_int;
const RSID_MAGICITEMS: c_int = 0xABCD0004u32 as c_int;
const RSID_KNOWS: c_int = 0xABCD0005u32 as c_int;
const RSID_GUESSES: c_int = 0xABCD0006u32 as c_int;
const RSID_OBJECTLIST: c_int = 0xABCD0007u32 as c_int;
const RSID_BAGOBJECT: c_int = 0xABCD0008u32 as c_int;
const RSID_MONSTERLIST: c_int = 0xABCD0009u32 as c_int;
const RSID_MONSTERSTATS: c_int = 0xABCD000Au32 as c_int;
const RSID_MONSTERS: c_int = 0xABCD000Bu32 as c_int;
const RSID_TRAP: c_int = 0xABCD000Cu32 as c_int;
const RSID_WINDOW: c_int = 0xABCD000Du32 as c_int;
const RSID_DAEMONS: c_int = 0xABCD000Eu32 as c_int;
const RSID_IWEAPS: c_int = 0xABCD000Fu32 as c_int;
const RSID_IARMOR: c_int = 0xABCD0010u32 as c_int;
const RSID_SPELLS: c_int = 0xABCD0011u32 as c_int;
const RSID_ILIST: c_int = 0xABCD0012u32 as c_int;
const RSID_HLIST: c_int = 0xABCD0013u32 as c_int;
const RSID_DEATHTYPE: c_int = 0xABCD0014u32 as c_int;
const RSID_CTYPES: c_int = 0xABCD0015u32 as c_int;
const RSID_COORDLIST: c_int = 0xABCD0016u32 as c_int;
const RSID_ROOMS: c_int = 0xABCD0017u32 as c_int;

const MAXSTR: usize = 1024;
const MAXLINES: c_int = 24;
const MAXCOLS: c_int = 80;

const MAXARMORS: usize = 8;
const MAXPOTIONS: usize = 14;
const MAXRINGS: usize = 14;
const MAXSCROLLS: usize = 18;
const MAXSTICKS: usize = 14;
const NUMTHINGS: usize = 7;
const MAXWEAPONS: usize = 9;
const MAXROOMS: usize = 9;
const MAXPASS: usize = 13;
const MAXDAEMONS: usize = 20;
const MAXMONSTERS: usize = 26;

/// `#ifdef MASTER` helper: replaced by a plain `const` so the preprocessor
/// conditional disappears.  The autoconf build (`configure.ac`) defines MASTER,
/// so wizard-mode and the `total` counter are saved/restored here too.
const MASTER: bool = true;

// ─── Module state (mirrors C statics) ────────────────────────────────────────

static mut READ_ERROR: c_int = 0;
static mut WRITE_ERROR: c_int = 0;
static mut FORMAT_ERROR: c_int = 0;
static ENDIAN: c_int = 0x01020304;

#[inline]
unsafe fn big_endian() -> bool {
    *((&raw const ENDIAN) as *const u8) == 0x01
}

#[inline]
unsafe fn read_stat() -> c_int {
    if FORMAT_ERROR != 0 || READ_ERROR != 0 {
        1
    } else {
        0
    }
}

// ─── C ABI mirror types ──────────────────────────────────────────────────────

#[repr(C)]
pub struct CFile {
    _private: [u8; 0],
}

#[repr(C)]
pub struct CWindow {
    _private: [u8; 0],
}

#[repr(C)]
pub struct CStone {
    pub st_name: *const c_char,
    pub st_value: c_int,
}

/// Local layout mirror of the C `struct monster` with `player::CStats`
/// (the `monsters::CStats` type is distinct and would not type-check here).
#[repr(C)]
struct CMonsterState {
    m_name: *mut c_char,
    m_carry: c_int,
    m_flags: c_short,
    m_stats: CStats,
}

/// Delayed-action callback slot type (same representation as `daemon::DFunc`).
type DFunc = Option<unsafe extern "C" fn(c_int)>;

// ─── Extern C globals (defined in vers.c) ────────────────────────────────────

unsafe extern "C" {
    // booleans (C bool -> c_uchar)
    static mut after: c_uchar;
    static mut again: c_uchar;
    static mut noscore: c_int;
    static mut seenstairs: c_uchar;
    static mut amulet: c_uchar;
    static mut door_stop: c_uchar;
    static mut fight_flush: c_uchar;
    static mut firstmove: c_uchar;
    static mut got_ltc: c_uchar;
    static mut has_hit: c_uchar;
    static mut in_shell: c_uchar;
    static mut inv_describe: c_uchar;
    static mut jump: c_uchar;
    static mut kamikaze: c_uchar;
    static mut lower_msg: c_uchar;
    static mut move_on: c_uchar;
    static mut msg_esc: c_uchar;
    static mut passgo: c_uchar;
    static mut playing: c_uchar;
    static mut q_comm: c_uchar;
    static mut running: c_uchar;
    static mut save_msg: c_uchar;
    static mut see_floor: c_uchar;
    static mut stat_msg: c_uchar;
    static mut terse: c_uchar;
    static mut to_death: c_uchar;
    static mut tombstone: c_uchar;
    static mut wizard: c_int;
    static mut pack_used: [c_uchar; 26];

    // chars
    static mut dir_ch: c_char;
    static mut file_name: [c_char; MAXSTR];
    static mut huh: [c_char; MAXSTR];
    static mut p_colors: [*mut c_char; MAXPOTIONS];
    static mut prbuf: [c_char; 2 * MAXSTR];
    static mut r_stones: [*mut c_char; MAXRINGS];
    static mut runch: c_char;
    static mut s_names: [*mut c_char; MAXSCROLLS];
    static mut take: c_char;
    static mut whoami: [c_char; MAXSTR];
    static mut ws_made: [*mut c_char; MAXSTICKS];
    static mut ws_type: [*mut c_char; MAXSTICKS];

    static mut orig_dsusp: c_int;
    static mut fruit: [c_char; MAXSTR];
    static mut home: [c_char; MAXSTR];
    static mut inv_t_name: [*mut c_char; 3];
    static mut l_last_comm: c_char;
    static mut l_last_dir: c_char;
    static mut last_comm: c_char;
    static mut last_dir: c_char;
    static mut tr_name: [*mut c_char; 8];
    static mut release: *mut c_char;

    // ints
    static mut n_objs: c_int;
    static mut ntraps: c_int;
    static mut hungry_state: c_int;
    static mut inpack: c_int;
    static mut inv_type: c_int;
    static mut level: c_int;
    static mut max_level: c_int;
    static mut mpos: c_int;
    static mut no_food: c_int;
    static mut a_class: [c_int; MAXARMORS];
    #[link_name = "count"]
    static mut COUNT: c_int;
    static mut food_left: c_int;
    static mut lastscore: c_int;
    static mut no_command: c_int;
    static mut no_move: c_int;
    static mut purse: c_int;
    static mut quiet: c_int;
    static mut vf_hit: c_int;
    static mut dnum: c_int;
    static mut seed: c_int;
    static mut e_levels: [c_int; 21];

    // coords
    static mut delta: CCoord;
    static mut oldpos: CCoord;
    static mut stairs: CCoord;

    // player / lists
    static mut player: CThing;
    static mut cur_armor: *mut CThing;
    static mut cur_ring: [*mut CThing; 2];
    static mut cur_weapon: *mut CThing;
    static mut l_last_pick: *mut CThing;
    static mut last_pick: *mut CThing;
    static mut lvl_obj: *mut CThing;
    static mut mlist: *mut CThing;

    // rooms / map
    static mut places: [CPlace; 32 * 80];
    static mut max_stats: CStats;
    static mut oldrp: *mut CRoom;
    static mut rooms: [CRoom; MAXROOMS];
    static mut passages: [CRoom; MAXPASS];

    // monster / object info tables
    static mut monsters: [CMonsterState; MAXMONSTERS];
    static mut things: [CObjInfo; NUMTHINGS];
    static mut arm_info: [CObjInfo; MAXARMORS];
    static mut pot_info: [CObjInfo; MAXPOTIONS];
    static mut ring_info: [CObjInfo; MAXRINGS];
    static mut scr_info: [CObjInfo; MAXSCROLLS];
    static mut weap_info: [CObjInfo; MAXWEAPONS + 1];
    static mut ws_info: [CObjInfo; MAXSTICKS];

    // daemons (defined in daemon.rs as `d_list`) and misc C-visible globals
    static mut d_list: [CDelayedAction; MAXDAEMONS];
    static mut total: c_int;
    static mut between: c_int;
    static mut nh: CCoord;
    static mut group: c_int;
    static mut stdscr: *mut c_void;

    // material arrays (defined in init.rs as rainbow/stones/wood/metal)
    static mut rainbow: [*mut c_char; 27];
    static stones: [CStone; 26];
    static mut wood: [*mut c_char; 33];
    static mut metal: [*mut c_char; 22];
    static mut cNCOLORS: c_int;
    static mut cNSTONES: c_int;
    static mut cNWOOD: c_int;
    static mut cNMETAL: c_int;

    // libc / curses
    fn new_item() -> *mut CThing;
    fn malloc(size: usize) -> *mut c_void;
    fn fwrite(ptr: *const u8, size: usize, nmemb: usize, stream: *mut CFile) -> usize;
    fn fread(ptr: *mut u8, size: usize, n: usize, stream: *mut CFile) -> usize;
    fn strlen(s: *const c_char) -> usize;
    fn strcmp(a: *const c_char, b: *const c_char) -> c_int;
    fn getmaxx(win: *mut CWindow) -> c_int;
    fn getmaxy(win: *mut CWindow) -> c_int;
    fn mvwinch(win: *mut CWindow, y: c_int, x: c_int) -> c_int;
    fn mvwaddch(win: *mut CWindow, y: c_int, x: c_int, ch: c_int) -> c_int;
}

// ─── Helpers ────────────────────────────────────────────────────────────────

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

/// Convert a ZST/single-address `extern "C" fn()` daemon callback into the
/// `fn(c_int)` slot representation used by `d_list` (matches daemon.rs's
/// transmute convention for storing C void* pointers).
#[inline]
unsafe fn fn_to_dfunc(f: unsafe extern "C" fn()) -> DFunc {
    std::mem::transmute::<unsafe extern "C" fn(), DFunc>(f)
}

// ─── Low-level primitives ────────────────────────────────────────────────────

#[inline]
unsafe fn rs_write(savef: *mut CFile, ptr: *const c_void, size: usize) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    if fwrite(ptr as *const u8, 1, size, savef) != size {
        WRITE_ERROR = 1;
    }

    WRITE_ERROR
}

#[inline]
unsafe fn rs_read(inf: *mut CFile, ptr: *mut u8, size: usize) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    if fread(ptr, 1, size, inf) != size {
        READ_ERROR = 1;
    }

    read_stat()
}

unsafe fn rs_write_int(savef: *mut CFile, c: c_int) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    if big_endian() {
        let src = (&raw const c) as *const u8;
        let bytes = [*src.add(3), *src.add(2), *src.add(1), *src.add(0)];
        rs_write(savef, bytes.as_ptr() as *const c_void, 4);
    } else {
        rs_write(savef, (&raw const c) as *const c_void, 4);
    }

    WRITE_ERROR
}

unsafe fn rs_read_int(inf: *mut CFile, i: *mut c_int) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut input: c_int = 0;
    let _ = rs_read(inf, (&mut input as *mut c_int) as *mut u8, 4);

    if big_endian() {
        let src = (&raw const input) as *const u8;
        let bytes = [*src.add(3), *src.add(2), *src.add(1), *src.add(0)];
        *i = i32::from_ne_bytes(bytes);
    } else {
        *i = input;
    }

    read_stat()
}

unsafe fn rs_write_char(savef: *mut CFile, c: c_char) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    rs_write(savef, (&raw const c) as *const c_void, 1);

    WRITE_ERROR
}

unsafe fn rs_read_char(inf: *mut CFile, c: *mut c_char) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read(inf, c as *mut u8, 1);

    read_stat()
}

unsafe fn rs_write_chars(savef: *mut CFile, c: *mut c_char, count: c_int) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, count);
    if count > 0 {
        let _ = rs_write(savef, c as *const c_void, count as usize);
    }

    WRITE_ERROR
}

unsafe fn rs_read_chars(inf: *mut CFile, i: *mut c_char, count: c_int) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut value);

    if value != count {
        FORMAT_ERROR = 1;
    }

    if count > 0 {
        let _ = rs_read(inf, i as *mut u8, count as usize);
    }

    read_stat()
}

unsafe fn rs_write_ints(savef: *mut CFile, c: *mut c_int, count: c_int) -> c_int {
    let mut n: c_int = 0;

    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, count);

    while n < count {
        if rs_write_int(savef, *c.add(n as usize)) != 0 {
            break;
        }
        n += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_ints(inf: *mut CFile, i: *mut c_int, count: c_int) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut value);

    if value != count {
        FORMAT_ERROR = 1;
    }

    let mut n: c_int = 0;
    while n < count {
        if rs_read_int(inf, &mut *i.add(n as usize)) != 0 {
            break;
        }
        n += 1;
    }

    read_stat()
}

unsafe fn rs_write_boolean(savef: *mut CFile, c: c_int) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let buf: u8 = if c == 0 { 0 } else { 1 };
    rs_write(savef, (&raw const buf) as *const c_void, 1);

    WRITE_ERROR
}

unsafe fn rs_read_boolean(inf: *mut CFile, i: *mut c_uchar) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut buf: u8 = 0;
    let _ = rs_read(inf, (&mut buf) as *mut u8, 1);

    *i = if buf != 0 { 1 } else { 0 };

    read_stat()
}

unsafe fn rs_write_booleans(savef: *mut CFile, c: *mut c_uchar, count: c_int) -> c_int {
    let mut n: c_int = 0;

    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, count);

    while n < count {
        if rs_write_boolean(savef, *c.add(n as usize) as c_int) != 0 {
            break;
        }
        n += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_booleans(inf: *mut CFile, i: *mut c_uchar, count: c_int) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut value);

    if value != count {
        FORMAT_ERROR = 1;
    }

    let mut n: c_int = 0;
    while n < count {
        if rs_read_boolean(inf, &mut *i.add(n as usize)) != 0 {
            break;
        }
        n += 1;
    }

    read_stat()
}

unsafe fn rs_write_short(savef: *mut CFile, c: c_short) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    if big_endian() {
        let src = (&raw const c) as *const u8;
        let bytes = [*src.add(1), *src.add(0)];
        rs_write(savef, bytes.as_ptr() as *const c_void, 2);
    } else {
        rs_write(savef, (&raw const c) as *const c_void, 2);
    }

    WRITE_ERROR
}

unsafe fn rs_read_short(inf: *mut CFile, i: *mut c_short) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut input: c_short = 0;
    let _ = rs_read(inf, (&mut input as *mut c_short) as *mut u8, 2);

    if big_endian() {
        let src = (&raw const input) as *const u8;
        let bytes = [*src.add(1), *src.add(0)];
        *i = i16::from_ne_bytes(bytes);
    } else {
        *i = input;
    }

    read_stat()
}

unsafe fn rs_write_shorts(savef: *mut CFile, c: *mut c_short, count: c_int) -> c_int {
    let mut n: c_int = 0;

    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, count);

    while n < count {
        if rs_write_short(savef, *c.add(n as usize)) != 0 {
            break;
        }
        n += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_shorts(inf: *mut CFile, i: *mut c_short, count: c_int) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut value);

    if value != count {
        FORMAT_ERROR = 1;
    }

    // NOTE: mirrors the C loop bound (uses the read `value`, not `count`).
    let mut n: c_int = 0;
    while n < value {
        if rs_read_short(inf, &mut *i.add(n as usize)) != 0 {
            break;
        }
        n += 1;
    }

    read_stat()
}

unsafe fn rs_write_ushort(savef: *mut CFile, c: c_ushort) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    if big_endian() {
        let src = (&raw const c) as *const u8;
        let bytes = [*src.add(1), *src.add(0)];
        rs_write(savef, bytes.as_ptr() as *const c_void, 2);
    } else {
        rs_write(savef, (&raw const c) as *const c_void, 2);
    }

    WRITE_ERROR
}

unsafe fn rs_read_ushort(inf: *mut CFile, i: *mut c_ushort) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut input: c_ushort = 0;
    let _ = rs_read(inf, (&mut input as *mut c_ushort) as *mut u8, 2);

    if big_endian() {
        let src = (&raw const input) as *const u8;
        let bytes = [*src.add(1), *src.add(0)];
        *i = u16::from_ne_bytes(bytes);
    } else {
        *i = input;
    }

    read_stat()
}

unsafe fn rs_write_uint(savef: *mut CFile, c: c_uint) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    if big_endian() {
        let src = (&raw const c) as *const u8;
        let bytes = [*src.add(3), *src.add(2), *src.add(1), *src.add(0)];
        rs_write(savef, bytes.as_ptr() as *const c_void, 4);
    } else {
        rs_write(savef, (&raw const c) as *const c_void, 4);
    }

    WRITE_ERROR
}

unsafe fn rs_read_uint(inf: *mut CFile, i: *mut c_uint) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut input: c_uint = 0;
    let _ = rs_read(inf, (&mut input as *mut c_uint) as *mut u8, 4);

    if big_endian() {
        let src = (&raw const input) as *const u8;
        let bytes = [*src.add(3), *src.add(2), *src.add(1), *src.add(0)];
        *i = u32::from_ne_bytes(bytes);
    } else {
        *i = input;
    }

    read_stat()
}

unsafe fn rs_write_marker(savef: *mut CFile, id: c_int) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    rs_write_int(savef, id);

    WRITE_ERROR
}

unsafe fn rs_read_marker(inf: *mut CFile, id: c_int) -> c_int {
    let mut nid: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    if rs_read_int(inf, &mut nid) == 0 {
        if id != nid {
            FORMAT_ERROR = 1;
        }
    }

    read_stat()
}

// ─── Strings ─────────────────────────────────────────────────────────────────

unsafe fn rs_write_string(savef: *mut CFile, s: *const c_char) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let len: c_int = if s.is_null() { 0 } else { strlen(s) as c_int + 1 };

    let _ = rs_write_int(savef, len);
    let _ = rs_write_chars(savef, s as *mut c_char, len);

    WRITE_ERROR
}

unsafe fn rs_read_string(inf: *mut CFile, s: *mut c_char, max: c_int) -> c_int {
    let mut len: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut len);

    if len > max {
        FORMAT_ERROR = 1;
    }

    let _ = rs_read_chars(inf, s, len);

    read_stat()
}

unsafe fn rs_read_new_string(inf: *mut CFile, s: *mut *mut c_char) -> c_int {
    let mut len: c_int = 0;
    let mut buf: *mut c_char = std::ptr::null_mut();

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut len);

    if len == 0 {
        buf = std::ptr::null_mut();
    } else {
        buf = malloc(len as usize) as *mut c_char;

        if buf.is_null() {
            READ_ERROR = 1;
        }
    }

    let _ = rs_read_chars(inf, buf, len);

    *s = buf;

    read_stat()
}

unsafe fn rs_write_strings(savef: *mut CFile, s: *mut *mut c_char, count: c_int) -> c_int {
    let mut n: c_int = 0;

    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, count);

    while n < count {
        if rs_write_string(savef, *s.add(n as usize)) != 0 {
            break;
        }
        n += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_strings(inf: *mut CFile, s: *mut *mut c_char, count: c_int, max: c_int) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut value);

    if value != count {
        FORMAT_ERROR = 1;
    }

    let mut n: c_int = 0;
    while n < count {
        if rs_read_string(inf, *s.add(n as usize), max) != 0 {
            break;
        }
        n += 1;
    }

    read_stat()
}

unsafe fn rs_read_new_strings(inf: *mut CFile, s: *mut *mut c_char, count: c_int) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut value);

    if value != count {
        FORMAT_ERROR = 1;
    }

    let mut n: c_int = 0;
    while n < count {
        if rs_read_new_string(inf, &mut *s.add(n as usize)) != 0 {
            break;
        }
        n += 1;
    }

    read_stat()
}

unsafe fn rs_write_string_index(
    savef: *mut CFile,
    master: *mut *mut c_char,
    max: c_int,
    s: *const c_char,
) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let mut i: c_int = 0;
    while i < max {
        if s == *master.add(i as usize) {
            return rs_write_int(savef, i);
        }
        i += 1;
    }

    rs_write_int(savef, -1)
}

unsafe fn rs_read_string_index(
    inf: *mut CFile,
    master: *mut *mut c_char,
    maxindex: c_int,
    s: *mut *mut c_char,
) -> c_int {
    let mut i: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut i);

    if i > maxindex {
        FORMAT_ERROR = 1;
    } else if i >= 0 {
        *s = *master.add(i as usize);
    } else {
        *s = std::ptr::null_mut();
    }

    read_stat()
}

unsafe fn rs_write_str_t(savef: *mut CFile, st: c_uint) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    rs_write_uint(savef, st);

    WRITE_ERROR
}

unsafe fn rs_read_str_t(inf: *mut CFile, st: *mut c_uint) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    rs_read_uint(inf, st);

    read_stat()
}

// ─── Coords / windows ────────────────────────────────────────────────────────

unsafe fn rs_write_coord(savef: *mut CFile, c: CCoord) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, c.x);
    let _ = rs_write_int(savef, c.y);

    WRITE_ERROR
}

unsafe fn rs_read_coord(inf: *mut CFile, c: *mut CCoord) -> c_int {
    let mut in_coord: CCoord = CCoord { x: 0, y: 0 };

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut in_coord.x);
    let _ = rs_read_int(inf, &mut in_coord.y);

    if read_stat() == 0 {
        (*c).x = in_coord.x;
        (*c).y = in_coord.y;
    }

    read_stat()
}

unsafe fn rs_write_window(savef: *mut CFile, win: *mut CWindow) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let width = getmaxx(win);
    let height = getmaxy(win);

    let _ = rs_write_marker(savef, RSID_WINDOW);
    let _ = rs_write_int(savef, height);
    let _ = rs_write_int(savef, width);

    let mut row: c_int = 0;
    while row < height {
        let mut col: c_int = 0;
        while col < width {
            if rs_write_int(savef, mvwinch(win, row, col)) != 0 {
                return WRITE_ERROR;
            }
            col += 1;
        }
        row += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_window(inf: *mut CFile, win: *mut CWindow) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let width = getmaxx(win);
    let height = getmaxy(win);

    let _ = rs_read_marker(inf, RSID_WINDOW);

    let mut maxlines: c_int = 0;
    let mut maxcols: c_int = 0;
    let _ = rs_read_int(inf, &mut maxlines);
    let _ = rs_read_int(inf, &mut maxcols);

    let mut row: c_int = 0;
    while row < maxlines {
        let mut col: c_int = 0;
        while col < maxcols {
            let mut value: c_int = 0;
            if rs_read_int(inf, &mut value) != 0 {
                return read_stat();
            }

            if row < height && col < width {
                let _ = mvwaddch(win, row, col, value);
            }
            col += 1;
        }
        row += 1;
    }

    read_stat()
}

// ─── List helpers ────────────────────────────────────────────────────────────

unsafe fn get_list_item(mut l: *mut CThing, i: c_int) -> *mut CThing {
    let mut count: c_int = 0;

    while !l.is_null() {
        if count == i {
            return l;
        }
        count += 1;
        l = (*thing_t(l)).l_next;
    }

    std::ptr::null_mut()
}

unsafe fn find_list_ptr(mut l: *mut CThing, ptr: *const c_void) -> c_int {
    let mut count: c_int = 0;

    while !l.is_null() {
        if l as *const c_void == ptr {
            return count;
        }
        count += 1;
        l = (*thing_t(l)).l_next;
    }

    -1
}

unsafe fn list_size(mut l: *mut CThing) -> c_int {
    let mut count: c_int = 0;

    while !l.is_null() {
        count += 1;
        l = (*thing_t(l)).l_next;
    }

    count
}

// ─── Stats / stone / item tables ─────────────────────────────────────────────

unsafe fn rs_write_stats(savef: *mut CFile, s: *mut CStats) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_marker(savef, RSID_STATS);
    let _ = rs_write_str_t(savef, (*s).s_str);
    let _ = rs_write_int(savef, (*s).s_exp);
    let _ = rs_write_int(savef, (*s).s_lvl);
    let _ = rs_write_int(savef, (*s).s_arm);
    let _ = rs_write_int(savef, (*s).s_hpt);
    let _ = rs_write_chars(savef, (&raw mut (*s).s_dmg) as *mut c_char, 13);
    let _ = rs_write_int(savef, (*s).s_maxhp);

    WRITE_ERROR
}

unsafe fn rs_read_stats(inf: *mut CFile, s: *mut CStats) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_marker(inf, RSID_STATS);
    let _ = rs_read_str_t(inf, &raw mut (*s).s_str);
    let _ = rs_read_int(inf, &mut (*s).s_exp);
    let _ = rs_read_int(inf, &mut (*s).s_lvl);
    let _ = rs_read_int(inf, &mut (*s).s_arm);
    let _ = rs_read_int(inf, &mut (*s).s_hpt);
    let _ = rs_read_chars(inf, (&raw mut (*s).s_dmg) as *mut c_char, 13);
    let _ = rs_read_int(inf, &mut (*s).s_maxhp);

    read_stat()
}

unsafe fn rs_write_stone_index(
    savef: *mut CFile,
    master: *const CStone,
    max: c_int,
    s: *const c_char,
) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let mut i: c_int = 0;
    while i < max {
        if s == (*master.add(i as usize)).st_name {
            let _ = rs_write_int(savef, i);
            return WRITE_ERROR;
        }
        i += 1;
    }

    let _ = rs_write_int(savef, -1);

    WRITE_ERROR
}

unsafe fn rs_read_stone_index(
    inf: *mut CFile,
    master: *const CStone,
    maxindex: c_int,
    s: *mut *mut c_char,
) -> c_int {
    let mut i: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut i);

    if i > maxindex {
        FORMAT_ERROR = 1;
    } else if i >= 0 {
        *s = (*master.add(i as usize)).st_name as *mut c_char;
    } else {
        *s = std::ptr::null_mut();
    }

    read_stat()
}

/// Serializes the global scroll names to the save file.
///
/// Uses globals: s_names.
unsafe fn rs_write_scrolls(savef: *mut CFile) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let mut i = 0;
    while i < MAXSCROLLS {
        let _ = rs_write_string(savef, s_names[i]);
        i += 1;
    }

    read_stat()
}

/// Restores the global scroll names from the save file.
///
/// Uses globals: s_names.
unsafe fn rs_read_scrolls(inf: *mut CFile) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut i = 0;
    while i < MAXSCROLLS {
        let _ = rs_read_new_string(inf, &mut s_names[i]);
        i += 1;
    }

    read_stat()
}

/// Serializes the global potion colors to the save file.
///
/// Uses globals: rainbow, p_colors.
unsafe fn rs_write_potions(savef: *mut CFile) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let mut i = 0;
    while i < MAXPOTIONS {
        let _ = rs_write_string_index(savef, (&raw mut rainbow) as *mut *mut c_char, cNCOLORS, p_colors[i]);
        i += 1;
    }

    WRITE_ERROR
}

/// Restores the global potion colors from the save file.
///
/// Uses globals: rainbow, p_colors.
unsafe fn rs_read_potions(inf: *mut CFile) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut i = 0;
    while i < MAXPOTIONS {
        let _ = rs_read_string_index(inf, (&raw mut rainbow) as *mut *mut c_char, cNCOLORS, &mut p_colors[i]);
        i += 1;
    }

    read_stat()
}

/// Serializes the global ring stone settings to the save file.
///
/// Uses globals: stones, r_stones.
unsafe fn rs_write_rings(savef: *mut CFile) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let mut i = 0;
    while i < MAXRINGS {
        let _ = rs_write_stone_index(savef, (&raw const stones) as *const CStone, cNSTONES, r_stones[i]);
        i += 1;
    }

    WRITE_ERROR
}

/// Restores the global ring stone settings from the save file.
///
/// Uses globals: stones, r_stones.
unsafe fn rs_read_rings(inf: *mut CFile) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut i = 0;
    while i < MAXRINGS {
        let _ = rs_read_stone_index(inf, (&raw const stones) as *const CStone, cNSTONES, &mut r_stones[i]);
        i += 1;
    }

    read_stat()
}

/// Serializes the global wand/staff descriptions to the save file.
///
/// Uses globals: ws_type, ws_made, wood, metal.
unsafe fn rs_write_sticks(savef: *mut CFile) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let mut i = 0;
    while i < MAXSTICKS {
        if strcmp(ws_type[i], c"staff".as_ptr()) == 0 {
            let _ = rs_write_int(savef, 0);
            let _ = rs_write_string_index(savef, (&raw mut wood) as *mut *mut c_char, cNWOOD, ws_made[i]);
        } else {
            let _ = rs_write_int(savef, 1);
            let _ = rs_write_string_index(savef, (&raw mut metal) as *mut *mut c_char, cNMETAL, ws_made[i]);
        }
        i += 1;
    }

    WRITE_ERROR
}

/// Restores the global wand/staff descriptions from the save file.
///
/// Uses globals: ws_type, ws_made, wood, metal.
unsafe fn rs_read_sticks(inf: *mut CFile) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut i: c_int = 0;
    let mut list: c_int = 0;

    while i < MAXSTICKS as c_int {
        let _ = rs_read_int(inf, &mut list);

        if list == 0 {
            let _ = rs_read_string_index(inf, (&raw mut wood) as *mut *mut c_char, cNWOOD, &mut ws_made[i as usize]);
            ws_type[i as usize] = c"staff".as_ptr() as *mut c_char;
        } else {
            let _ = rs_read_string_index(inf, (&raw mut metal) as *mut *mut c_char, cNMETAL, &mut ws_made[i as usize]);
            ws_type[i as usize] = c"wand".as_ptr() as *mut c_char;
        }
        i += 1;
    }

    read_stat()
}

// ─── Daemons ─────────────────────────────────────────────────────────────────

unsafe fn rs_write_daemons(savef: *mut CFile, dl: *mut CDelayedAction, cnt: c_int) -> c_int {
    let mut i: c_int = 0;
    let mut func: c_int = 0;

    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_marker(savef, RSID_DAEMONS);
    let _ = rs_write_int(savef, cnt);

    while i < cnt {
        let f = (*dl.add(i as usize)).d_func;

        if f == fn_to_dfunc(rollwand) {
            func = 1;
        } else if f == fn_to_dfunc(doctor) {
            func = 2;
        } else if f == fn_to_dfunc(stomach) {
            func = 3;
        } else if f == fn_to_dfunc(runners) {
            func = 4;
        } else if f == fn_to_dfunc(swander) {
            func = 5;
        } else if f == fn_to_dfunc(nohaste) {
            func = 6;
        } else if f == fn_to_dfunc(unconfuse) {
            func = 7;
        } else if f == fn_to_dfunc(unsee) {
            func = 8;
        } else if f == fn_to_dfunc(sight) {
            func = 9;
        } else if f.is_none() {
            func = 0;
        } else {
            func = -1;
        }

        let _ = rs_write_int(savef, (*dl.add(i as usize)).d_type);
        let _ = rs_write_int(savef, func);
        let _ = rs_write_int(savef, (*dl.add(i as usize)).d_arg);
        let _ = rs_write_int(savef, (*dl.add(i as usize)).d_time);

        i += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_daemons(inf: *mut CFile, dl: *mut CDelayedAction, cnt: c_int) -> c_int {
    let mut i: c_int = 0;
    let mut func: c_int = 0;
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_marker(inf, RSID_DAEMONS);
    let _ = rs_read_int(inf, &mut value);

    if value > cnt {
        FORMAT_ERROR = 1;
    }

    while i < cnt {
        func = 0;
        let _ = rs_read_int(inf, &mut (*dl.add(i as usize)).d_type);
        let _ = rs_read_int(inf, &mut func);
        let _ = rs_read_int(inf, &mut (*dl.add(i as usize)).d_arg);
        let _ = rs_read_int(inf, &mut (*dl.add(i as usize)).d_time);

        (*dl.add(i as usize)).d_func = match func {
            1 => fn_to_dfunc(rollwand),
            2 => fn_to_dfunc(doctor),
            3 => fn_to_dfunc(stomach),
            4 => fn_to_dfunc(runners),
            5 => fn_to_dfunc(swander),
            6 => fn_to_dfunc(nohaste),
            7 => fn_to_dfunc(unconfuse),
            8 => fn_to_dfunc(unsee),
            9 => fn_to_dfunc(sight),
            _ => None,
        };

        i += 1;
    }

    // Mirror the C sentinel cleanup; guarded so we never touch a slot past
    // the end of the (20-entry) daemon table.
    if (cnt as usize) < MAXDAEMONS {
        let d = &mut *dl.add(cnt as usize);
        if d.d_func.is_none() {
            d.d_type = 0;
            d.d_arg = 0;
            d.d_time = 0;
        }
    }

    read_stat()
}

// ─── Object info tables ──────────────────────────────────────────────────────

unsafe fn rs_write_obj_info(savef: *mut CFile, info: *mut CObjInfo, count: c_int) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_marker(savef, RSID_MAGICITEMS);
    let _ = rs_write_int(savef, count);

    let mut n: c_int = 0;
    while n < count {
        // oi_name is constant, defined at compile time in all cases
        let _ = rs_write_int(savef, (*info.add(n as usize)).oi_prob);
        let _ = rs_write_int(savef, (*info.add(n as usize)).oi_worth);
        let _ = rs_write_string(savef, (*info.add(n as usize)).oi_guess);
        let _ = rs_write_boolean(savef, (*info.add(n as usize)).oi_know as c_int);
        n += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_obj_info(inf: *mut CFile, mi: *mut CObjInfo, count: c_int) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_marker(inf, RSID_MAGICITEMS);
    let _ = rs_read_int(inf, &mut value);

    if value > count {
        FORMAT_ERROR = 1;
    }

    let mut n: c_int = 0;
    while n < value {
        // oi_name is constant, defined at compile time in all cases
        let _ = rs_read_int(inf, &mut (*mi.add(n as usize)).oi_prob);
        let _ = rs_read_int(inf, &mut (*mi.add(n as usize)).oi_worth);
        let _ = rs_read_new_string(inf, &mut (*mi.add(n as usize)).oi_guess);
        let _ = rs_read_boolean(inf, &mut (*mi.add(n as usize)).oi_know);
        n += 1;
    }

    read_stat()
}

// ─── Rooms ───────────────────────────────────────────────────────────────────

unsafe fn rs_write_room(savef: *mut CFile, r: *mut CRoom) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_coord(savef, (*r).r_pos);
    let _ = rs_write_coord(savef, (*r).r_max);
    let _ = rs_write_coord(savef, (*r).r_gold);
    let _ = rs_write_int(savef, (*r).r_goldval);
    let _ = rs_write_short(savef, (*r).r_flags);
    let _ = rs_write_int(savef, (*r).r_nexits);
    let mut i = 0;
    while i < 12 {
        let _ = rs_write_coord(savef, (*r).r_exit[i]);
        i += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_room(inf: *mut CFile, r: *mut CRoom) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_coord(inf, &mut (*r).r_pos);
    let _ = rs_read_coord(inf, &mut (*r).r_max);
    let _ = rs_read_coord(inf, &mut (*r).r_gold);
    let _ = rs_read_int(inf, &mut (*r).r_goldval);
    let _ = rs_read_short(inf, &mut (*r).r_flags);
    let _ = rs_read_int(inf, &mut (*r).r_nexits);
    let mut i = 0;
    while i < 12 {
        let _ = rs_read_coord(inf, &mut (*r).r_exit[i]);
        i += 1;
    }

    read_stat()
}

unsafe fn rs_write_rooms(savef: *mut CFile, r: *mut CRoom, count: c_int) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, count);

    let mut n: c_int = 0;
    while n < count {
        let _ = rs_write_room(savef, &mut *r.add(n as usize));
        n += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_rooms(inf: *mut CFile, r: *mut CRoom, count: c_int) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut value);

    if value > count {
        FORMAT_ERROR = 1;
    }

    let mut n: c_int = 0;
    while n < value {
        let _ = rs_read_room(inf, &mut *r.add(n as usize));
        n += 1;
    }

    read_stat()
}

/// Writes an index into the global rooms[] table for a room pointer.
///
/// Uses globals: rooms.
unsafe fn rs_write_room_reference(savef: *mut CFile, rp: *mut CRoom) -> c_int {
    let mut room: c_int = -1;

    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let mut i = 0;
    while i < MAXROOMS {
        if (&raw mut rooms[i]) as *mut CRoom == rp {
            room = i as c_int;
        }
        i += 1;
    }

    let _ = rs_write_int(savef, room);

    WRITE_ERROR
}

/// Reads an index into the global rooms[] table, resolving the pointer.
///
/// Uses globals: rooms.
unsafe fn rs_read_room_reference(inf: *mut CFile, rp: *mut *mut CRoom) -> c_int {
    let mut i: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut i);

    if (i as usize) < MAXROOMS {
        *rp = (&raw mut rooms[i as usize]) as *mut CRoom;
    } else {
        *rp = std::ptr::null_mut();
    }

    read_stat()
}

// ─── Monsters ────────────────────────────────────────────────────────────────

unsafe fn rs_write_monsters(savef: *mut CFile, m: *mut CMonsterState, count: c_int) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_marker(savef, RSID_MONSTERS);
    let _ = rs_write_int(savef, count);

    let mut n: c_int = 0;
    while n < count {
        let _ = rs_write_stats(savef, &mut (*m.add(n as usize)).m_stats);
        n += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_monsters(inf: *mut CFile, m: *mut CMonsterState, count: c_int) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_marker(inf, RSID_MONSTERS);
    let _ = rs_read_int(inf, &mut value);

    if value != count {
        FORMAT_ERROR = 1;
    }

    let mut n: c_int = 0;
    while n < count {
        let _ = rs_read_stats(inf, &mut (*m.add(n as usize)).m_stats);
        n += 1;
    }

    read_stat()
}

// ─── Objects ─────────────────────────────────────────────────────────────────

unsafe fn rs_write_object(savef: *mut CFile, o: *mut CThing) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let op = thing_o(o);

    let _ = rs_write_marker(savef, RSID_OBJECT);
    let _ = rs_write_int(savef, (*op).o_type);
    let _ = rs_write_coord(savef, (*op).o_pos);
    let _ = rs_write_int(savef, (*op).o_launch);
    let _ = rs_write_char(savef, (*op).o_packch);
    let _ = rs_write_chars(savef, (&raw mut (*op).o_damage) as *mut c_char, 8);
    let _ = rs_write_chars(savef, (&raw mut (*op).o_hurldmg) as *mut c_char, 8);
    let _ = rs_write_int(savef, (*op).o_count);
    let _ = rs_write_int(savef, (*op).o_which);
    let _ = rs_write_int(savef, (*op).o_hplus);
    let _ = rs_write_int(savef, (*op).o_dplus);
    let _ = rs_write_int(savef, (*op).o_arm);
    let _ = rs_write_int(savef, (*op).o_flags);
    let _ = rs_write_int(savef, (*op).o_group);
    let _ = rs_write_string(savef, (*op).o_label);

    WRITE_ERROR
}

unsafe fn rs_read_object(inf: *mut CFile, o: *mut CThing) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let op = thing_o(o);

    let _ = rs_read_marker(inf, RSID_OBJECT);
    let _ = rs_read_int(inf, &mut (*op).o_type);
    let _ = rs_read_coord(inf, &mut (*op).o_pos);
    let _ = rs_read_int(inf, &mut (*op).o_launch);
    let _ = rs_read_char(inf, &mut (*op).o_packch);
    let _ = rs_read_chars(inf, (&raw mut (*op).o_damage) as *mut c_char, 8);
    let _ = rs_read_chars(inf, (&raw mut (*op).o_hurldmg) as *mut c_char, 8);
    let _ = rs_read_int(inf, &mut (*op).o_count);
    let _ = rs_read_int(inf, &mut (*op).o_which);
    let _ = rs_read_int(inf, &mut (*op).o_hplus);
    let _ = rs_read_int(inf, &mut (*op).o_dplus);
    let _ = rs_read_int(inf, &mut (*op).o_arm);
    let _ = rs_read_int(inf, &mut (*op).o_flags);
    let _ = rs_read_int(inf, &mut (*op).o_group);
    let _ = rs_read_new_string(inf, &mut (*op).o_label);

    read_stat()
}

unsafe fn rs_write_object_list(savef: *mut CFile, mut l: *mut CThing) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_marker(savef, RSID_OBJECTLIST);
    let _ = rs_write_int(savef, list_size(l));

    while !l.is_null() {
        let _ = rs_write_object(savef, l);
        l = (*thing_t(l)).l_next;
    }

    WRITE_ERROR
}

unsafe fn rs_read_object_list(inf: *mut CFile, list: *mut *mut CThing) -> c_int {
    let mut cnt: c_int = 0;
    let mut l: *mut CThing = std::ptr::null_mut();
    let mut previous: *mut CThing = std::ptr::null_mut();
    let mut head: *mut CThing = std::ptr::null_mut();

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_marker(inf, RSID_OBJECTLIST);
    let _ = rs_read_int(inf, &mut cnt);

    let mut i: c_int = 0;
    while i < cnt {
        // new_item() zero-allocates, matching the C memset(l, 0, sizeof(THING)).
        l = new_item();

        (*thing_t(l)).l_prev = previous;

        if !previous.is_null() {
            (*thing_t(previous)).l_next = l;
        }

        let _ = rs_read_object(inf, l);

        if previous.is_null() {
            head = l;
        }

        previous = l;
        i += 1;
    }

    if !l.is_null() {
        (*thing_t(l)).l_next = std::ptr::null_mut();
    }

    *list = head;

    read_stat()
}

unsafe fn rs_write_object_reference(savef: *mut CFile, list: *mut CThing, item: *mut CThing) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let i = find_list_ptr(list, item as *const c_void);

    rs_write_int(savef, i)
}

unsafe fn rs_read_object_reference(inf: *mut CFile, list: *mut CThing, item: *mut *mut CThing) -> c_int {
    let mut i: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut i);

    *item = get_list_item(list, i);

    read_stat()
}

// ─── Thing serialization ─────────────────────────────────────────────────────

unsafe fn find_room_coord(rmlist: *mut CRoom, c: *mut CCoord, n: c_int) -> c_int {
    let mut i: c_int = 0;

    while i < n {
        if (&raw mut (*rmlist.add(i as usize)).r_gold) as *mut CCoord == c {
            return i;
        }
        i += 1;
    }

    -1
}

unsafe fn find_thing_coord(monlist: *mut CThing, c: *mut CCoord) -> c_int {
    let mut mitem: *mut CThing = monlist;
    let mut i: c_int = 0;

    while !mitem.is_null() {
        if c == (&raw mut (*thing_t(mitem)).t_pos) as *mut CCoord {
            return i;
        }
        i += 1;
        mitem = (*thing_t(mitem)).l_next;
    }

    -1
}

unsafe fn find_object_coord(objlist: *mut CThing, c: *mut CCoord) -> c_int {
    let mut oitem: *mut CThing = objlist;
    let mut i: c_int = 0;

    while !oitem.is_null() {
        if c == (&raw mut (*thing_o(oitem)).o_pos) as *mut CCoord {
            return i;
        }
        i += 1;
        oitem = (*thing_t(oitem)).l_next;
    }

    -1
}

/// Serializes a monster/player THING, encoding chase targets as references
/// into the global mlist, lvl_obj, rooms or hero.
///
/// Uses globals: hero, mlist, lvl_obj, rooms.
unsafe fn rs_write_thing(savef: *mut CFile, t: *mut CThing) -> c_int {
    let mut i: c_int = -1;

    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_marker(savef, RSID_THING);

    if t.is_null() {
        let _ = rs_write_int(savef, 0);
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, 1);
    let _ = rs_write_coord(savef, (*thing_t(t)).t_pos);
    let _ = rs_write_boolean(savef, (*thing_t(t)).t_turn as c_int);
    let _ = rs_write_char(savef, (*thing_t(t)).t_type);
    let _ = rs_write_char(savef, (*thing_t(t)).t_disguise);
    let _ = rs_write_char(savef, (*thing_t(t)).t_oldch);

    /*
        t_dest can be:
        0,0: NULL
        0,1: location of hero
        1,i: location of a thing (monster)
        2,i: location of an object
        3,i: location of gold in a room

        We need to remember what we are chasing rather than
        the current location of what we are chasing.
    */

    let hero_pos_ptr = (&raw mut (*thing_t(&raw mut player)).t_pos) as *mut CCoord;
    let t_dest = (*thing_t(t)).t_dest;

    if t_dest == hero_pos_ptr {
        let _ = rs_write_int(savef, 0);
        let _ = rs_write_int(savef, 1);
    } else if !t_dest.is_null() {
        i = find_thing_coord(mlist, t_dest);

        if i >= 0 {
            let _ = rs_write_int(savef, 1);
            let _ = rs_write_int(savef, i);
        } else {
            i = find_object_coord(lvl_obj, t_dest);

            if i >= 0 {
                let _ = rs_write_int(savef, 2);
                let _ = rs_write_int(savef, i);
            } else {
                i = find_room_coord((&raw mut rooms) as *mut CRoom, t_dest, MAXROOMS as c_int);

                if i >= 0 {
                    let _ = rs_write_int(savef, 3);
                    let _ = rs_write_int(savef, i);
                } else {
                    let _ = rs_write_int(savef, 0);
                    let _ = rs_write_int(savef, 1); /* chase the hero anyway */
                }
            }
        }
    } else {
        let _ = rs_write_int(savef, 0);
        let _ = rs_write_int(savef, 0);
    }

    let _ = rs_write_short(savef, (*thing_t(t)).t_flags);
    let _ = rs_write_stats(savef, &raw mut (*thing_t(t)).t_stats);
    let _ = rs_write_room_reference(savef, (*thing_t(t)).t_room);
    let _ = rs_write_object_list(savef, (*thing_t(t)).t_pack);

    WRITE_ERROR
}

/// Restores a monster/player THING, resolving chase-target references against
/// the global hero, mlist, lvl_obj and rooms tables.
///
/// Uses globals: hero, mlist, lvl_obj, rooms.
unsafe fn rs_read_thing(inf: *mut CFile, t: *mut CThing) -> c_int {
    let mut listid: c_int = 0;
    let mut index: c_int = -1;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_marker(inf, RSID_THING);
    let _ = rs_read_int(inf, &mut index);

    if index == 0 {
        return read_stat();
    }

    let _ = rs_read_coord(inf, &mut (*thing_t(t)).t_pos);
    let _ = rs_read_boolean(inf, &mut (*thing_t(t)).t_turn);
    let _ = rs_read_char(inf, &mut (*thing_t(t)).t_type);
    let _ = rs_read_char(inf, &mut (*thing_t(t)).t_disguise);
    let _ = rs_read_char(inf, &mut (*thing_t(t)).t_oldch);

    /*
        t_dest can be (listid,index):
        0,0: NULL
        0,1: location of hero
        1,i: location of a thing (monster)
        2,i: location of an object
        3,i: location of gold in a room

        We need to remember what we are chasing rather than
        the current location of what we are chasing.
    */

    let _ = rs_read_int(inf, &mut listid);
    let _ = rs_read_int(inf, &mut index);
    (*thing_t(t)).t_reserved = -1;

    if listid == 0 {
        /* hero or NULL */
        if index == 1 {
            (*thing_t(t)).t_dest = (&raw mut (*thing_t(&raw mut player)).t_pos) as *mut CCoord;
        } else {
            (*thing_t(t)).t_dest = std::ptr::null_mut();
        }
    } else if listid == 1 {
        /* monster/thing */
        (*thing_t(t)).t_dest = std::ptr::null_mut();
        (*thing_t(t)).t_reserved = index;
    } else if listid == 2 {
        /* object */
        let item = get_list_item(lvl_obj, index);

        if !item.is_null() {
            (*thing_t(t)).t_dest = (&raw mut (*thing_o(item)).o_pos) as *mut CCoord;
        }
    } else if listid == 3 {
        /* gold */
        if (index as usize) < MAXROOMS {
            (*thing_t(t)).t_dest = (&raw mut rooms[index as usize].r_gold) as *mut CCoord;
        } else {
            (*thing_t(t)).t_dest = std::ptr::null_mut();
        }
    } else {
        (*thing_t(t)).t_dest = std::ptr::null_mut();
    }

    let _ = rs_read_short(inf, &mut (*thing_t(t)).t_flags);
    let _ = rs_read_stats(inf, &raw mut (*thing_t(t)).t_stats);
    let _ = rs_read_room_reference(inf, &mut (*thing_t(t)).t_room);
    let _ = rs_read_object_list(inf, &mut (*thing_t(t)).t_pack);

    read_stat()
}

/// Resolves a deferred monster chase target stored in t_reserved.
///
/// Uses globals: mlist.
unsafe fn rs_fix_thing(t: *mut CThing) {
    if (*thing_t(t)).t_reserved < 0 {
        return;
    }

    let item = get_list_item(mlist, (*thing_t(t)).t_reserved);

    if !item.is_null() {
        (*thing_t(t)).t_dest = (&raw mut (*thing_t(item)).t_pos) as *mut CCoord;
    }
}

unsafe fn rs_write_thing_list(savef: *mut CFile, mut l: *mut CThing) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_marker(savef, RSID_MONSTERLIST);

    let cnt = list_size(l);

    let _ = rs_write_int(savef, cnt);

    if cnt < 1 {
        return WRITE_ERROR;
    }

    while !l.is_null() {
        let _ = rs_write_thing(savef, l);
        l = (*thing_t(l)).l_next;
    }

    WRITE_ERROR
}

unsafe fn rs_read_thing_list(inf: *mut CFile, list: *mut *mut CThing) -> c_int {
    let mut cnt: c_int = 0;
    let mut l: *mut CThing = std::ptr::null_mut();
    let mut previous: *mut CThing = std::ptr::null_mut();
    let mut head: *mut CThing = std::ptr::null_mut();

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_marker(inf, RSID_MONSTERLIST);
    let _ = rs_read_int(inf, &mut cnt);

    let mut i: c_int = 0;
    while i < cnt {
        l = new_item();

        (*thing_t(l)).l_prev = previous;

        if !previous.is_null() {
            (*thing_t(previous)).l_next = l;
        }

        let _ = rs_read_thing(inf, l);

        if previous.is_null() {
            head = l;
        }

        previous = l;
        i += 1;
    }

    if !l.is_null() {
        (*thing_t(l)).l_next = std::ptr::null_mut();
    }

    *list = head;

    read_stat()
}

unsafe fn rs_fix_thing_list(list: *mut CThing) {
    let mut item: *mut CThing = list;

    while !item.is_null() {
        rs_fix_thing(item);
        item = (*thing_t(item)).l_next;
    }
}

unsafe fn rs_write_thing_reference(savef: *mut CFile, list: *mut CThing, item: *mut CThing) -> c_int {
    let mut i: c_int;

    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    if item.is_null() {
        let _ = rs_write_int(savef, -1);
    } else {
        i = find_list_ptr(list, item as *const c_void);
        let _ = rs_write_int(savef, i);
    }

    WRITE_ERROR
}

unsafe fn rs_read_thing_reference(inf: *mut CFile, list: *mut CThing, item: *mut *mut CThing) -> c_int {
    let mut i: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut i);

    if i == -1 {
        *item = std::ptr::null_mut();
    } else {
        *item = get_list_item(list, i);
    }

    read_stat()
}

unsafe fn rs_write_thing_references(
    savef: *mut CFile,
    list: *mut CThing,
    items: *mut *mut CThing,
    count: c_int,
) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let mut i: c_int = 0;
    while i < count {
        let _ = rs_write_thing_reference(savef, list, *items.add(i as usize));
        i += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_thing_references(
    inf: *mut CFile,
    list: *mut CThing,
    items: *mut *mut CThing,
    count: c_int,
) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut i: c_int = 0;
    while i < count {
        let _ = rs_read_thing_reference(inf, list, &mut *items.add(i as usize));
        i += 1;
    }

    read_stat()
}

// ─── Places (level map) ──────────────────────────────────────────────────────

/// Serialize the playable cell grid (the top `24x80` screen rows of the
/// level): for each cell the tile discriminant, the four flag grids, the trap
/// kind, and the per-cell monster reference (indexed into the global `mlist`).
///
/// Uses globals: mlist, places (via crate::game), CURRENT_LEVEL.
unsafe fn rs_write_places(savef: *mut CFile, count: c_int) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let lvl = crate::game::current_level();
    let mut i: c_int = 0;
    while i < count {
        let y = i / MAXCOLS;
        let x = i % MAXCOLS;
        let idx = (y as usize) * crate::level::LEVEL_WIDTH + (x as usize);
        let tile = lvl
            .map
            .get(y as usize, x as usize)
            .unwrap_or(crate::level::Tile::Empty);
        let _ = rs_write_char(savef, tile.to_u8() as c_char);
        let _ = rs_write_boolean(savef, lvl.flags.real[idx] as c_int);
        let _ = rs_write_boolean(savef, lvl.flags.passage[idx] as c_int);
        let _ = rs_write_boolean(savef, lvl.flags.seen[idx] as c_int);
        let _ = rs_write_char(savef, lvl.flags.passnum[idx] as c_char);
        let _ = rs_write_char(savef, lvl.flags.trap[idx] as c_char);
        // Per-cell monster occupancy, using the legacy `(x<<5)+y` layout.
        let place_idx = ((x as usize) << 5) + (y as usize);
        let _ = rs_write_thing_reference(savef, mlist, crate::game::places[place_idx].p_monst);
        i += 1;
    }

    WRITE_ERROR
}

/// Restore the playable cell grid, resolving monster references against the
/// global mlist and writing everything back into `CURRENT_LEVEL` plus the
/// monster map.
///
/// Uses globals: mlist, places (via crate::game), CURRENT_LEVEL.
unsafe fn rs_read_places(inf: *mut CFile, count: c_int) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let lvl = crate::game::current_level_mut();
    let mut i: c_int = 0;
    while i < count {
        let y = i / MAXCOLS;
        let x = i % MAXCOLS;
        let idx = (y as usize) * crate::level::LEVEL_WIDTH + (x as usize);

        let mut tile_disc: c_char = 0;
        let mut real: c_uchar = 0;
        let mut passage: c_uchar = 0;
        let mut seen: c_uchar = 0;
        let mut passnum: c_char = 0;
        let mut trap_kind: c_char = 0;
        let mut monst: *mut CThing = std::ptr::null_mut();

        let _ = rs_read_char(inf, &mut tile_disc);
        let _ = rs_read_boolean(inf, &mut real);
        let _ = rs_read_boolean(inf, &mut passage);
        let _ = rs_read_boolean(inf, &mut seen);
        let _ = rs_read_char(inf, &mut passnum);
        let _ = rs_read_char(inf, &mut trap_kind);
        let _ = rs_read_thing_reference(inf, mlist, &mut monst);

        let tile = crate::level::Tile::from_u8(tile_disc as u8).unwrap_or(crate::level::Tile::Empty);
        let _ = lvl.map.set(y as usize, x as usize, tile);
        lvl.flags.real[idx] = real != 0;
        lvl.flags.passage[idx] = passage != 0;
        lvl.flags.seen[idx] = seen != 0;
        lvl.flags.passnum[idx] = passnum as u8;
        lvl.flags.trap[idx] = trap_kind as u8;

        // Per-cell monster occupancy, using the legacy `(x<<5)+y` layout.
        let place_idx = ((x as usize) << 5) + (y as usize);
        crate::game::places[place_idx].p_monst = monst;
        crate::game::MONSTERS[place_idx] = monst;
        i += 1;
    }

    read_stat()
}


// ─── Whole-game save / restore ───────────────────────────────────────────────

/// Writes the entire game state to the save file.
///
/// Uses globals: after, again, noscore, seenstairs, amulet, door_stop,
/// fight_flush, firstmove, got_ltc, has_hit, in_shell, inv_describe,
/// jump, kamikaze, lower_msg, move_on, msg_esc, passgo, playing,
/// q_comm, running, save_msg, see_floor, stat_msg, terse, to_death,
/// tombstone, wizard, pack_used, dir_ch, file_name, huh, p_colors,
/// rainbow, prbuf, r_stones, stones, release, runch, s_names, take,
/// whoami, ws_made, ws_type, wood, metal, orig_dsusp, fruit, home,
/// inv_t_name, l_last_comm, l_last_dir, last_comm, last_dir, tr_name,
/// n_objs, ntraps, hungry_state, inpack, inv_type, level, max_level,
/// mpos, no_food, a_class, count, food_left, lastscore, no_command,
/// no_move, purse, quiet, vf_hit, dnum, seed, e_levels, delta, oldpos,
/// stairs, player, cur_armor, cur_ring, cur_weapon, l_last_pick,
/// last_pick, lvl_obj, mlist, places, max_stats, rooms, oldrp,
/// passages, monsters, things, arm_info, pot_info, ring_info,
/// scr_info, weap_info, ws_info, d_list, total, between, nh, group,
/// stdscr.
#[no_mangle]
pub unsafe extern "C" fn rs_save_file(savef: *mut CFile) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_boolean(savef, after as c_int);             /* 1  */ /* extern.c */
    let _ = rs_write_boolean(savef, again as c_int);             /* 2  */
    let _ = rs_write_int(savef, noscore);                        /* 3  */
    let _ = rs_write_boolean(savef, seenstairs as c_int);        /* 4  */
    let _ = rs_write_boolean(savef, amulet as c_int);            /* 5  */
    let _ = rs_write_boolean(savef, door_stop as c_int);         /* 6  */
    let _ = rs_write_boolean(savef, fight_flush as c_int);       /* 7  */
    let _ = rs_write_boolean(savef, firstmove as c_int);         /* 8  */
    let _ = rs_write_boolean(savef, got_ltc as c_int);           /* 9  */
    let _ = rs_write_boolean(savef, has_hit as c_int);           /* 10 */
    let _ = rs_write_boolean(savef, in_shell as c_int);          /* 11 */
    let _ = rs_write_boolean(savef, inv_describe as c_int);      /* 12 */
    let _ = rs_write_boolean(savef, jump as c_int);              /* 13 */
    let _ = rs_write_boolean(savef, kamikaze as c_int);          /* 14 */
    let _ = rs_write_boolean(savef, lower_msg as c_int);         /* 15 */
    let _ = rs_write_boolean(savef, move_on as c_int);           /* 16 */
    let _ = rs_write_boolean(savef, msg_esc as c_int);           /* 17 */
    let _ = rs_write_boolean(savef, passgo as c_int);            /* 18 */
    let _ = rs_write_boolean(savef, playing as c_int);           /* 19 */
    let _ = rs_write_boolean(savef, q_comm as c_int);            /* 20 */
    let _ = rs_write_boolean(savef, running as c_int);           /* 21 */
    let _ = rs_write_boolean(savef, save_msg as c_int);          /* 22 */
    let _ = rs_write_boolean(savef, see_floor as c_int);         /* 23 */
    let _ = rs_write_boolean(savef, stat_msg as c_int);          /* 24 */
    let _ = rs_write_boolean(savef, terse as c_int);             /* 25 */
    let _ = rs_write_boolean(savef, to_death as c_int);          /* 26 */
    let _ = rs_write_boolean(savef, tombstone as c_int);         /* 27 */
    if MASTER {
        let _ = rs_write_int(savef, wizard);                     /* 28 */
    } else {
        let _ = rs_write_int(savef, 0);                          /* 28 */
    }
    let _ = rs_write_booleans(savef, (&raw mut pack_used) as *mut c_uchar, 26); /* 29 */
    let _ = rs_write_char(savef, dir_ch);
    let _ = rs_write_chars(savef, (&raw mut file_name) as *mut c_char, MAXSTR as c_int);
    let _ = rs_write_chars(savef, (&raw mut huh) as *mut c_char, MAXSTR as c_int);
    let _ = rs_write_potions(savef);
    let _ = rs_write_chars(savef, (&raw mut prbuf) as *mut c_char, (2 * MAXSTR) as c_int);
    let _ = rs_write_rings(savef);
    let _ = rs_write_string(savef, release);
    let _ = rs_write_char(savef, runch);
    let _ = rs_write_scrolls(savef);
    let _ = rs_write_char(savef, take);
    let _ = rs_write_chars(savef, (&raw mut whoami) as *mut c_char, MAXSTR as c_int);
    let _ = rs_write_sticks(savef);
    let _ = rs_write_int(savef, orig_dsusp);
    let _ = rs_write_chars(savef, (&raw mut fruit) as *mut c_char, MAXSTR as c_int);
    let _ = rs_write_chars(savef, (&raw mut home) as *mut c_char, MAXSTR as c_int);
    let _ = rs_write_strings(savef, (&raw mut inv_t_name) as *mut *mut c_char, 3);
    let _ = rs_write_char(savef, l_last_comm);
    let _ = rs_write_char(savef, l_last_dir);
    let _ = rs_write_char(savef, last_comm);
    let _ = rs_write_char(savef, last_dir);
    let _ = rs_write_strings(savef, (&raw mut tr_name) as *mut *mut c_char, 8);
    let _ = rs_write_int(savef, n_objs);
    let _ = rs_write_int(savef, ntraps);
    let _ = rs_write_int(savef, hungry_state);
    let _ = rs_write_int(savef, inpack);
    let _ = rs_write_int(savef, inv_type);
    let _ = rs_write_int(savef, level);
    let _ = rs_write_int(savef, max_level);
    let _ = rs_write_int(savef, mpos);
    let _ = rs_write_int(savef, no_food);
    let _ = rs_write_ints(savef, (&raw mut a_class) as *mut c_int, MAXARMORS as c_int);
    let _ = rs_write_int(savef, COUNT);
    let _ = rs_write_int(savef, food_left);
    let _ = rs_write_int(savef, lastscore);
    let _ = rs_write_int(savef, no_command);
    let _ = rs_write_int(savef, no_move);
    let _ = rs_write_int(savef, purse);
    let _ = rs_write_int(savef, quiet);
    let _ = rs_write_int(savef, vf_hit);
    let _ = rs_write_int(savef, dnum);
    let _ = rs_write_int(savef, seed);
    let _ = rs_write_ints(savef, (&raw mut e_levels) as *mut c_int, 21);
    let _ = rs_write_coord(savef, delta);
    let _ = rs_write_coord(savef, oldpos);
    let _ = rs_write_coord(savef, stairs);

    let _ = rs_write_thing(savef, &raw mut player);
    let _ = rs_write_object_reference(savef, (*thing_t(&raw mut player)).t_pack, cur_armor);
    let _ = rs_write_object_reference(savef, (*thing_t(&raw mut player)).t_pack, cur_ring[0]);
    let _ = rs_write_object_reference(savef, (*thing_t(&raw mut player)).t_pack, cur_ring[1]);
    let _ = rs_write_object_reference(savef, (*thing_t(&raw mut player)).t_pack, cur_weapon);
    let _ = rs_write_object_reference(savef, (*thing_t(&raw mut player)).t_pack, l_last_pick);
    let _ = rs_write_object_reference(savef, (*thing_t(&raw mut player)).t_pack, last_pick);

    let _ = rs_write_object_list(savef, lvl_obj);
    let _ = rs_write_thing_list(savef, mlist);

    let _ = rs_write_places(savef, MAXLINES * MAXCOLS);

    let _ = rs_write_stats(savef, &raw mut max_stats);
    let _ = rs_write_rooms(savef, (&raw mut rooms) as *mut CRoom, MAXROOMS as c_int);
    let _ = rs_write_room_reference(savef, oldrp);
    let _ = rs_write_rooms(savef, (&raw mut passages) as *mut CRoom, MAXPASS as c_int);

    let _ = rs_write_monsters(savef, (&raw mut monsters) as *mut CMonsterState, MAXMONSTERS as c_int);
    let _ = rs_write_obj_info(savef, (&raw mut things) as *mut CObjInfo, NUMTHINGS as c_int);
    let _ = rs_write_obj_info(savef, (&raw mut arm_info) as *mut CObjInfo, MAXARMORS as c_int);
    let _ = rs_write_obj_info(savef, (&raw mut pot_info) as *mut CObjInfo, MAXPOTIONS as c_int);
    let _ = rs_write_obj_info(savef, (&raw mut ring_info) as *mut CObjInfo, MAXRINGS as c_int);
    let _ = rs_write_obj_info(savef, (&raw mut scr_info) as *mut CObjInfo, MAXSCROLLS as c_int);
    let _ = rs_write_obj_info(savef, (&raw mut weap_info) as *mut CObjInfo, (MAXWEAPONS + 1) as c_int);
    let _ = rs_write_obj_info(savef, (&raw mut ws_info) as *mut CObjInfo, MAXSTICKS as c_int);

    let _ = rs_write_daemons(savef, (&raw mut d_list) as *mut CDelayedAction, MAXDAEMONS as c_int);
    if MASTER {
        let _ = rs_write_int(savef, total);                     /* 5.4-list.c */
    } else {
        let _ = rs_write_int(savef, 0);
    }
    let _ = rs_write_int(savef, between);                        /* 5.4-daemons.c */
    let _ = rs_write_coord(savef, nh);                           /* 5.4-move.c */
    let _ = rs_write_int(savef, group);                          /* 5.4-weapons.rs */

    let _ = rs_write_window(savef, stdscr as *mut CWindow);

    WRITE_ERROR
}

/// Reads the entire game state back from the save file, restoring all of the
/// global variables written by rs_save_file().
///
/// Uses globals: after, again, noscore, seenstairs, amulet, door_stop,
/// fight_flush, firstmove, got_ltc, has_hit, in_shell, inv_describe,
/// jump, kamikaze, lower_msg, move_on, msg_esc, passgo, playing,
/// q_comm, running, save_msg, see_floor, stat_msg, terse, to_death,
/// tombstone, wizard, pack_used, dir_ch, file_name, huh, p_colors,
/// rainbow, prbuf, r_stones, stones, release, runch, s_names, take,
/// whoami, ws_made, ws_type, wood, metal, orig_dsusp, fruit, home,
/// inv_t_name, l_last_comm, l_last_dir, last_comm, last_dir, tr_name,
/// n_objs, ntraps, hungry_state, inpack, inv_type, level, max_level,
/// mpos, no_food, a_class, count, food_left, lastscore, no_command,
/// no_move, purse, quiet, vf_hit, dnum, seed, e_levels, delta, oldpos,
/// stairs, player, cur_armor, cur_ring, cur_weapon, l_last_pick,
/// last_pick, lvl_obj, mlist, places, max_stats, rooms, oldrp,
/// passages, monsters, things, arm_info, pot_info, ring_info,
/// scr_info, weap_info, ws_info, d_list, total, between, nh, group,
/// stdscr.
#[no_mangle]
pub unsafe extern "C" fn rs_restore_file(inf: *mut CFile) -> c_int {
    let mut dummyint: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_boolean(inf, &mut after);               /* 1  */ /* extern.c */
    let _ = rs_read_boolean(inf, &mut again);               /* 2  */
    let _ = rs_read_int(inf, &mut noscore);                 /* 3  */
    let _ = rs_read_boolean(inf, &mut seenstairs);          /* 4  */
    let _ = rs_read_boolean(inf, &mut amulet);              /* 5  */
    let _ = rs_read_boolean(inf, &mut door_stop);           /* 6  */
    let _ = rs_read_boolean(inf, &mut fight_flush);         /* 7  */
    let _ = rs_read_boolean(inf, &mut firstmove);           /* 8  */
    let _ = rs_read_boolean(inf, &mut got_ltc);             /* 9  */
    let _ = rs_read_boolean(inf, &mut has_hit);             /* 10 */
    let _ = rs_read_boolean(inf, &mut in_shell);            /* 11 */
    let _ = rs_read_boolean(inf, &mut inv_describe);        /* 12 */
    let _ = rs_read_boolean(inf, &mut jump);                /* 13 */
    let _ = rs_read_boolean(inf, &mut kamikaze);            /* 14 */
    let _ = rs_read_boolean(inf, &mut lower_msg);           /* 15 */
    let _ = rs_read_boolean(inf, &mut move_on);             /* 16 */
    let _ = rs_read_boolean(inf, &mut msg_esc);             /* 17 */
    let _ = rs_read_boolean(inf, &mut passgo);              /* 18 */
    let _ = rs_read_boolean(inf, &mut playing);             /* 19 */
    let _ = rs_read_boolean(inf, &mut q_comm);              /* 20 */
    let _ = rs_read_boolean(inf, &mut running);             /* 21 */
    let _ = rs_read_boolean(inf, &mut save_msg);            /* 22 */
    let _ = rs_read_boolean(inf, &mut see_floor);           /* 23 */
    let _ = rs_read_boolean(inf, &mut stat_msg);            /* 24 */
    let _ = rs_read_boolean(inf, &mut terse);               /* 25 */
    let _ = rs_read_boolean(inf, &mut to_death);            /* 26 */
    let _ = rs_read_boolean(inf, &mut tombstone);           /* 27 */
    if MASTER {
        let _ = rs_read_int(inf, &mut wizard);              /* 28 */
    } else {
        let _ = rs_read_int(inf, &mut dummyint);            /* 28 */
    }
    let _ = rs_read_booleans(inf, (&raw mut pack_used) as *mut c_uchar, 26); /* 29 */
    let _ = rs_read_char(inf, &mut dir_ch);
    let _ = rs_read_chars(inf, (&raw mut file_name) as *mut c_char, MAXSTR as c_int);
    let _ = rs_read_chars(inf, (&raw mut huh) as *mut c_char, MAXSTR as c_int);
    let _ = rs_read_potions(inf);
    let _ = rs_read_chars(inf, (&raw mut prbuf) as *mut c_char, (2 * MAXSTR) as c_int);
    let _ = rs_read_rings(inf);
    let _ = rs_read_new_string(inf, &mut release);
    let _ = rs_read_char(inf, &mut runch);
    let _ = rs_read_scrolls(inf);
    let _ = rs_read_char(inf, &mut take);
    let _ = rs_read_chars(inf, (&raw mut whoami) as *mut c_char, MAXSTR as c_int);
    let _ = rs_read_sticks(inf);
    let _ = rs_read_int(inf, &mut orig_dsusp);
    let _ = rs_read_chars(inf, (&raw mut fruit) as *mut c_char, MAXSTR as c_int);
    let _ = rs_read_chars(inf, (&raw mut home) as *mut c_char, MAXSTR as c_int);
    let _ = rs_read_new_strings(inf, (&raw mut inv_t_name) as *mut *mut c_char, 3);
    let _ = rs_read_char(inf, &mut l_last_comm);
    let _ = rs_read_char(inf, &mut l_last_dir);
    let _ = rs_read_char(inf, &mut last_comm);
    let _ = rs_read_char(inf, &mut last_dir);
    let _ = rs_read_new_strings(inf, (&raw mut tr_name) as *mut *mut c_char, 8);
    let _ = rs_read_int(inf, &mut n_objs);
    let _ = rs_read_int(inf, &mut ntraps);
    let _ = rs_read_int(inf, &mut hungry_state);
    let _ = rs_read_int(inf, &mut inpack);
    let _ = rs_read_int(inf, &mut inv_type);
    let _ = rs_read_int(inf, &mut level);
    let _ = rs_read_int(inf, &mut max_level);
    let _ = rs_read_int(inf, &mut mpos);
    let _ = rs_read_int(inf, &mut no_food);
    let _ = rs_read_ints(inf, (&raw mut a_class) as *mut c_int, MAXARMORS as c_int);
    let _ = rs_read_int(inf, &mut COUNT);
    let _ = rs_read_int(inf, &mut food_left);
    let _ = rs_read_int(inf, &mut lastscore);
    let _ = rs_read_int(inf, &mut no_command);
    let _ = rs_read_int(inf, &mut no_move);
    let _ = rs_read_int(inf, &mut purse);
    let _ = rs_read_int(inf, &mut quiet);
    let _ = rs_read_int(inf, &mut vf_hit);
    let _ = rs_read_int(inf, &mut dnum);
    let _ = rs_read_int(inf, &mut seed);
    let _ = rs_read_ints(inf, (&raw mut e_levels) as *mut c_int, 21);
    let _ = rs_read_coord(inf, &mut delta);
    let _ = rs_read_coord(inf, &mut oldpos);
    let _ = rs_read_coord(inf, &mut stairs);

    let _ = rs_read_thing(inf, &raw mut player);
    let _ = rs_read_object_reference(inf, (*thing_t(&raw mut player)).t_pack, &raw mut cur_armor);
    let _ = rs_read_object_reference(inf, (*thing_t(&raw mut player)).t_pack, &raw mut cur_ring[0]);
    let _ = rs_read_object_reference(inf, (*thing_t(&raw mut player)).t_pack, &raw mut cur_ring[1]);
    let _ = rs_read_object_reference(inf, (*thing_t(&raw mut player)).t_pack, &raw mut cur_weapon);
    let _ = rs_read_object_reference(inf, (*thing_t(&raw mut player)).t_pack, &raw mut l_last_pick);
    let _ = rs_read_object_reference(inf, (*thing_t(&raw mut player)).t_pack, &raw mut last_pick);

    let _ = rs_read_object_list(inf, &raw mut lvl_obj);
    let _ = rs_read_thing_list(inf, &raw mut mlist);
    rs_fix_thing(&raw mut player);
    rs_fix_thing_list(mlist);

    let _ = rs_read_places(inf, MAXLINES * MAXCOLS);

    let _ = rs_read_stats(inf, &raw mut max_stats);
    let _ = rs_read_rooms(inf, (&raw mut rooms) as *mut CRoom, MAXROOMS as c_int);
    let _ = rs_read_room_reference(inf, &raw mut oldrp);
    let _ = rs_read_rooms(inf, (&raw mut passages) as *mut CRoom, MAXPASS as c_int);

    let _ = rs_read_monsters(inf, (&raw mut monsters) as *mut CMonsterState, MAXMONSTERS as c_int);
    let _ = rs_read_obj_info(inf, (&raw mut things) as *mut CObjInfo, NUMTHINGS as c_int);
    let _ = rs_read_obj_info(inf, (&raw mut arm_info) as *mut CObjInfo, MAXARMORS as c_int);
    let _ = rs_read_obj_info(inf, (&raw mut pot_info) as *mut CObjInfo, MAXPOTIONS as c_int);
    let _ = rs_read_obj_info(inf, (&raw mut ring_info) as *mut CObjInfo, MAXRINGS as c_int);
    let _ = rs_read_obj_info(inf, (&raw mut scr_info) as *mut CObjInfo, MAXSCROLLS as c_int);
    let _ = rs_read_obj_info(inf, (&raw mut weap_info) as *mut CObjInfo, (MAXWEAPONS + 1) as c_int);
    let _ = rs_read_obj_info(inf, (&raw mut ws_info) as *mut CObjInfo, MAXSTICKS as c_int);

    let _ = rs_read_daemons(inf, (&raw mut d_list) as *mut CDelayedAction, MAXDAEMONS as c_int);
    let _ = rs_read_int(inf, &mut dummyint);                  /* total */ /* 5.4-list.c */
    let _ = rs_read_int(inf, &mut between);                    /* 5.4-daemons.c */
    let _ = rs_read_coord(inf, &mut nh);                       /* 5.4-move.c */
    let _ = rs_read_int(inf, &mut group);                      /* 5.4-weapons.rs */

    let _ = rs_read_window(inf, stdscr as *mut CWindow);

    read_stat()
}