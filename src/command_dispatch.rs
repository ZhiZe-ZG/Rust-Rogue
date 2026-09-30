//! Runtime command dispatch.
//!
//! Executes player commands and owns the gameplay helpers used by the main loop.

use crate::command::Command;
use crate::config::GameConfig;
use crate::daemon::{do_daemons, do_fuses};
use crate::direction::Direction;
use crate::draw::{add_pass, look};
use crate::entity::chase::{diag_ok, see_monst};
use crate::entity::player::{do_move, do_run, MonsterFlags, ObjectFlags, Thing};
use crate::game::globals::{pot_info, ring_info, scr_info, ws_info};
use crate::game::PLAYER;
use crate::help::{help, identify};
use crate::item::arena::{new_item_id, ThingId, OBJECTS};
use crate::item::armor::{take_off, wear};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::{add_pack_id, get_item_id, inventory, pick_up, picky_inven};
use crate::item::potions::{quaff, raise_level, turn_see};
use crate::item::rings::{ring_off, ring_on};
use crate::item::scrolls::read_scroll;
use crate::item::sticks::do_zap;
use crate::item::things::{drop, inv_name_id};
use crate::item::weapons::{init_weapon, missile, wield};
use crate::dungeon::new_level;
use crate::misc::{eat, get_dir};
use crate::options::{option, read_line};
use crate::rip::total_winner;
use crate::rnd::rnd;
use crate::save::save_game;
use crate::startup::{quit, shell};
use crate::ui::output::{self, addmsg_str, endmsg, msg_str, status};
use crate::wizard::{create_obj, show_map, teleport, whatis};
use glam::IVec2;
// ─── Constants ────────────────────────────────────────────────────────────────

const F_REAL: u8 = 0x10u8 as u8;

// Weapon/armor kinds for the wizard ('^I' = CTRL-I) cheat
const TWOSWORD: i32 = 5;
const PLATE_MAIL: i32 = 7;

// Delayed-action phases
const BEFORE: i32 = 1;
const AFTER: i32 = 2;

/// Wizard-mode: the preprocessor conditional in the C code is replaced by a
/// runtime check on the `wizard` global.  All wizard helpers are always
/// compiled in, matching the style used by wizard.rs, chase.rs and friends.
const MASTER: bool = true;

// ─── Command-local persistent state ──────────────────────────────────────────

pub(crate) struct CommandState {
    repeat_command: Command,
    run_command: Command,
    new_count: bool,
}

impl Default for CommandState {
    fn default() -> Self {
        Self {
            repeat_command: Command::UnknownKey,
            run_command: Command::UnknownKey,
            new_count: false,
        }
    }
}

// ─── Extern C globals ─────────────────────────────────────────────────────────

use crate::game::globals::{
    after, again, amulet, count, delta, dir_ch, door_stop, firstmove, get_food_left,
    get_inpack, has_hit, inv_describe, jump, kamikaze, l_last_comm, l_last_dir, l_last_pick,
    last_comm, last_dir, last_pick, lastscore, max_hit, move_on, mpos, no_command, noscore,
    p_colors, purse, q_comm, r_stones, runch, running, save_msg, seenstairs, stat_msg, take, terse,
    to_death, wizard, ws_made,
};

// ─── Extern C functions called from this module ───────────────────────────────

// ─── Module-local helpers ─────────────────────────────────────────────────────

#[inline]
fn hero_pos() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    crate::game::PLAYER.has_flag(flag)
}

/// Copy a string slice into an owned [`String`].
#[inline]
fn cstr_at(s: &str) -> String {
    s.to_owned()
}

// ─── command() ────────────────────────────────────────────────────────────────

/// command:
/// Process the user commands.
///
/// Uses globals: player, has_hit, running, door_stop, lastscore,
/// purse, hero, jump, take, after, wizard, noscore, no_command,
/// count, move_on, mpos, runch, to_death, countch, l_last_comm /
/// last_comm / last_dir / last_pick (via reset_last/last_*), lvl_obj,
/// terse, mlist (via moat), max_hit, mp/t_flags (via to_death),
/// dir_ch, delta, q_comm, huh, release, amulet, level, seenstairs,
/// tr_name, stat_msg, inpack, food_left, equipment, inv_describe.
pub(crate) unsafe fn do_command(command_state: &mut CommandState) {
    let mut ntimes = initial_move_budget(player_has(MonsterFlags::HASTE));

    /*
     * Let the daemons start up
     */
    do_daemons(BEFORE);
    do_fuses(BEFORE);
    if crate::startup::exit_requested() {
        return;
    }

    while ntimes > 0 {
        ntimes -= 1;
        again = false as u8;
        // Level transitions can retry this command before the AFTER daemon
        // phase, so let the UI repaint a changed level before look/input.
        crate::daemon::Daemon::UiRender.run(0);
        if has_hit != 0 {
            look(false as u8);
            endmsg();
            has_hit = false as u8;
        }

        look(true as u8);
        if running == 0 {
            door_stop = false as u8;
        }
        crate::daemon::Daemon::UiRender.run(0);
        lastscore = purse;
        let hero = hero_pos();
        crate::ui::terminal::UI.move_cursor(IVec2::new(hero.x, hero.y));
        if !((running != 0 || count != 0) && jump != 0) {
            output::refresh(); // Draw screen
        }
        take = 0;
        after = true as u8;

        /*
         * Read command or continue run
         */
        if MASTER && wizard != 0 {
            noscore = 1;
        }

        let (mut command, can_dispatch) = read_command(command_state);
        if crate::startup::exit_requested() {
            return;
        }
        if can_dispatch {
            /*
             * execute a command
             */
            if count != 0 && running == 0 {
                count -= 1;
            }
            if !matches!(command, Command::Again | Command::Escape)
                && running == 0
                && count == 0
                && to_death == 0
            {
                l_last_comm = last_comm;
                l_last_dir = last_dir;
                l_last_pick = last_pick;
                last_comm = command;
                last_dir = Direction::None;
                last_pick = None;
            }

            // ── Command dispatch ────────────────────────────────────────────
            // The C code uses `goto over` from a few arms; we emulate it with
            // a labelled loop: arms that re-dispatch set `ch` and `continue`.
            'dispatch: loop {
                match command {
                    Command::Pickup => {
                        let hero = hero_pos();
                        let mut found_obj: Option<ThingId> = None;
                        for obj in crate::game::item_ids() {
                            if crate::item::arena::with_object(obj, |o| {
                                o.o_pos.y == hero.y && o.o_pos.x == hero.x
                            })
                            .unwrap_or(false)
                            {
                                found_obj = Some(obj);
                                break;
                            }
                        }

                        if let Some(obj) = found_obj {
                            if levit_check() == 0 {
                                let code =
                                    crate::item::arena::with_object(obj, |o| o.o_type.code())
                                        .unwrap_or(0);
                                pick_up(code as u8);
                            }
                        } else {
                            if terse == 0 {
                                addmsg_str("there is ");
                            }
                            addmsg_str("nothing here");
                            if terse == 0 {
                                addmsg_str(" to pick up");
                            }
                            endmsg();
                        }
                    }
                    Command::Shell => shell(),
                    Command::Move(direction) => {
                        let movement_delta = direction.delta();
                        do_move(movement_delta.y, movement_delta.x);
                    }
                    Command::Run(direction) => do_run(direction),
                    Command::RunPrefix(direction) => {
                        if !player_has(MonsterFlags::BLIND) {
                            door_stop = true as u8;
                            firstmove = true as u8;
                        }
                        command = if count != 0 && !command_state.new_count {
                            command_state.run_command
                        } else {
                            let run = Command::Run(direction);
                            command_state.run_command = run;
                            run
                        };
                        continue 'dispatch;
                    }
                    Command::Fire | Command::FireKamikaze => {
                        if command == Command::FireKamikaze {
                            kamikaze = true as u8;
                        }
                        if get_dir() == 0 {
                            after = false as u8;
                        } else {
                            let hero = hero_pos();
                            delta.y += hero.y;
                            delta.x += hero.x;
                            let mp = crate::game::monster_id_at(delta.y, delta.x);
                            let no_monster = match mp {
                                None => true,
                                Some(id) => {
                                    see_monst(id) == 0 && !player_has(MonsterFlags::SEEMONST)
                                }
                            };
                            if no_monster {
                                if terse == 0 {
                                    addmsg_str("I see ");
                                }
                                msg_str("no monster there");
                                after = false as u8;
                            } else if diag_ok(hero_pos(), delta) != 0 {
                                to_death = true as u8;
                                max_hit = 0;
                                if let Some(id) = mp {
                                    crate::game::DUNGEON.monster_list.with_mut(id, |t| {
                                        if let Thing::Monster { data } = t {
                                            data.t_flags.insert(MonsterFlags::TARGET);
                                        }
                                    });
                                }
                                runch = dir_ch;
                                command = Command::Move(dir_ch);
                                continue 'dispatch;
                            }
                        }
                    }
                    Command::Throw => {
                        if get_dir() == 0 {
                            after = false as u8;
                        } else {
                            missile(delta.y, delta.x);
                        }
                    }
                    Command::Again => {
                        if last_comm == Command::UnknownKey {
                            msg_str("you haven't typed a command yet");
                            after = false as u8;
                        } else {
                            command = last_comm;
                            again = true as u8;
                            continue 'dispatch;
                        }
                    }
                    Command::Quaff => quaff(),
                    Command::Quit => {
                        after = false as u8;
                        q_comm = true as u8;
                        quit();
                        q_comm = false as u8;
                    }
                    Command::Inventory => {
                        after = false as u8;
                        inventory(&crate::item::pack::pack_ptrs(), ItemFilter::Any);
                    }
                    Command::InventorySelect => {
                        after = false as u8;
                        picky_inven();
                    }
                    Command::Drop => drop(),
                    Command::ReadScroll => read_scroll(),
                    Command::Eat => eat(),
                    Command::Wield => wield(),
                    Command::Wear => wear(),
                    Command::TakeOff => take_off(),
                    Command::RingOn => ring_on(),
                    Command::RingOff => ring_off(),
                    Command::Options => {
                        option();
                        after = false as u8;
                    }
                    Command::Call => {
                        call();
                        after = false as u8;
                    }
                    Command::Descend => {
                        after = false as u8;
                        d_level();
                    }
                    Command::Ascend => {
                        after = false as u8;
                        u_level();
                    }
                    Command::Help => {
                        after = false as u8;
                        help();
                    }
                    Command::Identify => {
                        after = false as u8;
                        identify();
                    }
                    Command::Search => search(),
                    Command::Zap => {
                        if get_dir() != 0 {
                            do_zap();
                        } else {
                            after = false as u8;
                        }
                    }
                    Command::Discover => after = false as u8,
                    Command::MessageHistory => {
                        after = false as u8;
                        msg_str(&crate::game::globals::huh_string());
                    }
                    Command::Refresh => {
                        after = false as u8;
                        output::refresh();
                    }
                    Command::Version => {
                        after = false as u8;
                        msg_str(&format!(
                            "version {}. (mctesq was here)",
                            crate::vers::release()
                        ));
                    }
                    Command::Save => {
                        after = false as u8;
                        save_game();
                    }
                    Command::Rest => {
                        // Rest command
                    }
                    Command::Space => {
                        after = false as u8; // "Legal" illegal command
                    }
                    Command::FindTrap => {
                        after = false as u8;
                        if get_dir() != 0 {
                            let hero = hero_pos();
                            delta.y += hero.y;
                            delta.x += hero.x;
                            if terse == 0 {
                                addmsg_str("You have found ");
                            }
                            if !crate::draw::is_trap_cell(delta.y, delta.x) {
                                msg_str("no trap there");
                            } else if player_has(MonsterFlags::HALU) {
                                let name = crate::game::globals::trap_name(rnd(
                                    GameConfig::TRAP_KIND_COUNT,
                                )
                                    as usize);
                                msg_str(&name);
                            } else {
                                let name = crate::game::globals::trap_name(
                                    crate::draw::trap_kind_at(delta.y, delta.x) as usize,
                                );
                                msg_str(&name);
                                crate::draw::set_seen_at(delta.y, delta.x);
                            }
                        }
                    }
                    Command::WizardToggle => {
                        // Wizard toggle (was the `when '+'` arm under `#ifdef MASTER`)
                        after = false as u8;
                        if MASTER {
                            if wizard != 0 {
                                wizard = 0;
                                turn_see(true as u8);
                                msg_str("not wizard any more");
                            } else {
                                wizard = 1;
                                noscore = 1;
                                turn_see(false as u8);
                                msg_str(&format!(
                                    "you are suddenly as smart as Ken Arnold in dungeon #{}",
                                    1234
                                    // should show seed here
                                    // get_dnum()
                                ));
                            }
                        }
                    }
                    Command::Escape => {
                        door_stop = false as u8;
                        count = 0;
                        after = false as u8;
                        again = false as u8;
                    }
                    Command::MoveOn => {
                        move_on = true as u8;
                        if get_dir() == 0 {
                            after = false as u8;
                        } else {
                            command = Command::Move(dir_ch);
                            command_state.repeat_command = Command::Move(dir_ch);
                            continue 'dispatch;
                        }
                    }
                    Command::CurrentWeapon => {
                        current(PLAYER.equipment().weapon_id(), "wielding", "");
                    }
                    Command::CurrentArmor => {
                        current(PLAYER.equipment().armor_id(), "wearing", "");
                    }
                    Command::CurrentRings => {
                        let eq = PLAYER.equipment();
                        current(
                            eq.left_ring_id(),
                            "wearing",
                            if terse != 0 { "(L)" } else { "on left hand" },
                        );
                        current(
                            eq.right_ring_id(),
                            "wearing",
                            if terse != 0 { "(R)" } else { "on right hand" },
                        );
                    }
                    Command::Status => {
                        stat_msg = true as u8;
                        status();
                        stat_msg = false as u8;
                        after = false as u8;
                    }
                    _ => {
                        after = false as u8;
                        if MASTER && wizard != 0 {
                            match command {
                                Command::WizardPosition => {
                                    let hero = hero_pos();
                                    msg_str(&format!("@ {},{}", hero.y, hero.x));
                                }
                                Command::WizardCreate => create_obj(),
                                Command::WizardInpack => {
                                    msg_str(&format!("inpack = {}", get_inpack()));
                                }
                                Command::WizardInventory => {
                                    let _ = inventory(&crate::game::PLAYER.pack(), ItemFilter::Any);
                                }
                                Command::WizardIdentify => whatis(false as u8, ItemFilter::Any),
                                Command::WizardDown => {
                                    crate::game::set_current_depth(
                                        crate::game::current_depth() + 1,
                                    );
                                    new_level();
                                }
                                Command::WizardUp => {
                                    crate::game::set_current_depth(
                                        crate::game::current_depth() - 1,
                                    );
                                    new_level();
                                }
                                Command::WizardMap => show_map(),
                                Command::WizardTeleport => teleport(),
                                Command::WizardFood => {
                                    msg_str(&format!("food left: {}", get_food_left()));
                                }
                                Command::WizardAddPassage => add_pass(),
                                Command::WizardToggleSee => {
                                    turn_see(if player_has(MonsterFlags::SEEMONST) {
                                        true as u8
                                    } else {
                                        false as u8
                                    });
                                }
                                Command::WizardCharge => {
                                    let item = get_item_id(
                                        "charge",
                                        ItemFilter::Category(ItemType::STICK),
                                    );
                                    if let Some(id) = item {
                                        OBJECTS.with_object_mut(id, |o| o.o_arm = 10000);
                                    }
                                }
                                Command::WizardGear => {
                                    for _ in 0..9 {
                                        raise_level();
                                    }
                                    /*
                                     * Give him a sword (+1,+1)
                                     */
                                    let sword = new_item_id();
                                    init_weapon(sword, TWOSWORD);
                                    OBJECTS.with_object_mut(sword, |o| {
                                        o.o_hplus = 1;
                                        o.o_dplus = 1;
                                    });
                                    add_pack_id(Some(sword), true);
                                    PLAYER.set_weapon_id(Some(sword));
                                    /*
                                     * And his suit of armor
                                     */
                                    let armor = new_item_id();
                                    OBJECTS.with_object_mut(armor, |o| {
                                        o.o_type = ItemType::Armor(PLATE_MAIL);
                                        o.o_which = PLATE_MAIL;
                                        o.o_arm = -5;
                                        o.o_flags.insert(ObjectFlags::KNOW);
                                        o.o_count = 1;
                                        o.o_group = 0;
                                    });
                                    PLAYER.set_armor_id(Some(armor));
                                    add_pack_id(Some(armor), true);
                                }
                                Command::WizardList => pr_list(),
                                _ => illcom(command),
                            }
                        } else {
                            illcom(command);
                        }
                    }
                }
                break; // Fall out of the dispatch loop; C's `break` out of switch.
            }
        }

        if crate::startup::exit_requested() {
            return;
        }

        /*
         * If he ran into something to take, let him pick it up.
         */
        if take != 0 {
            pick_up(take);
        }
        if running == 0 {
            door_stop = false as u8;
        }
        if after == 0 {
            ntimes += 1;
        }
    }

    do_daemons(AFTER);
    do_fuses(AFTER);
}

/// Read the next command, consume any repeat-count prefix, and report whether
/// the command should be dispatched this turn (cooldown turns dispatch nothing).
unsafe fn read_command(command_state: &mut CommandState) -> (Command, bool) {
    if no_command != 0 {
        no_command -= 1;
        if no_command == 0 {
            crate::game::PLAYER.add_flag(MonsterFlags::RUN);
            msg_str("you can move again");
        }
        return (Command::Rest, false);
    }

    let mut command = if running != 0 || to_death != 0 {
        Command::Move(runch)
    } else if count != 0 {
        command_state.repeat_command
    } else {
        let command = Command::from_key_event(crate::ui::input::read_key_event());
        move_on = false as u8;
        if mpos != 0 {
            msg_str("");
        }
        command
    };

    command_state.new_count = false;
    if let Command::Digit(first_digit) = command {
        count = 0;
        command_state.new_count = true;
        let (next_command, prefix_count) = Command::capture_count_prefix(first_digit, || {
            Command::from_key_event(crate::ui::input::read_key_event())
        });
        command = next_command;
        count = prefix_count;
        command_state.repeat_command = command;
        if !command_state.repeat_command.is_repeatable() {
            count = 0;
        }
    }

    (command, true)
}

fn initial_move_budget(haste: bool) -> i32 {
    if haste {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod command_loop_tests {
    use super::initial_move_budget;

    #[test]
    fn haste_adds_one_move_to_the_initial_budget() {
        assert_eq!(initial_move_budget(false), 1);
        assert_eq!(initial_move_budget(true), 2);
    }
}

// ─── illcom() ─────────────────────────────────────────────────────────────────

/// illcom:
/// What to do with an illegal command.
///
/// Uses globals: save_msg, count.
pub unsafe fn illcom(command: Command) {
    save_msg = false as u8;
    count = 0;
    msg_str(&format!(
        "illegal command '{}'",
        command.illegal_command_name()
    ));
    save_msg = true as u8;
}

// ─── search() ─────────────────────────────────────────────────────────────────

/// search:
/// Player gropes about him to find hidden things.
///
/// Uses globals: hero, player, places (via chat/flat), count, running,
/// terse, tr_name.
pub unsafe fn search() {
    let hero = hero_pos();
    let ey = hero.y + 1;
    let ex = hero.x + 1;
    let mut probinc: i32 = 0;
    let mut found = false;

    if player_has(MonsterFlags::HALU) {
        probinc += 3;
    }
    if player_has(MonsterFlags::BLIND) {
        probinc += 2;
    }

    let mut y = hero.y - 1;
    while y <= ey {
        let mut x = hero.x - 1;
        while x <= ex {
            if y == hero.y && x == hero.x {
                x += 1;
                continue;
            }
            let flags = crate::draw::flat_at(y, x);
            if (flags as u8 & F_REAL as u8) == 0 {
                match crate::game::tile_at(y, x) {
                    crate::tile::Tile::Wall | crate::tile::Tile::HiddenDoor => {
                        if rnd(5 + probinc) == 0 {
                            crate::draw::reveal_secret_at(y, x);
                            msg_str("a secret door");
                            found = true;
                            count = false as u8 as i32;
                            running = false as u8;
                        }
                    }
                    crate::tile::Tile::Trap(_) => {
                        if rnd(2 + probinc) == 0 {
                            crate::level::with_current_level_mut(|current| {
                                current.reveal_trap(y as usize, x as usize);
                            });
                            if terse == 0 {
                                addmsg_str("you found ");
                            }
                            if player_has(MonsterFlags::HALU) {
                                let name = crate::game::globals::trap_name(rnd(
                                    GameConfig::TRAP_KIND_COUNT,
                                )
                                    as usize);
                                msg_str(&name);
                            } else {
                                let name = crate::game::globals::trap_name(
                                    crate::draw::trap_kind_at(y, x) as usize,
                                );
                                msg_str(&name);
                            }
                            found = true;
                            count = false as u8 as i32;
                            running = false as u8;
                        }
                    }
                    crate::tile::Tile::Empty => {
                        if rnd(3 + probinc) == 0 {
                            crate::draw::reveal_secret_at(y, x);
                            found = true;
                            count = false as u8 as i32;
                            running = false as u8;
                        }
                    }
                    _ => {}
                }
            }
            x += 1;
        }
        y += 1;
    }

    if found {
        look(false as u8);
    }
}

// ─── d_level() / u_level() / levit_check() ────────────────────────────────────

/// d_level:
/// He wants to go down a level.
///
/// Uses globals: hero, places (via chat), level, seenstairs.
pub unsafe fn d_level() {
    if levit_check() != 0 {
        return;
    }
    let hero = hero_pos();
    if crate::game::tile_at(hero.y, hero.x) != crate::tile::Tile::Stairs {
        msg_str("I see no way down");
    } else {
        crate::game::set_current_depth(crate::game::current_depth() + 1);
        seenstairs = false as u8;
        new_level();
    }
}

/// u_level:
/// He wants to go up a level.
///
/// Uses globals: hero, places (via chat), amulet, level.
pub unsafe fn u_level() {
    if levit_check() != 0 {
        return;
    }
    let hero = hero_pos();
    if crate::game::tile_at(hero.y, hero.x) == crate::tile::Tile::Stairs {
        if amulet != 0 {
            crate::game::set_current_depth(crate::game::current_depth() - 1);
            if crate::game::current_depth() == 0 {
                total_winner();
                if crate::startup::exit_requested() {
                    return;
                }
            }
            new_level();
            msg_str("you feel a wrenching sensation in your gut");
        } else {
            msg_str("your way is magically blocked");
        }
    } else {
        msg_str("I see no way up");
    }
}

/// levit_check:
/// Check to see if she's levitating, and if she is, print an
/// appropriate message.
///
/// Uses globals: player.
pub unsafe fn levit_check() -> u8 {
    if !player_has(MonsterFlags::LEVIT) {
        return false as u8;
    }
    msg_str("You can't.  You're floating off the ground!");
    true as u8
}

// ─── call() ───────────────────────────────────────────────────────────────────

/// call:
/// Allow a user to call a potion, scroll, or ring something.
///
/// Uses globals: ring_info, r_stones, pot_info, p_colors, scr_info,
/// s_names, ws_info, ws_made, terse, prbuf.
pub unsafe fn call() {
    let Some(obj) = get_item_id("call", ItemFilter::Callable) else {
        return;
    };

    let (otype, label, o_which) =
        match OBJECTS.with_object(obj, |o| (o.o_type, o.o_label.clone(), o.o_which)) {
            Some(v) => v,
            None => return,
        };

    if matches!(otype, ItemType::Food) {
        msg_str("you can't call that anything");
        return;
    }

    // Weapons and armor store their player-assigned name directly in `o_label`.
    if !matches!(
        otype,
        ItemType::Ring(_) | ItemType::Potion(_) | ItemType::Scroll(_) | ItemType::Stick(_)
    ) {
        if let Some(elsewise) = label.as_ref() {
            if terse == 0 {
                addmsg_str("Was ");
            }
            msg_str(&format!("called \"{}\"", elsewise));
        }

        if terse != 0 {
            msg_str("call it: ");
        } else {
            msg_str("what do you want to call it? ");
        }

        let initial = label.clone().unwrap_or_default();
        if let Some(text) = read_line(&initial) {
            OBJECTS.with_object_mut(obj, |o| o.o_label = Some(text));
        }
        return;
    }

    // Magic items keep their call-name in the obj-info table entry.
    let op = match otype {
        ItemType::Ring(_) => &mut ring_info[..],
        ItemType::Potion(_) => &mut pot_info[..],
        ItemType::Scroll(_) => &mut scr_info[..],
        _ => &mut ws_info[..],
    };

    let which = o_which as usize;
    let mut elsewise: String = match otype {
        ItemType::Ring(_) => cstr_at(r_stones[which]),
        ItemType::Potion(_) => cstr_at(p_colors[which]),
        ItemType::Scroll(_) => crate::game::globals::scroll_name(which),
        _ => cstr_at(ws_made[which]),
    };

    if let Some(guess) = &op[which].oi_guess {
        elsewise = guess.clone();
    }

    if op[which].oi_know {
        msg_str("that has already been identified");
        return;
    }

    if op[which].oi_guess.is_some() {
        if terse == 0 {
            addmsg_str("Was ");
        }
        msg_str(&format!("called \"{}\"", elsewise));
    }

    if terse != 0 {
        msg_str("call it: ");
    } else {
        msg_str("what do you want to call it? ");
    }

    if let Some(text) = read_line(&elsewise) {
        op[which].oi_guess = Some(text);
    }
}

// ─── current() ────────────────────────────────────────────────────────────────

/// current:
/// Print the currently equipped weapon/armor/ring, identified by arena handle.
///
/// Uses globals: after, terse, inv_describe.
pub unsafe fn current(cur: Option<crate::item::arena::ThingId>, how: &str, where_: &str) {
    after = false as u8;
    if let Some(id) = cur {
        if terse == 0 {
            addmsg_str(&format!("you are {} (", how));
        }
        inv_describe = false as u8;
        let packch = OBJECTS.with_object(id, |o| o.o_packch).unwrap_or(0);
        addmsg_str(&format!("{}) {}", packch as char, inv_name_id(id, true)));
        inv_describe = true as u8;
        if !where_.is_empty() {
            addmsg_str(&format!(" {}", where_));
        }
        endmsg();
    } else {
        if terse == 0 {
            addmsg_str("you are ");
        }
        addmsg_str(&format!("{} nothing", how));
        if !where_.is_empty() {
            addmsg_str(&format!(" {}", where_));
        }
        endmsg();
    }
}

// ─── pr_list() ────────────────────────────────────────────────────────────────

/// pr_list:
/// Wizard command to list the objects on the current level.
///
/// Uses globals: lvl_obj, mlist.
pub unsafe fn pr_list() {
    for obj in crate::game::item_ids() {
        let otype = crate::item::arena::with_object(obj, |o| o.o_type).unwrap_or(ItemType::None);
        msg_str(&format!(
            "{}) {}",
            crate::draw::item_glyph(otype),
            inv_name_id(obj, false)
        ));
    }
}
