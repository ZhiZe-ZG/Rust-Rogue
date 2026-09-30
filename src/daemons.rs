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
use glam::IVec2;

use crate::ui::output::{addmsg_str, msg_str};

use crate::daemon::{extinguish, fuse, kill_daemon, start_daemon, Daemon};
use crate::draw::enter_room;
use crate::entity::chase::{cansee, see_monst};
use crate::entity::monsters::{wanderer, MonsterType};
use crate::entity::player::{MonsterFlags, Thing};
use crate::dungeon::DUNGEON;
use crate::game::PLAYER;
use crate::item::arena::{ThingId, OBJECTS};
use crate::item::rings::{ring_eat, RingType};
use crate::misc::{choose_str, rnd_thing, spread};
use crate::rip::death;
use crate::rnd::roll;

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

use crate::game::globals::{
    after, amulet, count, food_left, hungry_state, jump, no_command, quiet, running, seenstairs,
    terse, to_death,
};

// ─── Module-local helpers ─────────────────────────────────────────────────────

/// The `(position, disguise, oldch, flags)` of a monster `id`, if it exists.
#[inline]
fn monster_view(id: crate::game::MonsterId) -> Option<(IVec2, u8, u8, MonsterFlags)> {
    DUNGEON.monster_list
        .with(id, |t| match t {
            Thing::Monster { data } => {
                Some((data.t_pos, data.t_disguise, data.t_oldch, data.t_flags))
            }
            Thing::Object { .. } => None,
        })
        .flatten()
}

// ─── Module globals ───────────────────────────────────────────────────────────

use crate::game::globals::between;

// ─── Daemon / fuse callbacks ──────────────────────────────────────────────────

/// Apply the effects of searching and teleportation rings.
pub unsafe fn ring_effects() {
    let equipment = PLAYER.equipment();
    for hand in 0..2usize {
        match equipment.ring_type(hand) {
            Some(RingType::Searching) => crate::command_dispatch::search(),
            Some(RingType::Teleport) => {
                if rnd(50) == 0 {
                    crate::wizard::teleport();
                }
            }
            _ => {}
        }
    }
}

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
    // A ring of regeneration adds an extra point on each hand it is worn.
    let regenerations = [0usize, 1]
        .into_iter()
        .filter(|&hand| PLAYER.equipment().ring_type(hand) == Some(RingType::Regeneration))
        .count() as i32;
    if regenerations != 0 {
        PLAYER.with_stats_mut(|stats| stats.hit_points += regenerations);
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
    for id in DUNGEON.monster_list.ids() {
        if let Some((pos, _disguise, oldch, flags)) = monster_view(id) {
            if flags.contains(MonsterFlags::INVIS) && see_monst(id) != 0 {
                crate::draw::write_cell_glyph(IVec2::new(pos.x, pos.y), oldch as char);
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
            let pos = PLAYER.pos();
            enter_room(pos);
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
            if crate::startup::exit_requested() {
                return;
            }
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
    for id in crate::game::item_ids() {
        if let Some((pos, otype)) = OBJECTS.with_object(id, |o| (o.o_pos, o.o_type)) {
            if cansee(pos.y, pos.x) != 0 {
                crate::draw::write_cell_glyph(
                    IVec2::new(pos.x, pos.y),
                    crate::draw::item_glyph(otype),
                );
            }
        }
    }

    // Undo the monsters.
    let seemonst = PLAYER.has_flag(MonsterFlags::SEEMONST);
    let cansee_invis = PLAYER.has_flag(MonsterFlags::CANSEE);
    for id in DUNGEON.monster_list.ids() {
        if let Some((pos, _disguise, _oldch, flags)) = monster_view(id) {
            if cansee(pos.y, pos.x) != 0 {
                if !flags.contains(MonsterFlags::INVIS) || cansee_invis {
                    crate::draw::write_cell_glyph(pos, crate::draw::monster_glyph(id));
                }
            } else if seemonst {
                crate::draw::write_reverse_video_cell_glyph(pos, crate::draw::monster_type_glyph(id));
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
    for id in crate::game::item_ids() {
        if let Some(pos) = OBJECTS.with_object(id, |o| o.o_pos) {
            if cansee(pos.y, pos.x) != 0 {
                crate::draw::write_cell_glyph(IVec2::new(pos.x, pos.y), rnd_thing() as char);
            }
        }
    }

    // Change the stairs.
    let stairs = crate::game::stairs();
    if seenstairs == 0 && cansee(stairs.y, stairs.x) != 0 {
        crate::draw::write_cell_glyph(stairs, rnd_thing() as char);
    }

    // Change the monsters.
    let seemonst = PLAYER.has_flag(MonsterFlags::SEEMONST);
    for id in DUNGEON.monster_list.ids() {
        if let Some((pos, disguise, _oldch, _flags)) = monster_view(id) {
            let typ = DUNGEON.monster_list
                .with(id, |t| match t {
                    Thing::Monster { data } => data.t_type,
                    Thing::Object { .. } => None,
                })
                .flatten();
            if see_monst(id) != 0 {
                if typ == Some(MonsterType::Xeroc) && disguise != b'X' {
                    crate::draw::write_cell_glyph(pos, rnd_thing() as char);
                } else {
                    crate::draw::write_cell_glyph(pos, crate::draw::hallucination_glyph());
                }
            } else if seemonst {
                crate::draw::write_reverse_video_cell_glyph(pos, crate::draw::hallucination_glyph());
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
