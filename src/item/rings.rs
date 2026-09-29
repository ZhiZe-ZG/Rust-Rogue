//! Rings: putting them on, taking them off, and their magical effects.
//!
//! Ported from `src/c/rings.c` to Rust.
use crate::entity::player::ObjectFlags;
use crate::item::potions::invis_on;
use crate::rnd::rnd;

use crate::game::PLAYER;
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::arena::{ThingId, OBJECTS};
use crate::item::pack::get_item_id;
use crate::item::things::{dropcheck_id, inv_name_id};
use crate::item::weapons::num;
use crate::misc::{aggravate, chg_str, is_current_id};
use crate::ui::input::readchar;
use crate::ui::output::{addmsg_str, msg_str};

const LEFT: usize = 0;
const RIGHT: usize = 1;
const ESCAPE: u8 = 27;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    pub const fn from_raw(value: i32) -> Option<Self> {
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

const USES: [i32; RingType::COUNT] = [
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

use crate::game::globals::{mpos, terse};


/// Prompts for a ring and equips it on an available hand, applying immediate ring effects.
pub unsafe fn ring_on() {
    let Some(obj) = get_item_id("put on", ItemFilter::Category(ItemType::RING)) else {
        return;
    };
    if !matches!(OBJECTS.with_object(obj, |o| o.o_type), Some(ItemType::Ring(_))) {
        if terse == 0 {
            msg_str("it would be difficult to wrap that around a finger");
        } else {
            msg_str("not a ring");
        }
        return;
    }

    if is_current_id(obj) {
        return;
    }

    let eq = PLAYER.equipment();
    let left_empty = eq.left_ring_id().is_none();
    let right_empty = eq.right_ring_id().is_none();
    let left_hand = if left_empty && right_empty {
        let hand = gethand();
        if hand < 0 {
            return;
        }
        hand as usize == LEFT
    } else if left_empty {
        true
    } else if right_empty {
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
        PLAYER.set_left_ring_id(Some(obj));
    } else {
        PLAYER.set_right_ring_id(Some(obj));
    }

    let (which, o_arm, packch) = OBJECTS
        .with_object(obj, |o| (o.o_which, o.o_arm, o.o_packch))
        .unwrap_or((0, 0, 0));

    match RingType::from_raw(which) {
        Some(RingType::AddStrength) => chg_str(o_arm),
        Some(RingType::SeeInvisible) => invis_on(),
        Some(RingType::Aggravate) => aggravate(),
        _ => {}
    }

    if terse == 0 {
        addmsg_str("you are now wearing ");
    }
    msg_str(&format!("{} ({})", inv_name_id(obj, true), packch as char));
}

/// Removes a worn ring from the chosen hand after passing drop constraints.
pub unsafe fn ring_off() {
    let eq = PLAYER.equipment();
    let left_empty = eq.left_ring_id().is_none();
    let right_empty = eq.right_ring_id().is_none();
    let left_hand = if left_empty && right_empty {
        if terse != 0 {
            msg_str("no rings");
        } else {
            msg_str("you aren't wearing any rings");
        }
        return;
    } else if left_empty {
        false
    } else if right_empty {
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
        PLAYER.equipment().left_ring_id()
    } else {
        PLAYER.equipment().right_ring_id()
    };
    let Some(obj) = obj else {
        msg_str("not wearing such a ring");
        return;
    };

    if dropcheck_id(obj) {
        let packch = OBJECTS.with_object(obj, |o| o.o_packch).unwrap_or(0);
        msg_str(&format!(
            "was wearing {}({})",
            inv_name_id(obj, true),
            packch as char,
        ));
    }
}

/// Asks which hand the player means and returns LEFT, RIGHT, or -1 on escape.
pub unsafe fn gethand() -> i32 {
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
            return LEFT as i32;
        }
        if c == b'r' || c == b'R' {
            return RIGHT as i32;
        }

        if terse != 0 {
            msg_str("L or R");
        } else {
            msg_str("please type L or R");
        }
    }
}

/// Computes per-turn food impact for the ring on the given hand.
pub unsafe fn ring_eat(hand: i32) -> i32 {
    let hand_idx = hand as usize;
    if hand_idx > RIGHT {
        return 0;
    }

    let ring_type = match PLAYER.equipment().ring_type(hand_idx) {
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
#[allow(dead_code)]
unsafe fn ring_num(id: ThingId) -> String {
    let Some((know, which, o_arm)) = OBJECTS.with_object(id, |o| {
        (o.o_flags.contains(ObjectFlags::KNOW), o.o_which, o.o_arm)
    }) else {
        return String::new();
    };
    if !know {
        return String::new();
    }

    match RingType::from_raw(which) {
        Some(
            RingType::Protection | RingType::AddStrength | RingType::AddDamage | RingType::AddHit,
        ) => {
            // Not a weapon, so `num` formats a single signed value.
            let inner = num(o_arm, 0, 0);
            format!(" [{}]", inner)
        }
        _ => String::new(),
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
