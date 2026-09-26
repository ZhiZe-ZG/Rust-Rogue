//! All the daemon and fuse callback functions.
//!
//! Ported from `src/c/daemons.c` to Rust.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use crate::rnd::rnd;
use crate::ui::output;
use glam::IVec2;

use crate::ui::output::{addmsg_str, msg_str};

use crate::daemon::{extinguish, fuse, kill_daemon, start_daemon, Daemon};
use crate::draw::enter_room;
use crate::entity::chase::{cansee, see_monst};
use crate::entity::monsters::{wanderer, MonsterType};
use crate::entity::player::{MonsterFlags, Thing, ThingMonster, ThingObject};
use crate::game::MONSTER_LIST;
use crate::game::PLAYER;
use crate::item::rings::{ring_eat, RingType};
use crate::misc::{choose_str, rnd_thing, spread};
use crate::rip::death;
use crate::startup::roll;

// ─── Constants ───────────────────────────────────────────────────────────────

// d_type flags (BEFORE/AFTER)
const BEFORE: i32 = 1; // spread(1) == 1 always
const AFTER: i32 = 2; // spread(2) == 2 always

const LEFT: usize = 0;
const RIGHT: usize = 1;

// Food constants
const MORETIME: i32 = 150;
const STARVETIME: i32 = 850;

// ─── Extern C globals ────────────────────────────────────────────────────────

use crate::globals::{after, amulet, count, food_left, hungry_state, jump, no_command, quiet, running, seenstairs, terse, to_death};


// ─── Module-local helpers ─────────────────────────────────────────────────────

#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

/// ISRING(hand, ring_type): true when the player wears ring_type on hand.
#[inline]
unsafe fn isring(ring: *mut Thing, ring_type: RingType) -> bool {
    !ring.is_null() && RingType::from_raw((*thing_o(ring)).o_which) == Some(ring_type)
}

// ─── Module globals ───────────────────────────────────────────────────────────

/// Counter used by rollwand() to pace wandering-monster checks.
/// Originally defined in daemons.c as `int between = 0;`.
pub static mut between: i32 = 0;

// ─── Daemon / fuse callbacks ──────────────────────────────────────────────────

/// doctor:
/// A healing daemon that restores hit points after rest.
pub unsafe fn doctor() {
    let lv = PLAYER.level();
    let ohp = PLAYER.stats().hit_points;
    quiet += 1;
    if lv < 8 {
        if quiet + (lv << 1) > 20 {
            PLAYER.with_stats_mut(|stats| stats.hit_points += 1);
        }
    } else if quiet >= 3 {
        PLAYER.with_stats_mut(|stats| stats.hit_points += rnd(lv - 7) + 1);
    }
    if isring(PLAYER.left_ring(), RingType::Regeneration) {
        PLAYER.with_stats_mut(|stats| stats.hit_points += 1);
    }
    if isring(PLAYER.right_ring(), RingType::Regeneration) {
        PLAYER.with_stats_mut(|stats| stats.hit_points += 1);
    }
    if ohp != PLAYER.stats().hit_points {
        let max = PLAYER.stats().max_hit_points;
        if PLAYER.stats().hit_points > max {
            PLAYER.with_stats_mut(|stats| stats.hit_points = max);
        }
        quiet = 0;
    }
}

/// swander:
/// Called when it is time to start rolling for wandering monsters.
pub unsafe fn swander() {
    start_daemon(Daemon::Rollwand, 0, BEFORE);
}

/// rollwand:
/// Called to roll to see if a wandering monster starts up.
pub unsafe fn rollwand() {
    between += 1;
    if between >= 4 {
        if roll(1, 6) == 4 {
            wanderer();
            kill_daemon(Daemon::Rollwand);
            fuse(Daemon::Swander, 0, spread(70), BEFORE);
        }
        between = 0;
    }
}

/// unconfuse:
/// Release the poor player from his confusion.
pub unsafe fn unconfuse() {
    PLAYER.remove_flag(MonsterFlags::HUH);
    msg_str(&format!(
        "you feel less {} now",
        choose_str("trippy", "confused")
    ));
}

/// unsee:
/// Turn off the ability to see invisible.
pub unsafe fn unsee() {
    for id in MONSTER_LIST.ids() {
        if let Some(th) = MONSTER_LIST.handle(id) {
            if (*thing_t(th)).t_flags.contains(MonsterFlags::INVIS) && see_monst(th) != 0 {
                output::write_glyph_at(
                    IVec2::new((*thing_t(th)).t_pos.x, (*thing_t(th)).t_pos.y),
                    ((*thing_t(th)).t_oldch as u8) as char,
                );
            }
        }
    }
    PLAYER.remove_flag(MonsterFlags::CANSEE);
}

/// sight:
/// He gets his sight back.
pub unsafe fn sight() {
    if PLAYER.has_flag(MonsterFlags::BLIND) {
        extinguish(Daemon::Sight);
        PLAYER.remove_flag(MonsterFlags::BLIND);
        let proom = PLAYER.room();
        if !crate::game::room_gone(proom) {
            let mut pos = PLAYER.pos();
            enter_room(&mut pos);
        }
        msg_str(choose_str(
            "far out!  Everything is all cosmic again",
            "the veil of darkness lifts",
        ));
    }
}

/// nohaste:
/// End the hasting.
pub unsafe fn nohaste() {
    PLAYER.remove_flag(MonsterFlags::HASTE);
    msg_str("you feel yourself slowing down");
}

/// stomach:
/// Digest the hero's food.
pub unsafe fn stomach() {
    let orig_hungry = hungry_state;

    if food_left <= 0 {
        // Post-decrement comparison: check old value, then decrement.
        let old_food = food_left;
        food_left -= 1;
        if old_food < -STARVETIME {
            death(b's' as u8);
        }
        // The hero is fainting.
        if no_command != 0 || rnd(5) != 0 {
            return;
        }
        no_command += rnd(8) + 4;
        hungry_state = 3;
        if terse == 0 {
            addmsg_str(choose_str(
                "the munchies overpower your motor capabilities.  ",
                "you feel too weak from lack of food.  ",
            ));
        }
        msg_str(choose_str("You freak out", "You faint"));
    } else {
        let oldfood = food_left;
        food_left -= ring_eat(LEFT as i32) + ring_eat(RIGHT as i32) + 1 - amulet as i32;

        if food_left < MORETIME && oldfood >= MORETIME {
            hungry_state = 2;
            msg_str(choose_str(
                "the munchies are interfering with your motor capabilites",
                "you are starting to feel weak",
            ));
        } else if food_left < 2 * MORETIME && oldfood >= 2 * MORETIME {
            hungry_state = 1;
            if terse != 0 {
                msg_str(choose_str("getting the munchies", "getting hungry"));
            } else {
                msg_str(choose_str(
                    "you are getting the munchies",
                    "you are starting to get hungry",
                ));
            }
        }
    }

    if hungry_state != orig_hungry {
        PLAYER.remove_flag(MonsterFlags::RUN);
        running = false as u8;
        to_death = false as u8;
        count = 0;
    }
}

/// come_down:
/// Take the hero down off her acid trip.
pub unsafe fn come_down() {
    if !PLAYER.has_flag(MonsterFlags::HALU) {
        return;
    }

    kill_daemon(Daemon::Visuals);
    PLAYER.remove_flag(MonsterFlags::HALU);

    if PLAYER.has_flag(MonsterFlags::BLIND) {
        return;
    }

    // Undo the things (objects on the level).
    let mut tp = crate::game::with_current_level(|level| level.items.head());
    while !tp.is_null() {
        let op = thing_o(tp);
        if cansee((*op).o_pos.y, (*op).o_pos.x) != 0 {
            output::write_glyph_at(
                IVec2::new((*op).o_pos.x, (*op).o_pos.y),
                ((*op).o_type as u8) as char,
            );
        }
        tp = crate::entity::player::thing_next(tp);
    }

    // Undo the monsters.
    let seemonst = PLAYER.has_flag(MonsterFlags::SEEMONST);
    let cansee_invis = PLAYER.has_flag(MonsterFlags::CANSEE);
    for id in MONSTER_LIST.ids() {
        if let Some(tp) = MONSTER_LIST.handle(id) {
            output::move_cursor(IVec2::new((*thing_t(tp)).t_pos.x, (*thing_t(tp)).t_pos.y));
            if cansee((*thing_t(tp)).t_pos.y, (*thing_t(tp)).t_pos.x) != 0 {
                if !(*thing_t(tp)).t_flags.contains(MonsterFlags::INVIS) || cansee_invis {
                    output::write_glyph(crate::draw::monster_glyph(tp));
                }
                // If invisible and player can't see invisible, skip (original code
                // falls through to the else-if, but cansee returned true here,
                // so seemonst branch is not reached — matching C behavior).
            } else if seemonst {
                output::set_standout(true);
                output::write_glyph(crate::draw::monster_type_glyph(tp));
                output::set_standout(false);
            }
        }
    }
    msg_str("Everything looks SO boring now.");
}

/// visuals:
/// Change the displayed characters for the hallucinating player.
pub unsafe fn visuals() {
    if after == 0 || (running != 0 && jump != 0) {
        return;
    }

    // Change the things (objects).
    let mut tp = crate::game::with_current_level(|level| level.items.head());
    while !tp.is_null() {
        let op = thing_o(tp);
        if cansee((*op).o_pos.y, (*op).o_pos.x) != 0 {
            output::write_glyph_at(
                IVec2::new((*op).o_pos.x, (*op).o_pos.y),
                (rnd_thing() as u8) as char,
            );
        }
        tp = crate::entity::player::thing_next(tp);
    }

    // Change the stairs.
    let stairs = crate::game::stairs();
    if seenstairs == 0 && cansee(stairs.y, stairs.x) != 0 {
        output::write_glyph_at(IVec2::new(stairs.x, stairs.y), (rnd_thing() as u8) as char);
    }

    // Change the monsters.
    let seemonst = PLAYER.has_flag(MonsterFlags::SEEMONST);
    for id in MONSTER_LIST.ids() {
        if let Some(tp) = MONSTER_LIST.handle(id) {
            output::move_cursor(IVec2::new((*thing_t(tp)).t_pos.x, (*thing_t(tp)).t_pos.y));
            if see_monst(tp) != 0 {
                if (*thing_t(tp)).t_type == Some(MonsterType::Xeroc) && (*thing_t(tp)).t_disguise != b'X' {
                    output::write_glyph((rnd_thing() as u8) as char);
                } else {
                    output::write_glyph(crate::draw::hallucination_glyph());
                }
            } else if seemonst {
                output::set_standout(true);
                output::write_glyph(crate::draw::hallucination_glyph());
                output::set_standout(false);
            }
        }
    }
}

/// land:
/// Land from a levitation potion.
pub unsafe fn land() {
    PLAYER.remove_flag(MonsterFlags::LEVIT);
    msg_str(choose_str(
        "bummer!  You've hit the ground",
        "you float gently to the ground",
    ));
}
