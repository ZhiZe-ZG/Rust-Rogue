use crate::player::{CThing, CThingObject};
use crate::potions::invis_on;
use crate::rnd::rnd;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};

use crate::game::EQUIPMENT;
use crate::io::{addmsg_str, msg_str, readchar};
use crate::misc::{aggravate, chg_str, is_current};
use crate::pack::get_item;
use crate::things::{dropcheck, inv_name};
use crate::weapons::num;

const LEFT: usize = 0;
const RIGHT: usize = 1;
const RING_TYPE: c_int = '=' as c_int;
const ESCAPE: u8 = 27;
const ISKNOW: c_int = 0o000002;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingType {
    Protection = 0,
    AddStrength = 1,
    SustainStrength = 2,
    Searching = 3,
    SeeInvisible = 4,
    Adornment = 5,
    Aggravate = 6,
    AddHit = 7,
    AddDamage = 8,
    Regeneration = 9,
    Digest = 10,
    Teleport = 11,
    Stealth = 12,
    SustainArmor = 13,
}

impl RingType {
    pub const COUNT: usize = 14;

    #[inline]
    pub const fn from_raw(value: c_int) -> Option<Self> {
        match value {
            0 => Some(Self::Protection),
            1 => Some(Self::AddStrength),
            2 => Some(Self::SustainStrength),
            3 => Some(Self::Searching),
            4 => Some(Self::SeeInvisible),
            5 => Some(Self::Adornment),
            6 => Some(Self::Aggravate),
            7 => Some(Self::AddHit),
            8 => Some(Self::AddDamage),
            9 => Some(Self::Regeneration),
            10 => Some(Self::Digest),
            11 => Some(Self::Teleport),
            12 => Some(Self::Stealth),
            13 => Some(Self::SustainArmor),
            _ => None,
        }
    }

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

const USES: [c_int; RingType::COUNT] = [
    1,  // Protection
    1,  // AddStrength
    1,  // SustainStrength
    -3, // Searching
    -5, // SeeInvisible
    0,  // Adornment
    0,  // Aggravate
    -3, // AddHit
    -3, // AddDamage
    2,  // Regeneration
    -2, // Digest
    0,  // Teleport
    1,  // Stealth
    1,  // SustainArmor
];

unsafe extern "C" {
    static mut terse: c_uchar;
    static mut mpos: c_int;

    fn snprintf(s: *mut c_char, n: usize, fmt: *const c_char, ...) -> c_int;
}

static mut RING_NUM_BUF: [c_char; 10] = [0; 10];

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

/// Prompts for a ring and equips it on an available hand, applying immediate ring effects.
#[no_mangle]
pub unsafe extern "C" fn ring_on() {
    let obj = get_item(c"put on".as_ptr(), RING_TYPE);
    if obj.is_null() {
        return;
    }
    if (*thing_o(obj)).o_type != RING_TYPE {
        if terse == 0 {
            msg_str("it would be difficult to wrap that around a finger");
        } else {
            msg_str("not a ring");
        }
        return;
    }

    if is_current(obj) {
        return;
    }

    let left_hand = if EQUIPMENT.left_ring().is_null() && EQUIPMENT.right_ring().is_null() {
        let hand = gethand();
        if hand < 0 {
            return;
        }
        hand as usize == LEFT
    } else if EQUIPMENT.left_ring().is_null() {
        true
    } else if EQUIPMENT.right_ring().is_null() {
        false
    } else {
        if terse == 0 {
            msg_str("you already have a ring on each hand");
        } else {
            msg_str("wearing two");
        }
        return;
    };

    if left_hand {
        EQUIPMENT.set_left_ring(obj);
    } else {
        EQUIPMENT.set_right_ring(obj);
    }

    match RingType::from_raw((*thing_o(obj)).o_which) {
        Some(RingType::AddStrength) => chg_str((*thing_o(obj)).o_arm),
        Some(RingType::SeeInvisible) => invis_on(),
        Some(RingType::Aggravate) => aggravate(),
        _ => {}
    }

    if terse == 0 {
        addmsg_str("you are now wearing ");
    }
    msg_str(&format!(
        "{} ({})",
        CStr::from_ptr(inv_name(obj, 1)).to_string_lossy(),
        (*thing_o(obj)).o_packch as u8 as char,
    ));
}

/// Removes a worn ring from the chosen hand after passing drop constraints.
#[no_mangle]
pub unsafe extern "C" fn ring_off() {
    let left_hand = if EQUIPMENT.left_ring().is_null() && EQUIPMENT.right_ring().is_null() {
        if terse != 0 {
            msg_str("no rings");
        } else {
            msg_str("you aren't wearing any rings");
        }
        return;
    } else if EQUIPMENT.left_ring().is_null() {
        false
    } else if EQUIPMENT.right_ring().is_null() {
        true
    } else {
        let hand = gethand();
        if hand < 0 {
            return;
        }
        hand as usize == LEFT
    };

    mpos = 0;
    let obj = if left_hand {
        EQUIPMENT.left_ring()
    } else {
        EQUIPMENT.right_ring()
    };
    if obj.is_null() {
        msg_str("not wearing such a ring");
        return;
    }

    if dropcheck(obj) != 0 {
        msg_str(&format!(
            "was wearing {}({})",
            CStr::from_ptr(inv_name(obj, 1)).to_string_lossy(),
            (*thing_o(obj)).o_packch as u8 as char,
        ));
    }
}

/// Asks which hand the player means and returns LEFT, RIGHT, or -1 on escape.
#[no_mangle]
pub unsafe extern "C" fn gethand() -> c_int {
    loop {
        if terse != 0 {
            msg_str("left or right ring? ");
        } else {
            msg_str("left hand or right hand? ");
        }

        let c = readchar() as u8;
        if c == ESCAPE {
            return -1;
        }

        mpos = 0;
        if c == b'l' || c == b'L' {
            return LEFT as c_int;
        }
        if c == b'r' || c == b'R' {
            return RIGHT as c_int;
        }

        if terse != 0 {
            msg_str("L or R");
        } else {
            msg_str("please type L or R");
        }
    }
}

/// Computes per-turn food impact for the ring on the given hand.
#[no_mangle]
pub unsafe extern "C" fn ring_eat(hand: c_int) -> c_int {
    let hand_idx = hand as usize;
    if hand_idx > RIGHT {
        return 0;
    }

    let ring = match hand_idx {
        LEFT => EQUIPMENT.left_ring(),
        RIGHT => EQUIPMENT.right_ring(),
        _ => return 0,
    };
    if ring.is_null() {
        return 0;
    }

    let ring_type = match RingType::from_raw((*thing_o(ring)).o_which) {
        Some(ring_type) => ring_type,
        None => return 0,
    };

    let mut eat = USES[ring_type.index()];
    if eat < 0 {
        eat = if rnd(-eat) == 0 { 1 } else { 0 };
    }
    if ring_type == RingType::Digest {
        eat = -eat;
    }
    eat
}

/// Returns bracketed ring bonus text for known stat-modifier rings.
unsafe fn ring_num(obj: *mut CThing) -> *mut c_char {
    if obj.is_null() {
        return c"".as_ptr() as *mut c_char;
    }
    if ((*thing_o(obj)).o_flags & ISKNOW) == 0 {
        return c"".as_ptr() as *mut c_char;
    }

    match RingType::from_raw((*thing_o(obj)).o_which) {
        Some(
            RingType::Protection | RingType::AddStrength | RingType::AddDamage | RingType::AddHit,
        ) => {
            let _ = snprintf(
                (&raw mut RING_NUM_BUF) as *mut c_char,
                10,
                c" [%s]".as_ptr(),
                num((*thing_o(obj)).o_arm, 0, RING_TYPE as c_char),
            );
            (&raw mut RING_NUM_BUF) as *mut c_char
        }
        _ => c"".as_ptr() as *mut c_char,
    }
}

#[cfg(test)]
mod tests {
    use super::RingType;

    #[test]
    fn ring_types_match_the_legacy_ids() {
        let ring_types = [
            RingType::Protection,
            RingType::AddStrength,
            RingType::SustainStrength,
            RingType::Searching,
            RingType::SeeInvisible,
            RingType::Adornment,
            RingType::Aggravate,
            RingType::AddHit,
            RingType::AddDamage,
            RingType::Regeneration,
            RingType::Digest,
            RingType::Teleport,
            RingType::Stealth,
            RingType::SustainArmor,
        ];

        assert_eq!(ring_types.len(), RingType::COUNT);
        for (index, ring_type) in ring_types.into_iter().enumerate() {
            assert_eq!(ring_type.index(), index);
            assert_eq!(RingType::from_raw(index as i32), Some(ring_type));
        }
        assert_eq!(RingType::from_raw(-1), None);
        assert_eq!(RingType::from_raw(RingType::COUNT as i32), None);
    }
}
