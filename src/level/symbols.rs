//! Legacy C ABI surface for the level module.
//!
//! Centralizes every raw `extern` declaration the C engine exposes (globals
//! like `places`, `rooms`, `player`, and callable helpers) plus the constants
//! that mirror `rogue.h` (room/thing flags, treasure tuning, glyphs). Sibling
//! modules import from here instead of redeclaring or re-hardcoding them.

use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

pub(crate) use crate::daemons::visuals;
pub(crate) use crate::draw::enter_room;
pub(crate) use crate::entity::chase::roomin;
pub(crate) use crate::entity::monsters::{give_pack, new_monster, randmonster, wake_monster};
use crate::entity::player::{CCoord, CPlace, CRoom, CThing, CThingMonster, CThingObject};
pub(crate) use crate::item::potions::turn_see;
pub(crate) use crate::item::thing_list::{attach, free_list, new_item};
pub(crate) use crate::item::things::new_thing;
pub(crate) use crate::level::glyph_is_walkable;
use crate::ui::terminal as cur;

use crate::config::GameConfig;

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

pub(crate) const GOLDGRP: c_int = 1;

// -- C booleans (flow through `c_uchar`) --

unsafe extern "C" {
    pub(crate) static mut level: c_int;
    pub(crate) static mut max_level: c_int;
    pub(crate) static mut amulet: bool;
    pub(crate) static mut rooms: [CRoom; GameConfig::MAX_ROOMS];
    pub(crate) static mut passages: [CRoom; GameConfig::MAX_PASSAGES];
    pub(crate) static mut lvl_obj: *mut CThing;
    pub(crate) static mut player: CThing;
    pub(crate) static mut mlist: *mut CThing;
    pub(crate) static mut no_food: c_int;
    pub(crate) static mut ntraps: c_int;
    pub(crate) static mut stairs: CCoord;
    pub(crate) static mut seenstairs: bool;

}

/// ncurses wrapper re-exports so the rest of the `level` module can keep using
/// the same names but go through the `crate::ui::terminal` shim (which calls the
/// `ncurses` crate) instead of raw C ABI.
pub(crate) use cur::{addch, clear, mvaddch, r#move, standend, standout};

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
