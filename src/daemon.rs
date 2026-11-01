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

use crate::game::globals;
use crate::rnd::Random;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

const EMPTY: i32 = 0;
const DAEMON: i32 = -1;
const MAXDAEMONS: usize = 20;
static LAST_RENDERED_LEVEL: AtomicU64 = AtomicU64::new(u64::MAX);

unsafe fn render_current_level(random: &mut Random) {
    let generation = crate::level::with_current_level(|level| level.generation());
    if LAST_RENDERED_LEVEL.swap(generation, Ordering::Relaxed) != generation {
        crate::ui::terminal::UI.clear();
        let hero = crate::dungeon::DUNGEON.player.pos();
        crate::draw::enter_room(hero, random);
        if crate::dungeon::DUNGEON.player.has_flag(crate::entity::player::MonsterFlags::SEEMONST) {
            crate::item::potions::turn_see(false as u8, random, &crate::dungeon::DUNGEON);
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
    /// (`DUNGEON.player`, `DUNGEON.monster_list`, `DUNGEON`, ...); the game is
    /// single-threaded, so callers must not invoke this concurrently.
    pub unsafe fn run(self, arg: i32, random: &mut Random) {
        match self {
            Daemon::Rollwand => crate::daemons::rollwand(random),
            Daemon::Doctor => crate::daemons::doctor(random),
            Daemon::Stomach => crate::daemons::stomach(random),
            Daemon::Runners => crate::entity::chase::runners(
                random,
                &crate::dungeon::DUNGEON,
                &raw mut globals::delta,
                &raw mut globals::running,
                &crate::dungeon::DUNGEON.command_state.count,
                &crate::dungeon::DUNGEON.player.quiet,
                &raw mut globals::to_death,
                &raw mut globals::kamikaze,
                &raw mut globals::has_hit,
                &raw mut globals::max_hit,
                &raw mut globals::no_command,
                &raw mut globals::vf_hit,
                &crate::dungeon::DUNGEON.player.purse,
                &raw const globals::fight_flush,
                &globals::e_levels,
                &raw mut globals::monsters,
                &crate::dungeon::DUNGEON.player.inpack,
                &crate::dungeon::DUNGEON.command_state.last_pick,
                &crate::dungeon::DUNGEON.player.pack_used,
                &raw const globals::inv_describe,
                &raw const globals::arm_info,
                &raw const globals::pot_info,
                &raw const globals::ring_info,
                &raw const globals::scr_info,
                &raw const globals::weap_info,
                &raw const globals::ws_info,
                &raw const globals::see_floor,
                &raw const globals::ROOM_GOLD,
                &crate::dungeon::DUNGEON.player.max_stats,
            ),
            Daemon::Swander => crate::daemons::swander(),
            Daemon::Nohaste => crate::daemons::nohaste(),
            Daemon::Unconfuse => crate::daemons::unconfuse(),
            Daemon::Unsee => crate::daemons::unsee(),
            Daemon::Sight => crate::daemons::sight(random),
            Daemon::Visuals => crate::daemons::visuals(random),
            Daemon::UiRender => render_current_level(random),
            Daemon::TurnSee => {
                let _ = crate::item::potions::turn_see(arg as u8, random, &crate::dungeon::DUNGEON);
            }
            Daemon::ComeDown => crate::daemons::come_down(),
            Daemon::Land => crate::daemons::land(),
            Daemon::RingEffects => crate::daemons::ring_effects(random),
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

const EMPTY_ACTION: CDelayedAction = CDelayedAction {
    d_type: EMPTY,
    d_func: None,
    d_arg: 0,
    d_time: 0,
};

/// Owns the delayed-action table and wandering-monster pacing counter.
///
/// Dispatch reads each slot live and releases its lock before invoking a
/// callback, so callbacks may safely change this scheduler during a pass.
pub struct Scheduler {
    slots: Mutex<[CDelayedAction; MAXDAEMONS]>,
    between: AtomicI32,
}

impl Scheduler {
    /// Build an empty scheduler with a zero pacing counter.
    pub const fn new() -> Self {
        Self {
            slots: Mutex::new([EMPTY_ACTION; MAXDAEMONS]),
            between: AtomicI32::new(0),
        }
    }

    fn slots(&self) -> MutexGuard<'_, [CDelayedAction; MAXDAEMONS]> {
        self.slots.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Copy the table without retaining its lock.
    pub fn snapshot(&self) -> [CDelayedAction; MAXDAEMONS] {
        *self.slots()
    }

    /// Replace the table, clearing unused tails and resetting the counter.
    /// Excess input records are ignored.
    pub fn restore(&self, records: &[CDelayedAction]) {
        let mut slots = self.slots();
        *slots = [EMPTY_ACTION; MAXDAEMONS];
        let len = records.len().min(MAXDAEMONS);
        slots[..len].copy_from_slice(&records[..len]);
        self.set_between(0);
    }

    pub fn between(&self) -> i32 {
        self.between.load(Ordering::Relaxed)
    }

    pub fn set_between(&self, value: i32) {
        self.between.store(value, Ordering::Relaxed);
    }

    /// Increment the counter, returning its previous value (post-increment).
    pub fn advance_between(&self) -> i32 {
        self.between.fetch_add(1, Ordering::Relaxed)
    }

    /// Insert a daemon into the first empty slot; a full table is unchanged.
    pub fn start_daemon(&self, func: Daemon, arg: i32, typ: i32) {
        self.fuse(func, arg, DAEMON, typ);
    }

    /// Remove only the first active record matching the callback.
    pub fn kill_daemon(&self, func: Daemon) {
        let mut slots = self.slots();
        if let Some(slot) = slots.iter_mut().find(|slot| {
            slot.d_type != EMPTY && slot.d_func == Some(func)
        }) {
            slot.d_type = EMPTY;
        }
    }

    /// Insert a fuse into the first empty slot; duplicate tags are allowed.
    pub fn fuse(&self, func: Daemon, arg: i32, time: i32, typ: i32) {
        let mut slots = self.slots();
        if let Some(slot) = slots.iter_mut().find(|slot| slot.d_type == EMPTY) {
            *slot = CDelayedAction {
                d_type: typ,
                d_func: Some(func),
                d_arg: arg,
                d_time: time,
            };
        }
    }

    /// Extend only the first active record matching the callback.
    pub fn lengthen(&self, func: Daemon, xtime: i32) {
        let mut slots = self.slots();
        if let Some(slot) = slots.iter_mut().find(|slot| {
            slot.d_type != EMPTY && slot.d_func == Some(func)
        }) {
            slot.d_time += xtime;
        }
    }

    pub fn extinguish(&self, func: Daemon) {
        self.kill_daemon(func);
    }

    /// Dispatch matching daemons in slot order, inspecting each slot live.
    pub fn run_daemons_with(&self, flag: i32, mut run: impl FnMut(Daemon, i32)) {
        for i in 0..MAXDAEMONS {
            let callback = {
                let slots = self.slots();
                let slot = &slots[i];
                if slot.d_type == flag && slot.d_time == DAEMON {
                    slot.d_func.map(|func| (func, slot.d_arg))
                } else {
                    None
                }
            };
            if let Some((func, arg)) = callback {
                run(func, arg);
            }
        }
    }

    /// Tick matching positive countdowns, expiring them before dispatch.
    pub fn run_fuses_with(&self, flag: i32, mut run: impl FnMut(Daemon, i32)) {
        for i in 0..MAXDAEMONS {
            let callback = {
                let mut slots = self.slots();
                let slot = &mut slots[i];
                if slot.d_type == flag && slot.d_time > 0 {
                    slot.d_time -= 1;
                    if slot.d_time == 0 {
                        slot.d_type = EMPTY;
                        slot.d_func.map(|func| (func, slot.d_arg))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            if let Some((func, arg)) = callback {
                run(func, arg);
            }
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Start a daemon: inserts `func` as a daemon (`d_time == DAEMON`, i.e. runs
/// every turn).
///
/// # Safety
/// Preserves the legacy unsafe API; scheduling itself is synchronized.
pub unsafe fn start_daemon(func: Daemon, arg: i32, typ: i32) {
    crate::dungeon::DUNGEON.scheduler.start_daemon(func, arg, typ);
}

/// Remove a daemon/fuse from the table by callback tag.
///
/// # Safety
/// Preserves the legacy unsafe API; scheduling itself is synchronized.
pub unsafe fn kill_daemon(func: Daemon) {
    crate::dungeon::DUNGEON.scheduler.kill_daemon(func);
}

/// Run all active daemons whose `d_type` matches `flag`.
///
/// Daemons are entries with `d_time == DAEMON`.
///
/// # Safety
/// Runs game-state callbacks; single-threaded use only.
pub unsafe fn do_daemons(flag: i32, random: &mut Random) {
    crate::dungeon::DUNGEON.scheduler.run_daemons_with(flag, |f, arg| {
        f.run(arg, random);
    });
}

/// Light a fuse: inserts `func` with a countdown of `time` turns.
///
/// # Safety
/// Preserves the legacy unsafe API; scheduling itself is synchronized.
pub unsafe fn fuse(func: Daemon, arg: i32, time: i32, typ: i32) {
    crate::dungeon::DUNGEON.scheduler.fuse(func, arg, time, typ);
}

/// Extend the countdown of an existing fuse by `xtime` turns.
///
/// # Safety
/// Preserves the legacy unsafe API; scheduling itself is synchronized.
pub unsafe fn lengthen(func: Daemon, xtime: i32) {
    crate::dungeon::DUNGEON.scheduler.lengthen(func, xtime);
}

/// Extinguish (cancel) a fuse or daemon by callback tag.
///
/// # Safety
/// Preserves the legacy unsafe API; scheduling itself is synchronized.
pub unsafe fn extinguish(func: Daemon) {
    crate::dungeon::DUNGEON.scheduler.extinguish(func);
}

/// Decrement all active fuses whose `d_type` matches `flag`, and fire any that
/// reach zero.
///
/// # Safety
/// Runs game-state callbacks; single-threaded use only.
pub unsafe fn do_fuses(flag: i32, random: &mut Random) {
    crate::dungeon::DUNGEON.scheduler.run_fuses_with(flag, |f, arg| {
        f.run(arg, random);
    });
}

#[cfg(test)]
mod tests {
    use super::{CDelayedAction, Daemon, Scheduler, DAEMON, EMPTY, EMPTY_ACTION, MAXDAEMONS};

    fn assert_slot(actual: CDelayedAction, expected: CDelayedAction) {
        assert_eq!(actual.d_type, expected.d_type);
        assert_eq!(actual.d_func, expected.d_func);
        assert_eq!(actual.d_arg, expected.d_arg);
        assert_eq!(actual.d_time, expected.d_time);
    }

    #[test]
    fn schedulers_are_independent_and_const_constructible() {
        const INITIAL: Scheduler = Scheduler::new();
        let first = INITIAL;
        let second = Scheduler::default();
        for slot in first.snapshot() {
            assert_slot(slot, EMPTY_ACTION);
        }
        first.start_daemon(Daemon::Doctor, 42, 1);
        first.set_between(7);
        assert_eq!(first.advance_between(), 7);
        assert_eq!(first.between(), 8);
        assert_eq!(second.between(), 0);
        for slot in second.snapshot() {
            assert_slot(slot, EMPTY_ACTION);
        }
        second.fuse(Daemon::Sight, 9, 3, 2);
        second.kill_daemon(Daemon::Doctor);
        assert_eq!(first.snapshot()[0].d_func, Some(Daemon::Doctor));
        assert_eq!(second.snapshot()[0].d_func, Some(Daemon::Sight));
    }

    #[test]
    fn capacity_duplicates_and_first_slot_operations() {
        let scheduler = Scheduler::new();
        for arg in 0..MAXDAEMONS as i32 {
            scheduler.fuse(Daemon::Doctor, arg, 5, 1);
        }
        scheduler.start_daemon(Daemon::Sight, 99, 2);
        scheduler.fuse(Daemon::Sight, 99, 1, 2);
        for (i, slot) in scheduler.snapshot().iter().enumerate() {
            assert_eq!(slot.d_arg, i as i32);
            assert_eq!(slot.d_func, Some(Daemon::Doctor));
        }
        scheduler.lengthen(Daemon::Doctor, 2);
        assert_eq!(scheduler.snapshot()[0].d_time, 7);
        assert_eq!(scheduler.snapshot()[1].d_time, 5);
        scheduler.kill_daemon(Daemon::Doctor);
        scheduler.lengthen(Daemon::Doctor, -2);
        assert_eq!(scheduler.snapshot()[1].d_time, 3);
        scheduler.start_daemon(Daemon::Sight, 77, 2);
        assert_slot(scheduler.snapshot()[0], CDelayedAction {
            d_type: 2, d_func: Some(Daemon::Sight), d_arg: 77, d_time: DAEMON,
        });
        scheduler.extinguish(Daemon::Doctor);
        assert_eq!(scheduler.snapshot()[1].d_type, EMPTY);
        assert_eq!(scheduler.snapshot()[2].d_type, 1);
        scheduler.kill_daemon(Daemon::Land);
        scheduler.extinguish(Daemon::Land);
        scheduler.lengthen(Daemon::Land, 5);
        assert_eq!(scheduler.snapshot()[2].d_time, 5);
    }

    #[test]
    fn phase_filters_countdown_lengthening_and_cancellation() {
        let scheduler = Scheduler::new();
        scheduler.start_daemon(Daemon::Doctor, 11, 1);
        scheduler.start_daemon(Daemon::Stomach, 22, 2);
        scheduler.fuse(Daemon::Sight, 33, 2, 1);
        scheduler.fuse(Daemon::Unsee, 44, 1, 2);
        scheduler.fuse(Daemon::Land, 55, 0, 1);
        scheduler.fuse(Daemon::ComeDown, 66, -2, 1);
        let mut calls = Vec::new();
        scheduler.run_daemons_with(1, |f, arg| calls.push((f, arg)));
        scheduler.run_daemons_with(2, |f, arg| calls.push((f, arg)));
        assert_eq!(calls, [(Daemon::Doctor, 11), (Daemon::Stomach, 22)]);
        calls.clear();
        scheduler.run_fuses_with(1, |f, arg| calls.push((f, arg)));
        assert!(calls.is_empty());
        assert_eq!(scheduler.snapshot()[2].d_time, 1);
        assert_eq!(scheduler.snapshot()[3].d_time, 1);
        scheduler.lengthen(Daemon::Sight, 1);
        scheduler.extinguish(Daemon::Unsee);
        scheduler.run_fuses_with(2, |f, arg| calls.push((f, arg)));
        scheduler.run_fuses_with(1, |f, arg| calls.push((f, arg)));
        assert!(calls.is_empty());
        scheduler.run_fuses_with(1, |f, arg| calls.push((f, arg)));
        assert_eq!(calls, [(Daemon::Sight, 33)]);
        scheduler.run_fuses_with(1, |_, _| panic!("expired fuse fired twice"));
        let slots = scheduler.snapshot();
        assert_eq!(slots[0].d_time, DAEMON);
        assert_eq!(slots[4].d_time, 0);
        assert_eq!(slots[5].d_time, -2);
    }

    #[test]
    fn daemon_dispatch_observes_reentrant_later_slot_changes() {
        let scheduler = Scheduler::new();
        scheduler.start_daemon(Daemon::Doctor, 1, 1);
        scheduler.start_daemon(Daemon::Stomach, 2, 1);
        let mut calls = Vec::new();
        scheduler.run_daemons_with(1, |func, arg| {
            calls.push((func, arg));
            if func == Daemon::Doctor {
                scheduler.kill_daemon(func);
                scheduler.start_daemon(Daemon::Land, 3, 1); // Earlier slot: next pass.
                scheduler.kill_daemon(Daemon::Stomach);
                scheduler.start_daemon(Daemon::Sight, 4, 1); // Later slot: this pass.
                scheduler.start_daemon(Daemon::Unsee, 5, 1);
                assert_eq!(scheduler.snapshot()[0].d_func, Some(Daemon::Land));
            }
        });
        assert_eq!(calls, [(Daemon::Doctor, 1), (Daemon::Sight, 4), (Daemon::Unsee, 5)]);
        calls.clear();
        scheduler.run_daemons_with(1, |f, arg| calls.push((f, arg)));
        assert_eq!(calls, [(Daemon::Land, 3), (Daemon::Sight, 4), (Daemon::Unsee, 5)]);
    }

    #[test]
    fn fuses_expire_before_reentrant_dispatch_and_observe_live_changes() {
        let scheduler = Scheduler::new();
        scheduler.fuse(Daemon::Sight, 1, 1, 1);
        scheduler.fuse(Daemon::Unsee, 2, 1, 1);
        scheduler.fuse(Daemon::Land, 3, 1, 1);
        let mut calls = Vec::new();
        scheduler.run_fuses_with(1, |func, arg| {
            calls.push((func, arg));
            if func == Daemon::Sight {
                assert_eq!(scheduler.snapshot()[0].d_type, EMPTY);
                scheduler.fuse(func, 4, 1, 1); // Reuses expired slot, not reticked.
                scheduler.lengthen(Daemon::Unsee, 1);
                scheduler.extinguish(Daemon::Land);
                scheduler.fuse(Daemon::ComeDown, 5, 1, 1);
            }
        });
        assert_eq!(calls, [(Daemon::Sight, 1), (Daemon::ComeDown, 5)]);
        assert_eq!(scheduler.snapshot()[0].d_time, 1);
        assert_eq!(scheduler.snapshot()[1].d_time, 1);
        calls.clear();
        scheduler.run_fuses_with(1, |f, arg| calls.push((f, arg)));
        assert_eq!(calls, [(Daemon::Sight, 4), (Daemon::Unsee, 2)]);
    }

    #[test]
    fn restore_clears_tails_resets_counter_and_truncates() {
        let scheduler = Scheduler::new();
        let record = CDelayedAction {
            d_type: 2, d_func: Some(Daemon::Doctor), d_arg: 42, d_time: 8,
        };
        scheduler.restore(&[record; MAXDAEMONS + 3]);
        for slot in scheduler.snapshot() {
            assert_slot(slot, record);
        }
        scheduler.set_between(9);
        scheduler.restore(&[record]);
        assert_eq!(scheduler.between(), 0);
        assert_slot(scheduler.snapshot()[0], record);
        for slot in &scheduler.snapshot()[1..] {
            assert_slot(*slot, EMPTY_ACTION);
        }
        let mut snapshot = scheduler.snapshot();
        snapshot[0] = EMPTY_ACTION;
        assert_slot(snapshot[0], EMPTY_ACTION);
        assert_slot(scheduler.snapshot()[0], record);
        scheduler.set_between(3);
        scheduler.restore(&[]);
        assert_eq!(scheduler.between(), 0);
        for slot in scheduler.snapshot() {
            assert_slot(slot, EMPTY_ACTION);
        }
    }

    #[test]
    fn poisoned_mutex_is_recovered() {
        let scheduler = Scheduler::new();
        let result = std::panic::catch_unwind(|| {
            let _guard = scheduler.slots.lock().unwrap();
            panic!("poison the scheduler lock");
        });
        assert!(result.is_err());
        scheduler.start_daemon(Daemon::Doctor, 7, 1);
        scheduler.run_daemons_with(1, |f, arg| {
            assert_eq!((f, arg), (Daemon::Doctor, 7));
            scheduler.kill_daemon(f);
        });
        scheduler.fuse(Daemon::Sight, 8, 1, 1);
        scheduler.lengthen(Daemon::Sight, 1);
        scheduler.run_fuses_with(1, |_, _| panic!("fuse fired early"));
        scheduler.run_fuses_with(1, |f, arg| assert_eq!((f, arg), (Daemon::Sight, 8)));
        scheduler.restore(&[]);
        for slot in scheduler.snapshot() {
            assert_slot(slot, EMPTY_ACTION);
        }
    }

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

        unsafe { Daemon::UiRender.run(0, &mut crate::rnd::RNG.lock().unwrap()) };

        assert!(!crate::ui::output::render_pending());
    }
}
