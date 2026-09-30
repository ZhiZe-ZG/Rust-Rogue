//! Delayed actions that happen in the future — daemons and fuses.
//!
//! Ported from `src/c/daemon.c` to Rust.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.
//!
//! The legacy engine stored a raw `void (*d_func)(int)` pointer in each slot.
//! This port replaces that pointer with the typed [`Daemon`] tag, which is both
//! the dispatch key and the value persisted by the save file. Firing a slot now
//! passes the stored `d_arg` to the callback, matching the legacy `d_func(d_arg)`
//! calling convention (the previous port called a zero-argument pointer and
//! dropped the argument, which was undefined behaviour for callbacks such as
//! `turn_see`).

use crate::game::globals::D_LIST;
use std::sync::atomic::{AtomicU64, Ordering};

const EMPTY: i32 = 0;
const DAEMON: i32 = -1;
const MAXDAEMONS: usize = 20;
static LAST_RENDERED_LEVEL: AtomicU64 = AtomicU64::new(u64::MAX);

unsafe fn render_current_level() {
    let generation = crate::level::with_current_level(|level| level.generation());
    if LAST_RENDERED_LEVEL.swap(generation, Ordering::Relaxed) != generation {
        crate::ui::terminal::clear();
        let hero = crate::game::PLAYER.pos();
        crate::draw::enter_room(hero);
        if crate::game::PLAYER.has_flag(crate::entity::player::MonsterFlags::SEEMONST) {
            crate::item::potions::turn_see(false as u8);
        }
        crate::draw::write_cell_glyph(hero, '@');
        crate::ui::output::refresh();
    }
    crate::ui::output::status();
    crate::ui::output::render_pending();
}

/// A delayed-action callback, identified by a stable tag.
///
/// Replaces the legacy `void (*d_func)(int)` pointer. The tag doubles as the
/// on-disk identity written by `state.rs` (`state::rs_write_daemons`); the
/// mapping in [`Daemon::save_id`] preserves the exact legacy integers.
#[derive(Copy, Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Daemon {
    Rollwand,
    Doctor,
    Stomach,
    Runners,
    Swander,
    Nohaste,
    Unconfuse,
    Unsee,
    Sight,
    Visuals,
    UiRender,
    TurnSee,
    ComeDown,
    Land,
    RingEffects,
}

impl Daemon {
    /// Execute the callback with the slot's stored `d_arg` value.
    ///
    /// # Safety
    ///
    /// Every callback mutates process-wide game state through the global owners
    /// (`PLAYER`, `MONSTER_LIST`, `CURRENT_LEVEL`, ...); the game is
    /// single-threaded, so callers must not invoke this concurrently.
    pub unsafe fn run(self, arg: i32) {
        match self {
            Daemon::Rollwand => crate::daemons::rollwand(),
            Daemon::Doctor => crate::daemons::doctor(),
            Daemon::Stomach => crate::daemons::stomach(),
            Daemon::Runners => crate::entity::chase::runners(),
            Daemon::Swander => crate::daemons::swander(),
            Daemon::Nohaste => crate::daemons::nohaste(),
            Daemon::Unconfuse => crate::daemons::unconfuse(),
            Daemon::Unsee => crate::daemons::unsee(),
            Daemon::Sight => crate::daemons::sight(),
            Daemon::Visuals => crate::daemons::visuals(),
            Daemon::UiRender => render_current_level(),
            Daemon::TurnSee => {
                let _ = crate::item::potions::turn_see(arg as u8);
            }
            Daemon::ComeDown => crate::daemons::come_down(),
            Daemon::Land => crate::daemons::land(),
            Daemon::RingEffects => crate::daemons::ring_effects(),
        }
    }

    /// The legacy save-file identifier for this callback.
    ///
    /// Only the nine callbacks the original `state.c` recognised map to an id;
    /// the remaining callbacks serialise as `-1`, exactly as before, so the save
    /// bytes are unchanged.
    pub const fn save_id(self) -> Option<i32> {
        match self {
            Daemon::Rollwand => Some(1),
            Daemon::Doctor => Some(2),
            Daemon::Stomach => Some(3),
            Daemon::Runners => Some(4),
            Daemon::Swander => Some(5),
            Daemon::Nohaste => Some(6),
            Daemon::Unconfuse => Some(7),
            Daemon::Unsee => Some(8),
            Daemon::Sight => Some(9),
            Daemon::Visuals
            | Daemon::UiRender
            | Daemon::TurnSee
            | Daemon::ComeDown
            | Daemon::Land
            | Daemon::RingEffects => None,
        }
    }

    /// Rebuild a callback from its legacy save-file identifier.
    pub const fn from_save_id(id: i32) -> Option<Self> {
        match id {
            1 => Some(Daemon::Rollwand),
            2 => Some(Daemon::Doctor),
            3 => Some(Daemon::Stomach),
            4 => Some(Daemon::Runners),
            5 => Some(Daemon::Swander),
            6 => Some(Daemon::Nohaste),
            7 => Some(Daemon::Unconfuse),
            8 => Some(Daemon::Unsee),
            9 => Some(Daemon::Sight),
            _ => None,
        }
    }
}

/// One slot in the delayed-action table.
///
/// This is a Rust-native record: the callback is the typed [`Daemon`] tag rather
/// than a raw function pointer. The field order still mirrors the legacy C
/// `struct delayed_action` (`d_type`, `d_func`, `d_arg`, `d_time`) so the save
/// stream stays readable.
#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CDelayedAction {
    pub d_type: i32,
    pub d_func: Option<Daemon>,
    pub d_arg: i32,
    pub d_time: i32,
}

/// Find an empty slot in the daemon/fuse list.
///
/// # Safety
/// Touches the process-wide `D_LIST`; single-threaded use only.
unsafe fn d_slot() -> Option<usize> {
    (0..MAXDAEMONS).find(|&i| D_LIST[i].d_type == EMPTY)
}

/// Find the index of the slot whose callback matches `func`.
///
/// # Safety
/// Touches the process-wide `D_LIST`; single-threaded use only.
unsafe fn find_slot(func: Daemon) -> Option<usize> {
    (0..MAXDAEMONS).find(|&i| D_LIST[i].d_type != EMPTY && D_LIST[i].d_func == Some(func))
}

/// Start a daemon: inserts `func` as a daemon (`d_time == DAEMON`, i.e. runs
/// every turn).
///
/// # Safety
/// Touches the process-wide `D_LIST`; single-threaded use only.
pub unsafe fn start_daemon(func: Daemon, arg: i32, typ: i32) {
    if let Some(i) = d_slot() {
        D_LIST[i].d_type = typ;
        D_LIST[i].d_func = Some(func);
        D_LIST[i].d_arg = arg;
        D_LIST[i].d_time = DAEMON;
    }
}

/// Remove a daemon/fuse from the table by callback tag.
///
/// # Safety
/// Touches the process-wide `D_LIST`; single-threaded use only.
pub unsafe fn kill_daemon(func: Daemon) {
    if let Some(i) = find_slot(func) {
        D_LIST[i].d_type = EMPTY;
    }
}

/// Run all active daemons whose `d_type` matches `flag`.
///
/// Daemons are entries with `d_time == DAEMON`.
///
/// # Safety
/// Runs game-state callbacks; single-threaded use only.
pub unsafe fn do_daemons(flag: i32) {
    for i in 0..MAXDAEMONS {
        if D_LIST[i].d_type == flag && D_LIST[i].d_time == DAEMON {
            // Capture the callback and its argument before the call, which may
            // modify the table.
            if let Some(f) = D_LIST[i].d_func {
                f.run(D_LIST[i].d_arg);
            }
        }
    }
}

/// Light a fuse: inserts `func` with a countdown of `time` turns.
///
/// # Safety
/// Touches the process-wide `D_LIST`; single-threaded use only.
pub unsafe fn fuse(func: Daemon, arg: i32, time: i32, typ: i32) {
    if let Some(i) = d_slot() {
        D_LIST[i].d_type = typ;
        D_LIST[i].d_func = Some(func);
        D_LIST[i].d_arg = arg;
        D_LIST[i].d_time = time;
    }
}

/// Extend the countdown of an existing fuse by `xtime` turns.
///
/// # Safety
/// Touches the process-wide `D_LIST`; single-threaded use only.
pub unsafe fn lengthen(func: Daemon, xtime: i32) {
    if let Some(i) = find_slot(func) {
        D_LIST[i].d_time += xtime;
    }
}

/// Extinguish (cancel) a fuse or daemon by callback tag.
///
/// # Safety
/// Touches the process-wide `D_LIST`; single-threaded use only.
pub unsafe fn extinguish(func: Daemon) {
    if let Some(i) = find_slot(func) {
        D_LIST[i].d_type = EMPTY;
    }
}

/// Decrement all active fuses whose `d_type` matches `flag`, and fire any that
/// reach zero.
///
/// # Safety
/// Runs game-state callbacks; single-threaded use only.
pub unsafe fn do_fuses(flag: i32) {
    for i in 0..MAXDAEMONS {
        if D_LIST[i].d_type == flag && D_LIST[i].d_time > 0 {
            D_LIST[i].d_time -= 1;
            if D_LIST[i].d_time == 0 {
                D_LIST[i].d_type = EMPTY;
                // Capture the callback and its argument before the call.
                if let Some(f) = D_LIST[i].d_func {
                    f.run(D_LIST[i].d_arg);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Daemon;

    /// The nine callbacks the legacy `state.c` persisted must keep their exact
    /// integer ids, and the mapping must round-trip.
    #[test]
    fn save_ids_match_legacy_state_c() {
        let pairs = [
            (Daemon::Rollwand, 1),
            (Daemon::Doctor, 2),
            (Daemon::Stomach, 3),
            (Daemon::Runners, 4),
            (Daemon::Swander, 5),
            (Daemon::Nohaste, 6),
            (Daemon::Unconfuse, 7),
            (Daemon::Unsee, 8),
            (Daemon::Sight, 9),
        ];
        for (daemon, id) in pairs {
            assert_eq!(daemon.save_id(), Some(id));
            assert_eq!(Daemon::from_save_id(id), Some(daemon));
        }
    }

    /// Callbacks the original engine did not persist must return `None` so the
    /// writer emits the legacy `-1` sentinel.
    #[test]
    fn unknown_callbacks_are_not_serialised() {
        for daemon in [
            Daemon::Visuals,
            Daemon::UiRender,
            Daemon::TurnSee,
            Daemon::ComeDown,
            Daemon::Land,
        ] {
            assert_eq!(daemon.save_id(), None);
        }
        assert_eq!(Daemon::from_save_id(0), None);
        assert_eq!(Daemon::from_save_id(-1), None);
        assert_eq!(Daemon::from_save_id(10), None);
    }

    #[test]
    fn ui_render_daemon_flushes_pending_frame() {
        crate::ui::output::refresh();

        unsafe { Daemon::UiRender.run(0) };

        assert!(!crate::ui::output::render_pending());
    }
}
