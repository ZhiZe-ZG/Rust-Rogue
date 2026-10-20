//! Random movement for confused or otherwise disoriented monsters.
//!
//! Split out from the movement logic originally found in `src/c/move.c`.
use glam::IVec2;

use crate::entity::chase::diag_ok;
use crate::entity::player::Thing;
use crate::item::scrolls::ScrollType;
use crate::rnd::rnd;

const SCROLL: u8 = b'?' as u8;

/// rndmove:
/// Pick a random move for a confused actor starting from `pos`.
///
/// Standing still is a valid outcome. Returns the chosen coordinate; the caller
/// owns it, so no pointer is stored in a shared slot.
pub unsafe fn rndmove_from(pos: IVec2) -> IVec2 {
    let mut chosen = IVec2 {
        y: pos.y + rnd(3) - 1,
        x: pos.x + rnd(3) - 1,
    };

    // Standing still is a valid outcome
    if chosen.y == pos.y && chosen.x == pos.x {
        return chosen;
    }

    if diag_ok(pos, chosen) == 0 {
        return pos;
    }

    if !crate::game::cell_is_walkable(chosen.y, chosen.x) {
        return pos;
    }

    // Refuse to step on a scroll of scare monster
    for id in crate::item::arena::OBJECTS.ids() {
        let at_pos = crate::item::arena::OBJECTS
            .with(id, |t| match t {
                Thing::Object { data } => {
                    if data.o_pos.y == chosen.y && data.o_pos.x == chosen.x {
                        Some(data.o_which)
                    } else {
                        None
                    }
                }
                Thing::Monster { .. } => None,
            })
            .flatten();
        if let Some(which) = at_pos {
            if which == ScrollType::Scare as i32 {
                return pos;
            }
            break;
        }
    }

    chosen
}

/// rndmove:
/// Move in a random direction if the monster/person is confused.
pub unsafe fn rndmove(pos: IVec2) -> IVec2 {
    rndmove_from(pos)
}
