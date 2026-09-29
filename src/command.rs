//! Player command vocabulary and key decoding.
//!
//! Ported from `src/c/command.c` to Rust.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use crate::direction::Direction;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    UnknownKey,
}

impl Command {
    pub fn from_key_event(event: KeyEvent) -> Self {
        match event.code {
            KeyCode::Char(character) => {
                if event.modifiers.contains(KeyModifiers::CONTROL) {
                    return match character.to_ascii_lowercase() {
                        'a' => Self::WizardUp,
                        'b' => Self::RunPrefix(Direction::SouthWest),
                        'c' => Self::Unknown(3),
                        'd' => Self::WizardDown,
                        'e' => Self::WizardFood,
                        'f' => Self::WizardMap,
                        'g' => Self::WizardInventory,
                        'h' => Self::RunPrefix(Direction::West),
                        'i' => Self::WizardGear,
                        'j' => Self::RunPrefix(Direction::South),
                        'k' => Self::RunPrefix(Direction::North),
                        'l' => Self::RunPrefix(Direction::East),
                        'n' => Self::RunPrefix(Direction::SouthEast),
                        'p' => Self::MessageHistory,
                        'r' => Self::Refresh,
                        't' => Self::WizardTeleport,
                        'u' => Self::RunPrefix(Direction::NorthEast),
                        'w' => Self::WizardIdentify,
                        'x' => Self::WizardToggleSee,
                        'y' => Self::RunPrefix(Direction::NorthWest),
                        '~' => Self::WizardCharge,
                        _ => Self::UnknownKey,
                    };
                }
                if event
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
                {
                    return Self::UnknownKey;
                }
                let shifted =
                    event.modifiers.contains(KeyModifiers::SHIFT) || character.is_ascii_uppercase();
                match character.to_ascii_lowercase() {
                    '0'..='9' => Self::Digit(character as u8 - b'0'),
                    ',' => Self::Pickup,
                    '!' => Self::Shell,
                    'h' => movement(Direction::West, shifted),
                    'j' => movement(Direction::South, shifted),
                    'k' => movement(Direction::North, shifted),
                    'l' => movement(Direction::East, shifted),
                    'y' => movement(Direction::NorthWest, shifted),
                    'u' => movement(Direction::NorthEast, shifted),
                    'b' => movement(Direction::SouthWest, shifted),
                    'n' => movement(Direction::SouthEast, shifted),
                    'f' => {
                        if shifted {
                            Self::FireKamikaze
                        } else {
                            Self::Fire
                        }
                    }
                    't' => {
                        if shifted {
                            Self::TakeOff
                        } else {
                            Self::Throw
                        }
                    }
                    'a' => Self::Again,
                    'q' => {
                        if shifted {
                            Self::Quit
                        } else {
                            Self::Quaff
                        }
                    }
                    'i' => {
                        if shifted {
                            Self::InventorySelect
                        } else {
                            Self::Inventory
                        }
                    }
                    'd' => {
                        if shifted {
                            Self::Discover
                        } else {
                            Self::Drop
                        }
                    }
                    'r' => {
                        if shifted {
                            Self::RingOff
                        } else {
                            Self::ReadScroll
                        }
                    }
                    'e' => Self::Eat,
                    'w' => {
                        if shifted {
                            Self::Wear
                        } else {
                            Self::Wield
                        }
                    }
                    'p' => {
                        if shifted {
                            Self::RingOn
                        } else {
                            Self::Unknown(b'p')
                        }
                    }
                    'o' => Self::Options,
                    'c' => {
                        if shifted {
                            Self::WizardCreate
                        } else {
                            Self::Call
                        }
                    }
                    '>' => Self::Descend,
                    '<' => Self::Ascend,
                    '?' => Self::Help,
                    '/' => Self::Identify,
                    's' => {
                        if shifted {
                            Self::Save
                        } else {
                            Self::Search
                        }
                    }
                    'z' => Self::Zap,
                    'v' => Self::Version,
                    '.' => Self::Rest,
                    ' ' => Self::Space,
                    '^' => Self::FindTrap,
                    '+' => Self::WizardToggle,
                    'm' => Self::MoveOn,
                    ')' => Self::CurrentWeapon,
                    ']' => Self::CurrentArmor,
                    '=' => Self::CurrentRings,
                    '@' => Self::Status,
                    '|' => Self::WizardPosition,
                    '$' => Self::WizardInpack,
                    '*' => Self::WizardList,
                    _ if character.is_ascii() => Self::Unknown(character as u8),
                    _ => Self::UnknownKey,
                }
            }
            KeyCode::Left => navigation_command(Direction::West, event.modifiers),
            KeyCode::Down => navigation_command(Direction::South, event.modifiers),
            KeyCode::Up => navigation_command(Direction::North, event.modifiers),
            KeyCode::Right => navigation_command(Direction::East, event.modifiers),
            KeyCode::Home => navigation_command(Direction::NorthWest, event.modifiers),
            KeyCode::PageUp => navigation_command(Direction::NorthEast, event.modifiers),
            KeyCode::End => navigation_command(Direction::SouthWest, event.modifiers),
            KeyCode::PageDown => navigation_command(Direction::SouthEast, event.modifiers),
            KeyCode::Esc => Self::Escape,
            KeyCode::Tab => Self::WizardGear,
            _ => Self::UnknownKey,
        }
    }

    pub(crate) fn illegal_command_name(self) -> String {
        format!("{self:?}")
    }

    pub const fn is_repeatable(self) -> bool {
        matches!(
            self,
            Self::RunPrefix(_)
                | Self::Move(_)
                | Self::Run(_)
                | Self::MoveOn
                | Self::Quaff
                | Self::ReadScroll
                | Self::Search
                | Self::Throw
                | Self::Zap
                | Self::Rest
                | Self::Again
                | Self::InventorySelect
                | Self::WizardCreate
                | Self::WizardDown
                | Self::WizardUp
        )
    }
}

const fn movement(direction: Direction, run: bool) -> Command {
    if run {
        Command::Run(direction)
    } else {
        Command::Move(direction)
    }
}

fn navigation_command(direction: Direction, modifiers: KeyModifiers) -> Command {
    if modifiers.intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL) {
        Command::RunPrefix(direction)
    } else {
        Command::Move(direction)
    }
}

#[cfg(test)]
mod command_type_tests {
    use super::Command;
    use crate::direction::Direction;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use glam::IVec2;

    #[test]
    fn key_events_map_to_commands_with_modifiers() {
        let event = |code, modifiers| KeyEvent::new(code, modifiers);

        assert_eq!(
            Command::from_key_event(event(KeyCode::Char('h'), KeyModifiers::NONE)),
            Command::Move(Direction::West)
        );
        assert_eq!(
            Command::from_key_event(event(KeyCode::Char('H'), KeyModifiers::SHIFT)),
            Command::Run(Direction::West)
        );
        assert_eq!(
            Command::from_key_event(event(KeyCode::Char('j'), KeyModifiers::CONTROL)),
            Command::RunPrefix(Direction::South)
        );
        assert_eq!(
            Command::from_key_event(event(KeyCode::Char('q'), KeyModifiers::SHIFT)),
            Command::Quit
        );
        assert_eq!(
            Command::from_key_event(event(KeyCode::Left, KeyModifiers::NONE)),
            Command::Move(Direction::West)
        );
        assert_eq!(
            Command::from_key_event(event(KeyCode::Left, KeyModifiers::SHIFT)),
            Command::RunPrefix(Direction::West)
        );
        assert_eq!(
            Command::from_key_event(event(KeyCode::F(1), KeyModifiers::NONE)),
            Command::UnknownKey
        );
    }

    #[test]
    fn illegal_command_label_uses_command_name() {
        assert_eq!(Command::WizardMap.illegal_command_name(), "WizardMap");
        assert_eq!(Command::UnknownKey.illegal_command_name(), "UnknownKey");
    }

    #[test]
    fn direction_has_expected_movement_deltas() {
        let cases = [
            (Direction::West, IVec2::new(-1, 0)),
            (Direction::South, IVec2::new(0, 1)),
            (Direction::North, IVec2::new(0, -1)),
            (Direction::East, IVec2::new(1, 0)),
            (Direction::NorthWest, IVec2::new(-1, -1)),
            (Direction::NorthEast, IVec2::new(1, -1)),
            (Direction::SouthWest, IVec2::new(-1, 1)),
            (Direction::SouthEast, IVec2::new(1, 1)),
        ];

        for (direction, delta) in cases {
            assert_eq!(direction.delta(), delta);
        }
    }

    #[test]
    fn repeatability_matches_supported_legacy_prefix_commands() {
        for command in [
            Command::RunPrefix(Direction::West),
            Command::RunPrefix(Direction::SouthEast),
            Command::Move(Direction::NorthWest),
            Command::Run(Direction::East),
            Command::MoveOn,
            Command::Quaff,
            Command::ReadScroll,
            Command::Search,
            Command::Throw,
            Command::Zap,
            Command::Rest,
            Command::Again,
            Command::InventorySelect,
            Command::WizardCreate,
            Command::WizardDown,
            Command::WizardUp,
        ] {
            assert!(command.is_repeatable(), "{command:?}");
        }
        for command in [
            Command::Quit,
            Command::Inventory,
            Command::Drop,
            Command::FireKamikaze,
            Command::Shell,
            Command::WizardGear,
        ] {
            assert!(!command.is_repeatable(), "{command:?}");
        }
    }
}
