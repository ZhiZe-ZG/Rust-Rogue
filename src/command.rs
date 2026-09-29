//! Player command reading and dispatch.
//!
//! Ported from `src/c/command.c` to Rust.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use crate::config::GameConfig;
use crate::daemon::{do_daemons, do_fuses};
use crate::draw::{add_pass, look};
use crate::entity::chase::{diag_ok, see_monst};
use crate::entity::player::{do_move, do_run, MonsterFlags, ObjectFlags, Thing};
use crate::game::{MonsterId, PLAYER};
use crate::game::globals::{pot_info, ring_info, scr_info, ws_info, ObjInfo};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::help::{help, identify};
use crate::item::armor::{take_off, wear};
use crate::item::pack::{add_pack_id, get_item_id, inventory, pick_up, picky_inven};
use crate::item::potions::{quaff, raise_level, turn_see};
use crate::item::rings::{ring_off, ring_on, RingType};
use crate::item::scrolls::read_scroll;
use crate::item::sticks::do_zap;
use crate::item::arena::{new_item_id, ThingId, OBJECTS};
use crate::item::things::{discovered, drop, inv_name_id};
use crate::item::weapons::{init_weapon, missile, wield};
use crate::level::new_level;
use crate::misc::{eat, get_dir};
use crate::options::{option, read_line};
use crate::rip::total_winner;
use crate::rnd::rnd;
use crate::save::save_game;
use crate::startup::{quit, shell};
use crate::ui::input::readchar;
use crate::ui::output::{self, addmsg_str, endmsg, msg_str, status};
use crate::wizard::{create_obj, show_map, teleport, whatis};
use glam::IVec2;

/// A normalized movement direction used by command and run handling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    #[default]
    None,
    West,
    South,
    North,
    East,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

impl Direction {
    pub const fn from_byte(key: u8) -> Option<Self> {
        match key.to_ascii_lowercase() {
            b'h' => Some(Self::West),
            b'j' => Some(Self::South),
            b'k' => Some(Self::North),
            b'l' => Some(Self::East),
            b'y' => Some(Self::NorthWest),
            b'u' => Some(Self::NorthEast),
            b'b' => Some(Self::SouthWest),
            b'n' => Some(Self::SouthEast),
            _ => None,
        }
    }

    pub const fn to_byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::West => b'h',
            Self::South => b'j',
            Self::North => b'k',
            Self::East => b'l',
            Self::NorthWest => b'y',
            Self::NorthEast => b'u',
            Self::SouthWest => b'b',
            Self::SouthEast => b'n',
        }
    }

    pub const fn delta(self) -> IVec2 {
        match self {
            Self::None => IVec2::ZERO,
            Self::West => IVec2::new(-1, 0),
            Self::South => IVec2::new(0, 1),
            Self::North => IVec2::new(0, -1),
            Self::East => IVec2::new(1, 0),
            Self::NorthWest => IVec2::new(-1, -1),
            Self::NorthEast => IVec2::new(1, -1),
            Self::SouthWest => IVec2::new(-1, 1),
            Self::SouthEast => IVec2::new(1, 1),
        }
    }
}

/// A command key interpreted by the main dispatcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    Digit(u8),
    Pickup,
    Shell,
    Move(Direction),
    Run(Direction),
    RunPrefix(Direction),
    Fire,
    FireKamikaze,
    Throw,
    Again,
    Quaff,
    Quit,
    Inventory,
    InventorySelect,
    Drop,
    ReadScroll,
    Eat,
    Wield,
    Wear,
    TakeOff,
    RingOn,
    RingOff,
    Options,
    Call,
    Descend,
    Ascend,
    Help,
    Identify,
    Search,
    Zap,
    Discover,
    MessageHistory,
    Refresh,
    Version,
    Save,
    Rest,
    FindTrap,
    WizardToggle,
    Escape,
    MoveOn,
    CurrentWeapon,
    CurrentArmor,
    CurrentRings,
    Status,
    WizardPosition,
    WizardCreate,
    WizardInpack,
    WizardInventory,
    WizardIdentify,
    WizardDown,
    WizardUp,
    WizardMap,
    WizardTeleport,
    WizardFood,
    WizardAddPassage,
    WizardToggleSee,
    WizardCharge,
    WizardGear,
    WizardList,
    Space,
    Unknown(u8),
}

impl Command {
    pub const fn from_byte(key: u8) -> Self {
        if key.is_ascii_digit() {
            return Self::Digit(key - b'0');
        }
        match key {
            b',' => Self::Pickup,
            b'!' => Self::Shell,
            b'h' => Self::Move(Direction::West),
            b'j' => Self::Move(Direction::South),
            b'k' => Self::Move(Direction::North),
            b'l' => Self::Move(Direction::East),
            b'y' => Self::Move(Direction::NorthWest),
            b'u' => Self::Move(Direction::NorthEast),
            b'b' => Self::Move(Direction::SouthWest),
            b'n' => Self::Move(Direction::SouthEast),
            b'H' => Self::Run(Direction::West),
            b'J' => Self::Run(Direction::South),
            b'K' => Self::Run(Direction::North),
            b'L' => Self::Run(Direction::East),
            b'Y' => Self::Run(Direction::NorthWest),
            b'U' => Self::Run(Direction::NorthEast),
            b'B' => Self::Run(Direction::SouthWest),
            b'N' => Self::Run(Direction::SouthEast),
            0x08 => Self::RunPrefix(Direction::West),
            0x0a => Self::RunPrefix(Direction::South),
            0x0b => Self::RunPrefix(Direction::North),
            0x0c => Self::RunPrefix(Direction::East),
            0x19 => Self::RunPrefix(Direction::NorthWest),
            0x15 => Self::RunPrefix(Direction::NorthEast),
            0x02 => Self::RunPrefix(Direction::SouthWest),
            0x0e => Self::RunPrefix(Direction::SouthEast),
            b'f' => Self::Fire,
            b'F' => Self::FireKamikaze,
            b't' => Self::Throw,
            b'a' => Self::Again,
            b'q' => Self::Quaff,
            b'Q' => Self::Quit,
            b'i' => Self::Inventory,
            b'I' => Self::InventorySelect,
            b'd' => Self::Drop,
            b'r' => Self::ReadScroll,
            b'e' => Self::Eat,
            b'w' => Self::Wield,
            b'W' => Self::Wear,
            b'T' => Self::TakeOff,
            b'P' => Self::RingOn,
            b'R' => Self::RingOff,
            b'o' => Self::Options,
            b'c' => Self::Call,
            b'>' => Self::Descend,
            b'<' => Self::Ascend,
            b'?' => Self::Help,
            b'/' => Self::Identify,
            b's' => Self::Search,
            b'z' => Self::Zap,
            b'D' => Self::Discover,
            0x10 => Self::MessageHistory,
            0x12 => Self::Refresh,
            b'v' => Self::Version,
            b'S' => Self::Save,
            b'.' => Self::Rest,
            b' ' => Self::Space,
            b'^' => Self::FindTrap,
            b'+' => Self::WizardToggle,
            0x1b => Self::Escape,
            b'm' => Self::MoveOn,
            b')' => Self::CurrentWeapon,
            b']' => Self::CurrentArmor,
            b'=' => Self::CurrentRings,
            b'@' => Self::Status,
            b'|' => Self::WizardPosition,
            b'C' => Self::WizardCreate,
            b'$' => Self::WizardInpack,
            0x07 => Self::WizardInventory,
            0x17 => Self::WizardIdentify,
            0x04 => Self::WizardDown,
            0x01 => Self::WizardUp,
            0x06 => Self::WizardMap,
            0x14 => Self::WizardTeleport,
            0x05 => Self::WizardFood,
            0x03 => Self::WizardAddPassage,
            0x18 => Self::WizardToggleSee,
            0x1e => Self::WizardCharge,
            0x09 => Self::WizardGear,
            b'*' => Self::WizardList,
            _ => Self::Unknown(key),
        }
    }

    pub const fn to_byte(self) -> u8 {
        match self {
            Self::Digit(digit) => b'0' + digit,
            Self::Pickup => b',', Self::Shell => b'!',
            Self::Move(dir) => dir.to_byte(),
            Self::Run(dir) => dir.to_byte().to_ascii_uppercase(),
            Self::RunPrefix(dir) => match dir {
                Direction::West => 0x08,
                Direction::South => 0x0a,
                Direction::North => 0x0b,
                Direction::East => 0x0c,
                Direction::NorthWest => 0x19,
                Direction::NorthEast => 0x15,
                Direction::SouthWest => 0x02,
                Direction::SouthEast => 0x0e,
                Direction::None => 0,
            },
            Self::Fire => b'f', Self::FireKamikaze => b'F', Self::Throw => b't',
            Self::Again => b'a', Self::Quaff => b'q', Self::Quit => b'Q',
            Self::Inventory => b'i', Self::InventorySelect => b'I', Self::Drop => b'd',
            Self::ReadScroll => b'r', Self::Eat => b'e', Self::Wield => b'w',
            Self::Wear => b'W', Self::TakeOff => b'T', Self::RingOn => b'P',
            Self::RingOff => b'R', Self::Options => b'o', Self::Call => b'c',
            Self::Descend => b'>', Self::Ascend => b'<', Self::Help => b'?',
            Self::Identify => b'/', Self::Search => b's', Self::Zap => b'z',
            Self::Discover => b'D', Self::MessageHistory => 0x10, Self::Refresh => 0x12,
            Self::Version => b'v', Self::Save => b'S', Self::Rest => b'.', Self::Space => b' ',
            Self::FindTrap => b'^', Self::WizardToggle => b'+', Self::Escape => 0x1b,
            Self::MoveOn => b'm', Self::CurrentWeapon => b')', Self::CurrentArmor => b']',
            Self::CurrentRings => b'=', Self::Status => b'@', Self::WizardPosition => b'|',
            Self::WizardCreate => b'C', Self::WizardInpack => b'$', Self::WizardInventory => 0x07,
            Self::WizardIdentify => 0x17, Self::WizardDown => 0x04, Self::WizardUp => 0x01,
            Self::WizardMap => 0x06, Self::WizardTeleport => 0x14, Self::WizardFood => 0x05,
            Self::WizardAddPassage => 0x03, Self::WizardToggleSee => 0x18,
            Self::WizardCharge => 0x1e, Self::WizardGear => 0x09, Self::WizardList => b'*',
            Self::Unknown(key) => key,
        }
    }

    pub const fn is_repeatable(self) -> bool {
        matches!(self,
            Self::RunPrefix(_) | Self::Move(_) | Self::Run(_) | Self::MoveOn |
            Self::Quaff | Self::ReadScroll | Self::Search | Self::Throw | Self::Zap |
            Self::Rest | Self::Again | Self::InventorySelect | Self::WizardCreate |
            Self::WizardDown | Self::WizardUp
        )
    }
}

// ─── Constants ────────────────────────────────────────────────────────────────

const MAXSTR: usize = 1024;

// Glyphs
const PASSAGE: u8 = b'#' as u8;
const DOOR: u8 = b'+' as u8;
const FLOOR: u8 = b'.' as u8;
const TRAP: u8 = b'^' as u8;
const STAIRS: u8 = b'%' as u8;

// Map flags
const F_REAL: u8 = 0x10u8 as u8;
const F_SEEN: u8 = 0x40u8 as u8;
const F_TMASK: u8 = 0x07u8 as u8;

// Escape
const ESCAPE: i32 = 27;

// get_str() return codes
const NORM: i32 = 0;

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

// ─── Static locals for command() ─────────────────────────────────────────────

static mut COUNTCH: Command = Command::Unknown(0);
static mut DIRECTION: Command = Command::Unknown(0);
static mut NEWCOUNT: u8 = false as u8;

// ─── Extern C globals ─────────────────────────────────────────────────────────

use crate::game::globals::{after, again, amulet, count, delta, dir_ch, door_stop, firstmove, get_dnum, get_food_left, get_inpack, has_hit, inv_describe, jump, kamikaze, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick, lastscore, max_hit, move_on, mpos, no_command, noscore, p_colors, purse, q_comm, r_stones, runch, running, save_msg, seenstairs, stat_msg, take, terse, to_death, wizard, ws_made};


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
pub unsafe fn command() {
    let mut ch: u8;
    let mut ntimes: i32 = 1; // Number of player moves
    let mut mp: Option<MonsterId>;

    if player_has(MonsterFlags::HASTE) {
        ntimes += 1;
    }

    /*
     * Let the daemons start up
     */
    do_daemons(BEFORE);
    do_fuses(BEFORE);

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

        /*
         * these are illegal things for the player to be, so if any are
         * set, someone's been poking in memory
         */
        if player_has(
            MonsterFlags::SLOW
                | MonsterFlags::GREED
                | MonsterFlags::INVIS
                | MonsterFlags::REGEN
                | MonsterFlags::TARGET,
        ) {
            std::process::exit(1);
        }

        look(true as u8);
        if running == 0 {
            door_stop = false as u8;
        }
        crate::daemon::Daemon::UiRender.run(0);
        lastscore = purse;
        let hero = hero_pos();
        output::move_cursor(IVec2::new(hero.x, hero.y));
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

        if no_command == 0 {
            if running != 0 || to_death != 0 {
                ch = runch.to_byte();
            } else if count != 0 {
                ch = COUNTCH.to_byte();
            } else {
                ch = readchar() as u8;
                move_on = false as u8;
                if mpos != 0 {
                    // Erase message if it's there
                    msg_str("");
                }
            }
        } else {
            ch = b'.';
        }

        if no_command != 0 {
            no_command -= 1;
            if no_command == 0 {
                crate::game::PLAYER.add_flag(MonsterFlags::RUN);
                msg_str("you can move again");
            }
        } else {
            /*
             * check for prefixes
             */
            NEWCOUNT = false as u8;
            if ch.is_ascii_digit() {
                count = 0;
                NEWCOUNT = true as u8;
                while ch.is_ascii_digit() {
                    count = count * 10 + (ch - b'0') as i32;
                    if count > 255 {
                        count = 255;
                    }
                    ch = readchar() as u8;
                }
                COUNTCH = Command::from_byte(ch);
                /*
                 * turn off count for commands which don't make sense
                 * to repeat
                 */
                if !COUNTCH.is_repeatable() {
                    count = 0;
                }
            }

            let mut command = Command::from_byte(ch);

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
                                let code = crate::item::arena::with_object(obj, |o| o.o_type.code())
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
                        command = if count != 0 && NEWCOUNT == 0 {
                            DIRECTION
                        } else {
                            let run = Command::Run(direction);
                            DIRECTION = run;
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
                            mp = crate::game::monster_id_at(delta.y, delta.x);
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
                                    crate::game::MONSTER_LIST.with_mut(id, |t| {
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
                        if last_comm == Command::Unknown(0) {
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
                        quit(0);
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
                    Command::Discover => {
                        after = false as u8;
                        discovered();
                    }
                    Command::MessageHistory => {
                        after = false as u8;
                        msg_str(&crate::game::globals::huh_string());
                    }
                    Command::Refresh => {
                        after = false as u8;
                        output::set_clear_on_refresh(true);
                        output::refresh_window();
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
                                let name = crate::game::globals::trap_name(
                                    rnd(GameConfig::TRAP_KIND_COUNT) as usize,
                                );
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
                                    get_dnum()
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
                            COUNTCH = Command::Move(dir_ch);
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
                            if terse != 0 {
                                "(L)"
                            } else {
                                "on left hand"
                            },
                        );
                        current(
                            eq.right_ring_id(),
                            "wearing",
                            if terse != 0 {
                                "(R)"
                            } else {
                                "on right hand"
                            },
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
                                    msg_str(&format!(
                                        "inpack = {}",
                                        get_inpack()
                                    ));
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
                                    msg_str(&format!(
                                        "food left: {}",
                                        get_food_left()
                                    ));
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
                                    let item =
                                        get_item_id("charge", ItemFilter::Category(ItemType::STICK));
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
                                _ => illcom(command.to_byte() as i32),
                            }
                        } else {
                            illcom(command.to_byte() as i32);
                        }
                    }
                }
                break; // Fall out of the dispatch loop; C's `break` out of switch.
            }
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
    // Ring-of-searching / ring-of-teleportation effects, evaluated per hand
    // through the pointer-free equipment accessor.
    let equipment = PLAYER.equipment();
    for hand in 0..2usize {
        match equipment.ring_type(hand) {
            Some(RingType::Searching) => search(),
            Some(RingType::Teleport) => {
                if rnd(50) == 0 {
                    teleport();
                }
            }
            _ => {}
        }
    }
}

// ─── illcom() ─────────────────────────────────────────────────────────────────

/// illcom:
/// What to do with an illegal command.
///
/// Uses globals: save_msg, count.
pub unsafe fn illcom(ch: i32) {
    save_msg = false as u8;
    count = 0;
    msg_str(&format!(
        "illegal command '{}'",
        output::format_key(ch as u8)
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
                                let name = crate::game::globals::trap_name(
                                    rnd(GameConfig::TRAP_KIND_COUNT) as usize,
                                );
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

    let (otype, label, o_which) = match OBJECTS.with_object(obj, |o| {
        (o.o_type, o.o_label.clone(), o.o_which)
    }) {
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
            addmsg_str(&format!(
                "you are {} (",
                how
            ));
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
        addmsg_str(&format!(
            "{} nothing",
            how
        ));
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

#[cfg(test)]
mod command_type_tests {
    use super::{Command, Direction};
    use glam::IVec2;

    #[test]
    fn command_keys_round_trip_including_unknown_bytes() {
        let keys = [
            b'a', b'h', b'H', b'f', b'F', b',', b'!', b'>', b'<', b'@',
            b'?', b'/', b'=', b']', b')', b'*', 0x01, 0x08, 0x10, 0x1e,
            0xff,
        ];

        for key in keys {
            assert_eq!(Command::from_byte(key).to_byte(), key);
        }
        assert_eq!(Command::from_byte(0xff), Command::Unknown(0xff));
    }

    #[test]
    fn direction_accepts_uppercase_and_has_legacy_deltas() {
        let cases = [
            (b'h', Direction::West, IVec2::new(-1, 0)),
            (b'J', Direction::South, IVec2::new(0, 1)),
            (b'k', Direction::North, IVec2::new(0, -1)),
            (b'L', Direction::East, IVec2::new(1, 0)),
            (b'y', Direction::NorthWest, IVec2::new(-1, -1)),
            (b'U', Direction::NorthEast, IVec2::new(1, -1)),
            (b'b', Direction::SouthWest, IVec2::new(-1, 1)),
            (b'N', Direction::SouthEast, IVec2::new(1, 1)),
        ];

        for (key, direction, delta) in cases {
            assert_eq!(Direction::from_byte(key), Some(direction));
            assert_eq!(direction.delta(), delta);
            assert_eq!(Direction::from_byte(direction.to_byte()), Some(direction));
        }
        assert_eq!(Direction::from_byte(b'?'), None);
    }

    #[test]
    fn repeatability_matches_supported_legacy_prefix_commands() {
        for key in [
            b'\x02', b'\x08', b'\x0a', b'\x0b', b'\x0c', b'\x0e', b'\x15',
            b'\x19', b'.', b'a', b'b', b'h', b'j', b'k', b'l', b'm', b'n',
            b'q', b'r', b's', b't', b'u', b'y', b'z', b'B', b'C', b'H', b'I',
            b'J', b'K', b'L', b'N', b'U', b'Y', b'\x01', b'\x04',
        ] {
            assert!(Command::from_byte(key).is_repeatable(), "key {key:#x}");
        }
        for key in [b'Q', b'i', b'd', b'F', b'!', b'\x09'] {
            assert!(!Command::from_byte(key).is_repeatable(), "key {key:#x}");
        }
    }
}
