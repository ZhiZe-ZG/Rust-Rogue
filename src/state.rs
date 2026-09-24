//! Portable Rogue save-state code.
//!
//! Ported from `src/c/state.c` to Rust.
//!
//! Copyright (C) 1999, 2000, 2005 Nicholas J. Kisseberth
//! All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions
//! are met:
//! 1. Redistributions of source code must retain the above copyright
//!    notice, this list of conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright
//!    notice, this list of conditions and the following disclaimer in the
//!    documentation and/or other materials provided with the distribution.
//! 3. Neither the name(s) of the author(s) nor the names of other contributors
//!    may be used to endorse or promote products derived from this software
//!    without specific prior written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE AUTHOR(S) AND CONTRIBUTORS ``AS IS'' AND
//! ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//! IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//! ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR(S) OR CONTRIBUTORS BE LIABLE
//! FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
//! DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
//! OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
//! HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
//! LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
//! OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
//! SUCH DAMAGE.

use glam::IVec2;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint, c_ushort, c_void};

use crate::daemon::CDelayedAction;
use crate::daemons::{doctor, nohaste, rollwand, sight, stomach, swander, unconfuse, unsee};
use crate::entity::chase::runners;
use crate::entity::monster_list::MLIST;
use crate::entity::player::{Stats, CThing, CThingMonster, CThingObject};
use crate::game::EQUIPMENT;
use crate::globals::{
    arm_info, monsters, pot_info, ring_info, scr_info, things, weap_info, ws_info, CMonster,
    CObjInfo,
};
use crate::item::thing_list::{allocated_count, new_actor, new_item};
use crate::level::{PassageLinks, Room};

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

const MAXARMORS: usize = 8;
const MAXPOTIONS: usize = 14;
const MAXRINGS: usize = 14;
const MAXSCROLLS: usize = 18;
const MAXSTICKS: usize = 14;
const NUMTHINGS: usize = 7;
const MAXWEAPONS: usize = 9;
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
pub struct CStone {
    pub st_name: *const c_char,
    pub st_value: c_int,
}

/// Delayed-action callback slot type (same representation as `daemon::DFunc`).
type DFunc = Option<unsafe extern "C" fn()>;

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
    static mut msg_esc: bool;
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
    static mut delta: IVec2;
    static mut oldpos: IVec2;

    // player / lists
    static mut l_last_pick: *mut CThing;
    static mut last_pick: *mut CThing;

    // rooms / map
    static mut max_stats: Stats;
    static mut oldrp: Option<usize>;

    // daemons (defined in daemon.rs as `d_list`) and misc C-visible globals
    static mut d_list: [CDelayedAction; MAXDAEMONS];
    static mut between: c_int;
    static mut nh: IVec2;
    static mut group: c_int;

    // material arrays (defined in init.rs as stones/wood/metal)
    static stones: [CStone; 26];
    static mut wood: [*mut c_char; 33];
    static mut metal: [*mut c_char; 22];
    static mut cNSTONES: c_int;
    static mut cNWOOD: c_int;
    static mut cNMETAL: c_int;

    // libc / curses
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn fwrite(ptr: *const u8, size: usize, nmemb: usize, stream: *mut CFile) -> usize;
    fn fread(ptr: *mut u8, size: usize, n: usize, stream: *mut CFile) -> usize;
    fn strlen(s: *const c_char) -> usize;
    fn strcmp(a: *const c_char, b: *const c_char) -> c_int;
}

// ─── Helpers ────────────────────────────────────────────────────────────────

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    crate::entity::player::thing_o(tp)
}

/// Wrap a daemon callback in the nullable function-pointer representation.
#[inline]
unsafe fn fn_to_dfunc(f: unsafe extern "C" fn()) -> DFunc {
    Some(f)
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

    let len: c_int = if s.is_null() {
        0
    } else {
        strlen(s) as c_int + 1
    };

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

/// Read a length-prefixed C string into an owned Rust [`String`] (`None` if the
/// stored length was zero).
unsafe fn rs_read_new_cstring(inf: *mut CFile, s: &mut Option<String>) -> c_int {
    let mut buf: *mut c_char = std::ptr::null_mut();
    let stat = rs_read_new_string(inf, &mut buf);
    if buf.is_null() {
        *s = None;
    } else {
        *s = Some(CStr::from_ptr(buf).to_string_lossy().into_owned());
        free(buf as *mut c_void);
    }
    stat
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

unsafe fn rs_write_coord(savef: *mut CFile, c: IVec2) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, c.x);
    let _ = rs_write_int(savef, c.y);

    WRITE_ERROR
}

unsafe fn rs_read_coord(inf: *mut CFile, c: *mut IVec2) -> c_int {
    let mut in_coord: IVec2 = IVec2 { x: 0, y: 0 };

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

/// Dump the visible screen grid to the save file using the legacy window
/// header (height, width, then one cell per position).
unsafe fn rs_write_window(savef: *mut CFile) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let size = crate::ui::screen_size();
    let height = size.y;
    let width = size.x;

    let _ = rs_write_marker(savef, RSID_WINDOW);
    let _ = rs_write_int(savef, height);
    let _ = rs_write_int(savef, width);

    let mut row: c_int = 0;
    while row < height {
        let mut col: c_int = 0;
        while col < width {
            let cell = crate::ui::screen_cell(row, col) as c_int;
            if rs_write_int(savef, cell) != 0 {
                return WRITE_ERROR;
            }
            col += 1;
        }
        row += 1;
    }

    WRITE_ERROR
}

/// Reload the visible screen grid from the save file, clipping the stored
/// dimensions to the fixed terminal size.
unsafe fn rs_read_window(inf: *mut CFile) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let size = crate::ui::screen_size();
    let height = size.y;
    let width = size.x;

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
                crate::ui::set_screen_cell(row, col, value as u8);
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
        l = crate::entity::player::thing_next(l);
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
        l = crate::entity::player::thing_next(l);
    }

    -1
}

unsafe fn list_size(mut l: *mut CThing) -> c_int {
    let mut count: c_int = 0;

    while !l.is_null() {
        count += 1;
        l = crate::entity::player::thing_next(l);
    }

    count
}

// ─── Stats / stone / item tables ─────────────────────────────────────────────

unsafe fn rs_write_stats(savef: *mut CFile, s: *mut Stats) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_marker(savef, RSID_STATS);
    let _ = rs_write_str_t(savef, (*s).strength);
    let _ = rs_write_int(savef, (*s).experience);
    let _ = rs_write_int(savef, (*s).level);
    let _ = rs_write_int(savef, (*s).armor);
    let _ = rs_write_int(savef, (*s).hit_points);
    let _ = rs_write_chars(savef, (&raw mut (*s).damage) as *mut c_char, 13);
    let _ = rs_write_int(savef, (*s).max_hit_points);

    WRITE_ERROR
}

unsafe fn rs_read_stats(inf: *mut CFile, s: *mut Stats) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_marker(inf, RSID_STATS);
    let _ = rs_read_str_t(inf, &raw mut (*s).strength);
    let _ = rs_read_int(inf, &mut (*s).experience);
    let _ = rs_read_int(inf, &mut (*s).level);
    let _ = rs_read_int(inf, &mut (*s).armor);
    let _ = rs_read_int(inf, &mut (*s).hit_points);
    let _ = rs_read_chars(inf, (&raw mut (*s).damage) as *mut c_char, 13);
    let _ = rs_read_int(inf, &mut (*s).max_hit_points);

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

/// Index of `ptr` within [`crate::colors::POTION_COLORS`], or `-1` if it is not
/// a potion colour. Bridges the legacy `p_colors` pointer array to the Rust
/// colour table.
fn potion_color_index(ptr: *const c_char) -> c_int {
    for (i, color) in crate::colors::POTION_COLORS.iter().enumerate() {
        if color.as_ptr() as *const c_char == ptr {
            return i as c_int;
        }
    }
    -1
}

/// Serializes the global potion colors to the save file.
///
/// Uses globals: p_colors.
unsafe fn rs_write_potions(savef: *mut CFile) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let mut i = 0;
    while i < MAXPOTIONS {
        let _ = rs_write_int(savef, potion_color_index(p_colors[i] as *const c_char));
        i += 1;
    }

    WRITE_ERROR
}

/// Restores the global potion colors from the save file.
///
/// Uses globals: p_colors.
unsafe fn rs_read_potions(inf: *mut CFile) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut i = 0;
    while i < MAXPOTIONS {
        let mut idx: c_int = 0;
        let _ = rs_read_int(inf, &mut idx);
        p_colors[i] = if idx >= 0 && (idx as usize) < crate::colors::POTION_COLOR_COUNT {
            crate::colors::POTION_COLORS[idx as usize].as_ptr() as *mut c_char
        } else {
            std::ptr::null_mut()
        };
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
        let _ = rs_write_stone_index(
            savef,
            (&raw const stones) as *const CStone,
            cNSTONES,
            r_stones[i],
        );
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
        let _ = rs_read_stone_index(
            inf,
            (&raw const stones) as *const CStone,
            cNSTONES,
            &mut r_stones[i],
        );
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
            let _ = rs_write_string_index(
                savef,
                (&raw mut wood) as *mut *mut c_char,
                cNWOOD,
                ws_made[i],
            );
        } else {
            let _ = rs_write_int(savef, 1);
            let _ = rs_write_string_index(
                savef,
                (&raw mut metal) as *mut *mut c_char,
                cNMETAL,
                ws_made[i],
            );
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
            let _ = rs_read_string_index(
                inf,
                (&raw mut wood) as *mut *mut c_char,
                cNWOOD,
                &mut ws_made[i as usize],
            );
            ws_type[i as usize] = c"staff".as_ptr() as *mut c_char;
        } else {
            let _ = rs_read_string_index(
                inf,
                (&raw mut metal) as *mut *mut c_char,
                cNMETAL,
                &mut ws_made[i as usize],
            );
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
        let guess = (*info.add(n as usize)).oi_guess.as_deref();
        let guess_ptr = guess.map_or(std::ptr::null(), |s| s.as_ptr());
        let _ = rs_write_string(savef, guess_ptr as *const c_char);
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
        let mut guess_ptr: *mut c_char = std::ptr::null_mut();
        let _ = rs_read_new_string(inf, &mut guess_ptr);
        (*mi.add(n as usize)).oi_guess = if guess_ptr.is_null() {
            None
        } else {
            Some(CStr::from_ptr(guess_ptr).to_string_lossy().into_owned())
        };
        let mut know: c_uchar = 0;
        let _ = rs_read_boolean(inf, &mut know);
        (*mi.add(n as usize)).oi_know = know != 0;
        n += 1;
    }

    read_stat()
}

// ─── Rooms ───────────────────────────────────────────────────────────────────

unsafe fn rs_write_room(savef: *mut CFile, r: &Room) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_coord(savef, r.position);
    let _ = rs_write_coord(savef, r.size);
    let _ = rs_write_coord(savef, r.gold);
    let _ = rs_write_int(savef, r.goldval);
    let _ = rs_write_boolean(savef, r.gone as c_int);
    let _ = rs_write_boolean(savef, r.dark as c_int);
    let _ = rs_write_boolean(savef, r.maze as c_int);
    let _ = rs_write_int(savef, r.entry_point_count);
    let mut i = 0;
    while i < 12 {
        let exit = r.entry_points.get(i).copied().unwrap_or(IVec2::ZERO);
        let _ = rs_write_coord(savef, exit);
        i += 1;
    }

    WRITE_ERROR
}

unsafe fn rs_read_room(inf: *mut CFile, r: &mut Room) -> c_int {
    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let mut position = IVec2::ZERO;
    let mut size = IVec2::ZERO;
    let mut gold = IVec2::ZERO;
    let mut gone = 0;
    let mut dark = 0;
    let mut maze = 0;
    let _ = rs_read_coord(inf, &mut position);
    let _ = rs_read_coord(inf, &mut size);
    let _ = rs_read_coord(inf, &mut gold);
    let _ = rs_read_int(inf, &mut r.goldval);
    let _ = rs_read_boolean(inf, &mut gone);
    let _ = rs_read_boolean(inf, &mut dark);
    let _ = rs_read_boolean(inf, &mut maze);
    let _ = rs_read_int(inf, &mut r.entry_point_count);
    r.position = position;
    r.size = size;
    r.gold = gold;
    r.gone = gone != 0;
    r.dark = dark != 0;
    r.maze = maze != 0;
    r.entry_points.clear();
    let mut i = 0;
    while i < 12 {
        let mut exit = IVec2::ZERO;
        let _ = rs_read_coord(inf, &mut exit);
        if i < r.entry_point_count.max(0) as usize {
            r.entry_points.push(exit);
        }
        i += 1;
    }

    read_stat()
}

unsafe fn rs_write_rooms(savef: *mut CFile, rooms: &[Room]) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, rooms.len() as c_int);

    for room in rooms {
        let _ = rs_write_room(savef, room);
    }

    WRITE_ERROR
}

unsafe fn rs_read_rooms(inf: *mut CFile, rooms: &mut [Room]) -> c_int {
    let mut value: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut value);

    if value < 0 || value as usize > rooms.len() {
        FORMAT_ERROR = 1;
    }

    let mut n: c_int = 0;
    while n < value && (n as usize) < rooms.len() {
        let _ = rs_read_room(inf, &mut rooms[n as usize]);
        n += 1;
    }

    read_stat()
}

unsafe fn rs_write_passage_links(savef: *mut CFile, links: &[PassageLinks]) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, links.len() as c_int);
    for link in links {
        let _ = rs_write_int(savef, link.exits.len() as c_int);
        for exit in &link.exits {
            let _ = rs_write_coord(savef, *exit);
        }
    }

    WRITE_ERROR
}

unsafe fn rs_read_passage_links(inf: *mut CFile, links: &mut Vec<PassageLinks>) -> c_int {
    let mut count = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut count);
    if count < 0 {
        FORMAT_ERROR = 1;
        return read_stat();
    }

    links.clear();
    for _ in 0..count {
        let mut exit_count = 0;
        let _ = rs_read_int(inf, &mut exit_count);
        if exit_count < 0 {
            FORMAT_ERROR = 1;
            break;
        }
        let mut exits = Vec::with_capacity(exit_count as usize);
        for _ in 0..exit_count {
            let mut exit = IVec2::ZERO;
            let _ = rs_read_coord(inf, &mut exit);
            exits.push(exit);
        }
        links.push(PassageLinks { exits });
    }

    read_stat()
}

unsafe fn rs_write_room_reference(savef: *mut CFile, room: Option<usize>) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_int(savef, room.map_or(-1, |i| i as c_int));

    WRITE_ERROR
}

unsafe fn rs_read_room_reference(inf: *mut CFile, room: &mut Option<usize>) -> c_int {
    let mut i: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut i);

    if i >= 0 && (i as usize) < crate::config::GameConfig::MAX_ROOMS {
        *room = Some(i as usize);
    } else {
        *room = None;
    }

    read_stat()
}

// ─── Monsters ────────────────────────────────────────────────────────────────

unsafe fn rs_write_monsters(savef: *mut CFile, m: *mut CMonster, count: c_int) -> c_int {
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

unsafe fn rs_read_monsters(inf: *mut CFile, m: *mut CMonster, count: c_int) -> c_int {
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
    let _ = rs_write_char(savef, (*op).o_packch as c_char);
    let _ = rs_write_chars(savef, (&raw mut (*op).o_damage) as *mut c_char, 8);
    let _ = rs_write_chars(savef, (&raw mut (*op).o_hurldmg) as *mut c_char, 8);
    let _ = rs_write_int(savef, (*op).o_count);
    let _ = rs_write_int(savef, (*op).o_which);
    let _ = rs_write_int(savef, (*op).o_hplus);
    let _ = rs_write_int(savef, (*op).o_dplus);
    let _ = rs_write_int(savef, (*op).o_arm);
    let _ = rs_write_int(savef, (*op).o_flags.bits());
    let _ = rs_write_int(savef, (*op).o_group);
    let label_c = (*op)
        .o_label
        .as_ref()
        .and_then(|label| std::ffi::CString::new(label.as_str()).ok());
    let _ = rs_write_string(
        savef,
        label_c
            .as_ref()
            .map_or(std::ptr::null(), |label| label.as_ptr()),
    );

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
    let mut packch_ch: c_char = 0;
    let _ = rs_read_char(inf, &mut packch_ch);
    (*op).o_packch = packch_ch as u8;
    let _ = rs_read_chars(inf, (&raw mut (*op).o_damage) as *mut c_char, 8);
    let _ = rs_read_chars(inf, (&raw mut (*op).o_hurldmg) as *mut c_char, 8);
    let _ = rs_read_int(inf, &mut (*op).o_count);
    let _ = rs_read_int(inf, &mut (*op).o_which);
    let _ = rs_read_int(inf, &mut (*op).o_hplus);
    let _ = rs_read_int(inf, &mut (*op).o_dplus);
    let _ = rs_read_int(inf, &mut (*op).o_arm);
    let mut o_flags_bits: c_int = 0;
    let _ = rs_read_int(inf, &mut o_flags_bits);
    (*op).o_flags = crate::entity::player::ObjectFlags::from_bits(o_flags_bits);
    let _ = rs_read_int(inf, &mut (*op).o_group);
    let _ = rs_read_new_cstring(inf, &mut (*op).o_label);

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
        l = crate::entity::player::thing_next(l);
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

        crate::entity::player::set_thing_prev(l, previous);

        if !previous.is_null() {
            crate::entity::player::set_thing_next(previous, l);
        }

        let _ = rs_read_object(inf, l);

        if previous.is_null() {
            head = l;
        }

        previous = l;
        i += 1;
    }

    if !l.is_null() {
        crate::entity::player::set_thing_next(l, std::ptr::null_mut());
    }

    *list = head;

    read_stat()
}

unsafe fn rs_write_object_reference(
    savef: *mut CFile,
    list: *mut CThing,
    item: *mut CThing,
) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let i = find_list_ptr(list, item as *const c_void);

    rs_write_int(savef, i)
}

unsafe fn rs_read_object_reference(
    inf: *mut CFile,
    list: *mut CThing,
    item: *mut *mut CThing,
) -> c_int {
    let mut i: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_int(inf, &mut i);

    *item = get_list_item(list, i);

    read_stat()
}

// ─── Thing serialization ─────────────────────────────────────────────────────

unsafe fn find_room_coord(c: *mut IVec2) -> c_int {
    let mut i: c_int = 0;

    while (i as usize) < crate::config::GameConfig::MAX_ROOMS {
        if crate::game::room_gold_ptr(Some(i as usize)) == c {
            return i;
        }
        i += 1;
    }

    -1
}

unsafe fn find_thing_coord(monlist: *mut CThing, c: *mut IVec2) -> c_int {
    let mut mitem: *mut CThing = monlist;
    let mut i: c_int = 0;

    while !mitem.is_null() {
        if c == (&raw mut (*thing_t(mitem)).t_pos) as *mut IVec2 {
            return i;
        }
        i += 1;
        mitem = crate::entity::player::thing_next(mitem);
    }

    -1
}

unsafe fn find_object_coord(objlist: *mut CThing, c: *mut IVec2) -> c_int {
    let mut oitem: *mut CThing = objlist;
    let mut i: c_int = 0;

    while !oitem.is_null() {
        if c == (&raw mut (*thing_o(oitem)).o_pos) as *mut IVec2 {
            return i;
        }
        i += 1;
        oitem = crate::entity::player::thing_next(oitem);
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
    let _ = rs_write_char(savef, (*thing_t(t)).t_type as c_char);
    let _ = rs_write_char(savef, (*thing_t(t)).t_disguise as c_char);
    let _ = rs_write_char(savef, (*thing_t(t)).t_oldch as c_char);

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

    let hero_pos_ptr = (&raw mut (*thing_t(crate::game::player_ptr())).t_pos) as *mut IVec2;
    let t_dest = crate::entity::player::thing_dest(t);

    if t_dest == hero_pos_ptr {
        let _ = rs_write_int(savef, 0);
        let _ = rs_write_int(savef, 1);
    } else if !t_dest.is_null() {
        i = find_thing_coord(MLIST.head(), t_dest);

        if i >= 0 {
            let _ = rs_write_int(savef, 1);
            let _ = rs_write_int(savef, i);
        } else {
            i = find_object_coord(crate::game::with_current_level(|level| level.items.head()), t_dest);

            if i >= 0 {
                let _ = rs_write_int(savef, 2);
                let _ = rs_write_int(savef, i);
            } else {
                i = find_room_coord(t_dest);

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

    let _ = rs_write_short(savef, (*thing_t(t)).t_flags.bits());
    let _ = rs_write_stats(savef, &raw mut (*thing_t(t)).t_stats);
    let _ = rs_write_room_reference(savef, (*thing_t(t)).t_room);
    let _ = rs_write_object_list(savef, crate::entity::player::thing_pack(t));

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
    let mut turn_byte: c_uchar = 0;
    let _ = rs_read_boolean(inf, &mut turn_byte);
    (*thing_t(t)).t_turn = turn_byte != 0;
    let mut type_ch: c_char = 0;
    let _ = rs_read_char(inf, &mut type_ch);
    (*thing_t(t)).t_type = type_ch as u8;
    let mut disguise_ch: c_char = 0;
    let _ = rs_read_char(inf, &mut disguise_ch);
    (*thing_t(t)).t_disguise = disguise_ch as u8;
    let mut oldch_ch: c_char = 0;
    let _ = rs_read_char(inf, &mut oldch_ch);
    (*thing_t(t)).t_oldch = oldch_ch as u8;

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
            crate::entity::player::set_thing_dest(
                t,
                (&raw mut (*thing_t(crate::game::player_ptr())).t_pos) as *mut IVec2,
            );
        } else {
            crate::entity::player::set_thing_dest(t, std::ptr::null_mut());
        }
    } else if listid == 1 {
        /* monster/thing */
        crate::entity::player::set_thing_dest(t, std::ptr::null_mut());
        (*thing_t(t)).t_reserved = index;
    } else if listid == 2 {
        /* object */
        let item = get_list_item(crate::game::with_current_level(|level| level.items.head()), index);

        if !item.is_null() {
            crate::entity::player::set_thing_dest(t, (&raw mut (*thing_o(item)).o_pos) as *mut IVec2);
        }
    } else if listid == 3 {
        /* gold */
        if (index as usize) < crate::config::GameConfig::MAX_ROOMS {
            crate::entity::player::set_thing_dest(t, crate::game::room_gold_ptr(Some(index as usize)));
        } else {
            crate::entity::player::set_thing_dest(t, std::ptr::null_mut());
        }
    } else {
        crate::entity::player::set_thing_dest(t, std::ptr::null_mut());
    }

    let mut t_flags_bits: c_short = 0;
    let _ = rs_read_short(inf, &mut t_flags_bits);
    (*thing_t(t)).t_flags = crate::entity::player::MonsterFlags::from_bits(t_flags_bits);
    let _ = rs_read_stats(inf, &raw mut (*thing_t(t)).t_stats);
    let _ = rs_read_room_reference(inf, &mut (*thing_t(t)).t_room);
    let mut pack_head: *mut CThing = std::ptr::null_mut();
    let _ = rs_read_object_list(inf, &mut pack_head);
    crate::entity::player::set_thing_pack(t, pack_head);

    read_stat()
}

/// Resolves a deferred monster chase target stored in t_reserved.
///
/// Uses globals: mlist.
unsafe fn rs_fix_thing(t: *mut CThing) {
    if (*thing_t(t)).t_reserved < 0 {
        return;
    }

    let item = get_list_item(MLIST.head(), (*thing_t(t)).t_reserved);

    if !item.is_null() {
        crate::entity::player::set_thing_dest(t, (&raw mut (*thing_t(item)).t_pos) as *mut IVec2);
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
        l = crate::entity::player::thing_next(l);
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
        l = new_actor();

        crate::entity::player::set_thing_prev(l, previous);

        if !previous.is_null() {
            crate::entity::player::set_thing_next(previous, l);
        }

        let _ = rs_read_thing(inf, l);

        if previous.is_null() {
            head = l;
        }

        previous = l;
        i += 1;
    }

    if !l.is_null() {
        crate::entity::player::set_thing_next(l, std::ptr::null_mut());
    }

    *list = head;

    read_stat()
}

unsafe fn rs_fix_thing_list(list: *mut CThing) {
    let mut item: *mut CThing = list;

    while !item.is_null() {
        rs_fix_thing(item);
        item = crate::entity::player::thing_next(item);
    }
}

unsafe fn rs_write_thing_reference(
    savef: *mut CFile,
    list: *mut CThing,
    item: *mut CThing,
) -> c_int {
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

unsafe fn rs_read_thing_reference(
    inf: *mut CFile,
    list: *mut CThing,
    item: *mut *mut CThing,
) -> c_int {
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

    crate::game::with_current_level(|lvl| {
        let mut i: c_int = 0;
        while i < count {
            let y = i / crate::config::GameConfig::SCREEN_COLS;
            let x = i % crate::config::GameConfig::SCREEN_COLS;
            let idx = (y as usize) * crate::config::GameConfig::LEVEL_WIDTH + (x as usize);
            let tile = lvl
                .map
                .get(y as usize, x as usize)
                .unwrap_or(crate::level::Tile::Empty);
            let _ = rs_write_char(savef, tile.to_u8() as c_char);
            let _ = rs_write_boolean(savef, lvl.flags.real[idx] as c_int);
            let _ = rs_write_boolean(savef, lvl.flags.passage[idx] as c_int);
            let _ = rs_write_boolean(savef, lvl.flags.seen[idx] as c_int);
            let _ = rs_write_char(savef, lvl.flags.passnum[idx] as c_char);
            let _ = rs_write_char(savef, tile.trap() as u8 as c_char);
            // Per-cell monster occupancy.
            let _ = rs_write_thing_reference(
                savef,
                MLIST.head(),
                lvl.monsters.at(y as usize, x as usize),
            );
            i += 1;
        }

        WRITE_ERROR
    })
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

    crate::game::with_current_level_mut(|lvl| {
        let mut i: c_int = 0;
        while i < count {
            let y = i / crate::config::GameConfig::SCREEN_COLS;
            let x = i % crate::config::GameConfig::SCREEN_COLS;
            let idx = (y as usize) * crate::config::GameConfig::LEVEL_WIDTH + (x as usize);

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
            let _ = rs_read_thing_reference(inf, MLIST.head(), &mut monst);

            let trap = crate::level::Trap::from_raw(trap_kind as u8);
            let tile =
                crate::level::Tile::from_u8(tile_disc as u8).unwrap_or(crate::level::Tile::Empty);
            let tile = match tile {
                crate::level::Tile::Trap(_) => crate::level::Tile::Trap(trap),
                other => other,
            };
            let _ = lvl.map.set(y as usize, x as usize, tile);
            lvl.flags.real[idx] = real != 0;
            lvl.flags.passage[idx] = passage != 0;
            lvl.flags.seen[idx] = seen != 0;
            lvl.flags.passnum[idx] = passnum as u8;

            // Per-cell monster occupancy.
            lvl.monsters.set(y as usize, x as usize, monst);
            i += 1;
        }

        read_stat()
    })
}

// ─── Whole-game save / restore ───────────────────────────────────────────────

/// Writes the entire game state to the save file.
///
/// Uses globals: after, again, noscore, seenstairs, amulet, door_stop,
/// fight_flush, firstmove, got_ltc, has_hit, in_shell, inv_describe,
/// jump, kamikaze, lower_msg, move_on, msg_esc, passgo, playing,
/// q_comm, running, save_msg, see_floor, stat_msg, terse, to_death,
/// tombstone, wizard, pack_used, dir_ch, file_name, huh, p_colors,
/// prbuf, r_stones, stones, release, runch, s_names, take,
/// whoami, ws_made, ws_type, wood, metal, orig_dsusp, fruit, home,
/// inv_t_name, l_last_comm, l_last_dir, last_comm, last_dir, tr_name,
/// n_objs, ntraps, hungry_state, inpack, inv_type, level, max_level,
/// mpos, no_food, a_class, count, food_left, lastscore, no_command,
/// no_move, purse, quiet, vf_hit, dnum, seed, e_levels, delta, oldpos,
/// stairs, player, equipment slots, l_last_pick,
/// last_pick, lvl_obj, mlist, places, max_stats, rooms, oldrp,
/// passages, monsters, things, arm_info, pot_info, ring_info,
/// scr_info, weap_info, ws_info, d_list, total, between, nh, group.
#[no_mangle]
pub unsafe extern "C" fn rs_save_file(savef: *mut CFile) -> c_int {
    if WRITE_ERROR != 0 {
        return WRITE_ERROR;
    }

    let _ = rs_write_boolean(savef, after as c_int); /* 1  */
    /* extern.c */
    let _ = rs_write_boolean(savef, again as c_int); /* 2  */
    let _ = rs_write_int(savef, noscore); /* 3  */
    let _ = rs_write_boolean(savef, seenstairs as c_int); /* 4  */
    let _ = rs_write_boolean(savef, amulet as c_int); /* 5  */
    let _ = rs_write_boolean(savef, door_stop as c_int); /* 6  */
    let _ = rs_write_boolean(savef, fight_flush as c_int); /* 7  */
    let _ = rs_write_boolean(savef, firstmove as c_int); /* 8  */
    let _ = rs_write_boolean(savef, got_ltc as c_int); /* 9  */
    let _ = rs_write_boolean(savef, has_hit as c_int); /* 10 */
    let _ = rs_write_boolean(savef, in_shell as c_int); /* 11 */
    let _ = rs_write_boolean(savef, inv_describe as c_int); /* 12 */
    let _ = rs_write_boolean(savef, jump as c_int); /* 13 */
    let _ = rs_write_boolean(savef, kamikaze as c_int); /* 14 */
    let _ = rs_write_boolean(savef, lower_msg as c_int); /* 15 */
    let _ = rs_write_boolean(savef, move_on as c_int); /* 16 */
    let _ = rs_write_boolean(savef, msg_esc as c_int); /* 17 */
    let _ = rs_write_boolean(savef, passgo as c_int); /* 18 */
    let _ = rs_write_boolean(savef, playing as c_int); /* 19 */
    let _ = rs_write_boolean(savef, q_comm as c_int); /* 20 */
    let _ = rs_write_boolean(savef, running as c_int); /* 21 */
    let _ = rs_write_boolean(savef, save_msg as c_int); /* 22 */
    let _ = rs_write_boolean(savef, see_floor as c_int); /* 23 */
    let _ = rs_write_boolean(savef, stat_msg as c_int); /* 24 */
    let _ = rs_write_boolean(savef, terse as c_int); /* 25 */
    let _ = rs_write_boolean(savef, to_death as c_int); /* 26 */
    let _ = rs_write_boolean(savef, tombstone as c_int); /* 27 */
    if MASTER {
        let _ = rs_write_int(savef, wizard); /* 28 */
    } else {
        let _ = rs_write_int(savef, 0); /* 28 */
    }
    let _ = rs_write_booleans(savef, (&raw mut pack_used) as *mut c_uchar, 26); /* 29 */
    let _ = rs_write_char(savef, dir_ch);
    let _ = rs_write_chars(savef, (&raw mut file_name) as *mut c_char, MAXSTR as c_int);
    let _ = rs_write_chars(savef, (&raw mut huh) as *mut c_char, MAXSTR as c_int);
    let _ = rs_write_potions(savef);
    let _ = rs_write_chars(
        savef,
        (&raw mut prbuf) as *mut c_char,
        (2 * MAXSTR) as c_int,
    );
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
    let _ = rs_write_int(savef, crate::game::current_depth());
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
    let _ = rs_write_coord(savef, crate::game::stairs());

    let _ = rs_write_thing(savef, crate::game::player_ptr());
    let player_pack = crate::entity::player::thing_pack(crate::game::player_ptr());
    let _ = rs_write_object_reference(savef, player_pack, EQUIPMENT.armor());
    let _ = rs_write_object_reference(savef, player_pack, EQUIPMENT.left_ring());
    let _ = rs_write_object_reference(savef, player_pack, EQUIPMENT.right_ring());
    let _ = rs_write_object_reference(savef, player_pack, EQUIPMENT.weapon());
    let _ = rs_write_object_reference(savef, player_pack, l_last_pick);
    let _ = rs_write_object_reference(savef, player_pack, last_pick);

    let _ = rs_write_object_list(savef, crate::game::with_current_level(|level| level.items.head()));
    let _ = rs_write_thing_list(savef, MLIST.head());

    let _ = rs_write_places(
        savef,
        crate::config::GameConfig::SCREEN_LINES * crate::config::GameConfig::SCREEN_COLS,
    );

    let _ = rs_write_stats(savef, &raw mut max_stats);
    let _ = crate::game::with_current_level(|level| rs_write_rooms(savef, &level.rooms));
    let _ = rs_write_room_reference(savef, oldrp);
    let _ = crate::game::with_current_level(|level| {
        rs_write_passage_links(savef, &level.passage_links)
    });

    let _ = rs_write_monsters(
        savef,
        (&raw mut monsters) as *mut CMonster,
        MAXMONSTERS as c_int,
    );
    let _ = rs_write_obj_info(
        savef,
        (&raw mut things) as *mut CObjInfo,
        NUMTHINGS as c_int,
    );
    let _ = rs_write_obj_info(
        savef,
        (&raw mut arm_info) as *mut CObjInfo,
        MAXARMORS as c_int,
    );
    let _ = rs_write_obj_info(
        savef,
        (&raw mut pot_info) as *mut CObjInfo,
        MAXPOTIONS as c_int,
    );
    let _ = rs_write_obj_info(
        savef,
        (&raw mut ring_info) as *mut CObjInfo,
        MAXRINGS as c_int,
    );
    let _ = rs_write_obj_info(
        savef,
        (&raw mut scr_info) as *mut CObjInfo,
        MAXSCROLLS as c_int,
    );
    let _ = rs_write_obj_info(
        savef,
        (&raw mut weap_info) as *mut CObjInfo,
        (MAXWEAPONS + 1) as c_int,
    );
    let _ = rs_write_obj_info(
        savef,
        (&raw mut ws_info) as *mut CObjInfo,
        MAXSTICKS as c_int,
    );

    let _ = rs_write_daemons(
        savef,
        (&raw mut d_list) as *mut CDelayedAction,
        MAXDAEMONS as c_int,
    );
    if MASTER {
        let _ = rs_write_int(savef, allocated_count()); /* 5.4-list.c */
    } else {
        let _ = rs_write_int(savef, 0);
    }
    let _ = rs_write_int(savef, between); /* 5.4-daemons.c */
    let _ = rs_write_coord(savef, nh); /* 5.4-move.c */
    let _ = rs_write_int(savef, group); /* 5.4-weapons.rs */

    let _ = rs_write_window(savef);

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
/// prbuf, r_stones, stones, release, runch, s_names, take,
/// whoami, ws_made, ws_type, wood, metal, orig_dsusp, fruit, home,
/// inv_t_name, l_last_comm, l_last_dir, last_comm, last_dir, tr_name,
/// n_objs, ntraps, hungry_state, inpack, inv_type, level, max_level,
/// mpos, no_food, a_class, count, food_left, lastscore, no_command,
/// no_move, purse, quiet, vf_hit, dnum, seed, e_levels, delta, oldpos,
/// stairs, player, equipment slots, l_last_pick,
/// last_pick, lvl_obj, mlist, places, max_stats, rooms, oldrp,
/// passages, monsters, things, arm_info, pot_info, ring_info,
/// scr_info, weap_info, ws_info, d_list, total, between, nh, group.
#[no_mangle]
pub unsafe extern "C" fn rs_restore_file(inf: *mut CFile) -> c_int {
    let mut dummyint: c_int = 0;
    let mut depth: c_int = 0;

    if READ_ERROR != 0 || FORMAT_ERROR != 0 {
        return read_stat();
    }

    let _ = rs_read_boolean(inf, &mut after); /* 1  */
    /* extern.c */
    let _ = rs_read_boolean(inf, &mut again); /* 2  */
    let _ = rs_read_int(inf, &mut noscore); /* 3  */
    let _ = rs_read_boolean(inf, &mut seenstairs); /* 4  */
    let _ = rs_read_boolean(inf, &mut amulet); /* 5  */
    let _ = rs_read_boolean(inf, &mut door_stop); /* 6  */
    let _ = rs_read_boolean(inf, &mut fight_flush); /* 7  */
    let _ = rs_read_boolean(inf, &mut firstmove); /* 8  */
    let _ = rs_read_boolean(inf, &mut got_ltc); /* 9  */
    let _ = rs_read_boolean(inf, &mut has_hit); /* 10 */
    let _ = rs_read_boolean(inf, &mut in_shell); /* 11 */
    let _ = rs_read_boolean(inf, &mut inv_describe); /* 12 */
    let _ = rs_read_boolean(inf, &mut jump); /* 13 */
    let _ = rs_read_boolean(inf, &mut kamikaze); /* 14 */
    let _ = rs_read_boolean(inf, &mut lower_msg); /* 15 */
    let _ = rs_read_boolean(inf, &mut move_on); /* 16 */
    let mut msg_esc_byte = msg_esc as c_uchar;
    let _ = rs_read_boolean(inf, &mut msg_esc_byte); /* 17 */
    msg_esc = msg_esc_byte != 0;
    let _ = rs_read_boolean(inf, &mut passgo); /* 18 */
    let _ = rs_read_boolean(inf, &mut playing); /* 19 */
    let _ = rs_read_boolean(inf, &mut q_comm); /* 20 */
    let _ = rs_read_boolean(inf, &mut running); /* 21 */
    let _ = rs_read_boolean(inf, &mut save_msg); /* 22 */
    let _ = rs_read_boolean(inf, &mut see_floor); /* 23 */
    let _ = rs_read_boolean(inf, &mut stat_msg); /* 24 */
    let _ = rs_read_boolean(inf, &mut terse); /* 25 */
    let _ = rs_read_boolean(inf, &mut to_death); /* 26 */
    let _ = rs_read_boolean(inf, &mut tombstone); /* 27 */
    if MASTER {
        let _ = rs_read_int(inf, &mut wizard); /* 28 */
    } else {
        let _ = rs_read_int(inf, &mut dummyint); /* 28 */
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
    let _ = rs_read_int(inf, &mut depth);
    crate::game::set_current_depth(depth);
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
    let mut stairs = IVec2::ZERO;
    let _ = rs_read_coord(inf, &mut stairs);
    crate::game::set_stairs(stairs);

    let _ = rs_read_thing(inf, crate::game::player_ptr());
    let player_pack = crate::entity::player::thing_pack(crate::game::player_ptr());
    let mut equipment_item = std::ptr::null_mut();
    let _ = rs_read_object_reference(inf, player_pack, &raw mut equipment_item);
    EQUIPMENT.set_armor(equipment_item);
    let _ = rs_read_object_reference(inf, player_pack, &raw mut equipment_item);
    EQUIPMENT.set_left_ring(equipment_item);
    let _ = rs_read_object_reference(inf, player_pack, &raw mut equipment_item);
    EQUIPMENT.set_right_ring(equipment_item);
    let _ = rs_read_object_reference(inf, player_pack, &raw mut equipment_item);
    EQUIPMENT.set_weapon(equipment_item);
    let _ = rs_read_object_reference(inf, player_pack, &raw mut l_last_pick);
    let _ = rs_read_object_reference(inf, player_pack, &raw mut last_pick);

    let mut items_head: *mut CThing = std::ptr::null_mut();
    let _ = rs_read_object_list(inf, &raw mut items_head);
    crate::game::with_current_level_mut(|level| level.items.set_head(items_head));
    let mut mlist: *mut CThing = std::ptr::null_mut();
    let _ = rs_read_thing_list(inf, &raw mut mlist);
    MLIST.set_head(mlist);
    rs_fix_thing(crate::game::player_ptr());
    rs_fix_thing_list(mlist);

    let _ = rs_read_places(
        inf,
        crate::config::GameConfig::SCREEN_LINES * crate::config::GameConfig::SCREEN_COLS,
    );

    let _ = rs_read_stats(inf, &raw mut max_stats);
    let _ = crate::game::with_current_level_mut(|level| rs_read_rooms(inf, &mut level.rooms));
    let _ = rs_read_room_reference(inf, &mut oldrp);
    let _ = crate::game::with_current_level_mut(|level| {
        rs_read_passage_links(inf, &mut level.passage_links)
    });

    let _ = rs_read_monsters(
        inf,
        (&raw mut monsters) as *mut CMonster,
        MAXMONSTERS as c_int,
    );
    let _ = rs_read_obj_info(inf, (&raw mut things) as *mut CObjInfo, NUMTHINGS as c_int);
    let _ = rs_read_obj_info(
        inf,
        (&raw mut arm_info) as *mut CObjInfo,
        MAXARMORS as c_int,
    );
    let _ = rs_read_obj_info(
        inf,
        (&raw mut pot_info) as *mut CObjInfo,
        MAXPOTIONS as c_int,
    );
    let _ = rs_read_obj_info(
        inf,
        (&raw mut ring_info) as *mut CObjInfo,
        MAXRINGS as c_int,
    );
    let _ = rs_read_obj_info(
        inf,
        (&raw mut scr_info) as *mut CObjInfo,
        MAXSCROLLS as c_int,
    );
    let _ = rs_read_obj_info(
        inf,
        (&raw mut weap_info) as *mut CObjInfo,
        (MAXWEAPONS + 1) as c_int,
    );
    let _ = rs_read_obj_info(inf, (&raw mut ws_info) as *mut CObjInfo, MAXSTICKS as c_int);

    let _ = rs_read_daemons(
        inf,
        (&raw mut d_list) as *mut CDelayedAction,
        MAXDAEMONS as c_int,
    );
    let _ = rs_read_int(inf, &mut dummyint); /* total */
    /* 5.4-list.c */
    let _ = rs_read_int(inf, &mut between); /* 5.4-daemons.c */
    let _ = rs_read_coord(inf, &mut nh); /* 5.4-move.c */
    let _ = rs_read_int(inf, &mut group); /* 5.4-weapons.rs */

    let _ = rs_read_window(inf);

    read_stat()
}
