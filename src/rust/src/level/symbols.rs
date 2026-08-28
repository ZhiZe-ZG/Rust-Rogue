//! Legacy C ABI surface for the level module.
//!
//! Centralizes every raw `extern` declaration the C engine exposes (globals
//! like `places`, `rooms`, `player`, and callable helpers) plus the constants
//! that mirror `rogue.h` (room/thing flags, treasure tuning, glyphs). Sibling
//! modules import from here instead of redeclaring or re-hardcoding them.

use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::player::{CCoord, CPlace, CRoom, CThing, CThingMonster, CThingObject};

use super::passages::MAX_PASSAGES;
use super::roomgraph::MAX_ROOMS;

/// Number of room slots on a level (mirrors C `MAXROOMS`).
pub(crate) const MAXROOMS: usize = MAX_ROOMS;

// -- Room flags (`r_flags`) --
pub(crate) const ISDARK: c_short = 0o000001;
pub(crate) const ISGONE: c_short = 0o000002;
pub(crate) const ISMAZE: c_short = 0o000004;

// -- Object/thing flags --
pub(crate) const ISMANY: c_int = 0o0000010;
pub(crate) const ISMEAN: c_short = 0o0004000;
pub(crate) const ISHELD: c_short = 0o0000400;
pub(crate) const SEEMONST: c_short = 0o040000;
pub(crate) const ISHALU: c_short = 0o0004000;

// -- Glyphs --
pub(crate) const AMULET: c_char = b',' as c_char;
pub(crate) const GOLD: c_char = b'*' as c_char;
pub(crate) const PLAYER: c_char = b'@' as c_char;

// -- Treasure/object tuning --
pub(crate) const MAXTREAS: c_int = 10;
pub(crate) const MINTREAS: c_int = 2;
pub(crate) const MAXTRIES: c_int = 10;
pub(crate) const MAXOBJ: c_int = 9;
pub(crate) const TREAS_ROOM: c_int = 20;
pub(crate) const AMULETLEVEL: c_int = 26;
pub(crate) const GOLDGRP: c_int = 1;

// -- Traps --
pub(crate) const MAXTRAPS: c_int = 10;
pub(crate) const NTRAPS: c_int = 8;

// -- C booleans (flow through `c_uchar`) --
pub(crate) const FALSE: c_uchar = 0;
pub(crate) const TRUE: c_uchar = 1;

unsafe extern "C" {
    pub(crate) static mut level: c_int;
    pub(crate) static mut max_level: c_int;
    pub(crate) static mut amulet: c_uchar;
    pub(crate) static mut rooms: [CRoom; MAXROOMS];
    pub(crate) static mut passages: [CRoom; MAX_PASSAGES];
    pub(crate) static mut lvl_obj: *mut CThing;
    pub(crate) static mut player: CThing;
    pub(crate) static mut mlist: *mut CThing;
    pub(crate) static mut no_food: c_int;
    pub(crate) static mut ntraps: c_int;
    pub(crate) static mut stairs: CCoord;
    pub(crate) static mut seenstairs: c_uchar;

    pub(crate) fn wake_monster(y: c_int, x: c_int);
    pub(crate) fn step_ok(ch: c_int) -> c_int;
    pub(crate) fn new_thing() -> *mut CThing;
    pub(crate) fn new_item() -> *mut CThing;
    pub(crate) fn _attach(list: *mut *mut CThing, item: *mut CThing);
    pub(crate) fn randmonster(wander: c_uchar) -> c_char;
    pub(crate) fn new_monster(tp: *mut CThing, kind: c_char, cp: *mut CCoord);
    pub(crate) fn give_pack(tp: *mut CThing);

    pub(crate) fn clear() -> c_int;
    pub(crate) fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int;
    pub(crate) fn enter_room(cp: *mut CCoord);
    pub(crate) fn turn_see(turn_off: c_uchar) -> c_uchar;
    pub(crate) fn _free_list(ptr: *mut *mut CThing);
    pub(crate) fn roomin(cp: *mut CCoord) -> *mut CRoom;
    pub(crate) fn visuals();

    pub(crate) fn r#move(y: c_int, x: c_int) -> c_int;
    pub(crate) fn addch(ch: c_uint) -> c_int;
    pub(crate) fn standout() -> c_int;
    pub(crate) fn standend() -> c_int;
}

/// Interpret `tp` as an object (`CThingObject`).
#[inline]
pub(crate) unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

/// Interpret `tp` as a monster (`CThingMonster`).
#[inline]
pub(crate) unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}