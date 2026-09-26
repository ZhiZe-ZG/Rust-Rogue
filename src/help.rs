//! Command help and map-symbol identification.

use std::os::raw::{c_int, c_uchar};

use crate::config::GameConfig;
use crate::globals::{lower_msg, monsters, mpos};
use crate::ui::input::{readchar, wait_for};
use crate::ui::output::msg_str;
use crate::ui::{output, Window};
use glam::IVec2;

const ESCAPE: c_int = 27;

struct HelpEntry {
    ch: u8,
    desc: &'static str,
    print: bool,
}

const fn help_entry(ch: u8, desc: &'static str, print: bool) -> HelpEntry {
    HelpEntry { ch, desc, print }
}

static HELP_ENTRIES: &[HelpEntry] = &[
    help_entry(b'?', "\tprints help", true),
    help_entry(b'/', "\tidentify object", true),
    help_entry(b'h', "\tleft", true),
    help_entry(b'j', "\tdown", true),
    help_entry(b'k', "\tup", true),
    help_entry(b'l', "\tright", true),
    help_entry(b'y', "\tup & left", true),
    help_entry(b'u', "\tup & right", true),
    help_entry(b'b', "\tdown & left", true),
    help_entry(b'n', "\tdown & right", true),
    help_entry(b'H', "\trun left", false),
    help_entry(b'J', "\trun down", false),
    help_entry(b'K', "\trun up", false),
    help_entry(b'L', "\trun right", false),
    help_entry(b'Y', "\trun up & left", false),
    help_entry(b'U', "\trun up & right", false),
    help_entry(b'B', "\trun down & left", false),
    help_entry(b'N', "\trun down & right", false),
    help_entry(0x08, "\trun left until adjacent", false),
    help_entry(0x0a, "\trun down until adjacent", false),
    help_entry(0x0b, "\trun up until adjacent", false),
    help_entry(0x0c, "\trun right until adjacent", false),
    help_entry(0x19, "\trun up & left until adjacent", false),
    help_entry(0x15, "\trun up & right until adjacent", false),
    help_entry(0x02, "\trun down & left until adjacent", false),
    help_entry(0x16, "\trun down & right until adjacent", false),
    help_entry(0, "\t<SHIFT><dir>: run that way", true),
    help_entry(0, "\t<CTRL><dir>: run till adjacent", true),
    help_entry(b'f', "<dir>\tfight till death or near death", true),
    help_entry(b't', "<dir>\tthrow something", true),
    help_entry(b'm', "<dir>\tmove onto without picking up", true),
    help_entry(b'z', "<dir>\tzap a wand in a direction", true),
    help_entry(b'^', "<dir>\tidentify trap type", true),
    help_entry(b's', "\tsearch for trap/secret door", true),
    help_entry(b'>', "\tgo down a staircase", true),
    help_entry(b'<', "\tgo up a staircase", true),
    help_entry(b'.', "\trest for a turn", true),
    help_entry(b',', "\tpick something up", true),
    help_entry(b'i', "\tinventory", true),
    help_entry(b'I', "\tinventory single item", true),
    help_entry(b'q', "\tquaff potion", true),
    help_entry(b'r', "\tread scroll", true),
    help_entry(b'e', "\teat food", true),
    help_entry(b'w', "\twield a weapon", true),
    help_entry(b'W', "\twear armor", true),
    help_entry(b'T', "\ttake armor off", true),
    help_entry(b'P', "\tput on ring", true),
    help_entry(b'R', "\tremove ring", true),
    help_entry(b'd', "\tdrop object", true),
    help_entry(b'c', "\tcall object", true),
    help_entry(b'a', "\trepeat last command", true),
    help_entry(b')', "\tprint current weapon", true),
    help_entry(b']', "\tprint current armor", true),
    help_entry(b'=', "\tprint current rings", true),
    help_entry(b'@', "\tprint current stats", true),
    help_entry(b'D', "\trecall what's been discovered", true),
    help_entry(b'o', "\texamine/set options", true),
    help_entry(0x12, "\tredraw screen", true),
    help_entry(0x10, "\trepeat last message", true),
    help_entry(0x1b, "\tcancel command", true),
    help_entry(b'S', "\tsave game", true),
    help_entry(b'Q', "\tquit", true),
    help_entry(b'!', "\tshell escape", true),
    help_entry(b'F', "<dir>\tfight till either of you dies", true),
    help_entry(b'v', "\tprint version number", true),
];

struct IdentItem {
    ch: u8,
    desc: &'static str,
}

static IDENT_ITEMS: &[IdentItem] = &[
    IdentItem {
        ch: b'|',
        desc: "wall of a room",
    },
    IdentItem {
        ch: b'-',
        desc: "wall of a room",
    },
    IdentItem {
        ch: b'*',
        desc: "gold",
    },
    IdentItem {
        ch: b'%',
        desc: "a staircase",
    },
    IdentItem {
        ch: b'+',
        desc: "door",
    },
    IdentItem {
        ch: b'.',
        desc: "room floor",
    },
    IdentItem {
        ch: b'@',
        desc: "you",
    },
    IdentItem {
        ch: b'#',
        desc: "passage",
    },
    IdentItem {
        ch: b'^',
        desc: "trap",
    },
    IdentItem {
        ch: b'!',
        desc: "potion",
    },
    IdentItem {
        ch: b'?',
        desc: "scroll",
    },
    IdentItem {
        ch: b':',
        desc: "food",
    },
    IdentItem {
        ch: b')',
        desc: "weapon",
    },
    IdentItem {
        ch: b' ',
        desc: "solid rock",
    },
    IdentItem {
        ch: b']',
        desc: "armor",
    },
    IdentItem {
        ch: b',',
        desc: "the Amulet of Yendor",
    },
    IdentItem {
        ch: b'=',
        desc: "ring",
    },
    IdentItem {
        ch: b'/',
        desc: "wand or staff",
    },
];

/// Gives help for one command, or displays the complete printable command list.
pub(crate) unsafe fn help() {
    msg_str("character you want help for (* for all): ");
    let helpch = readchar() as u8;
    mpos = 0;

    if helpch != b'*' {
        output::move_cursor(IVec2::new(0, 0));
        if let Some(entry) = HELP_ENTRIES.iter().find(|entry| entry.ch == helpch) {
            lower_msg = true as c_uchar;
            msg_str(&format!(
                "{}{}",
                output::format_key(entry.ch),
                entry.desc
            ));
            lower_msg = false as c_uchar;
        } else {
            msg_str(&format!(
                "unknown character '{}'",
                output::format_key(helpch)
            ));
        }
        return;
    }

    let mut numprint = HELP_ENTRIES.iter().filter(|entry| entry.print).count() as c_int;
    if numprint & 1 != 0 {
        numprint += 1;
    }
    numprint = (numprint / 2).min(GameConfig::SCREEN_LINES - 1);

    let help_window = Window::Stdscr;
    output::clear_window(help_window);
    for (count, entry) in HELP_ENTRIES
        .iter()
        .filter(|entry| entry.print)
        .take((numprint * 2) as usize)
        .enumerate()
    {
        let count = count as c_int;
        output::move_window_cursor(
            help_window,
            IVec2::new(
                if count >= numprint {
                    GameConfig::SCREEN_COLS / 2
                } else {
                    0
                },
                count % numprint,
            ),
        );
        if entry.ch != 0 {
            output::write_window_text(help_window, &output::format_key(entry.ch));
        }
        output::write_window_text(help_window, &entry.desc);
    }

    output::move_window_cursor(help_window, IVec2::new(0, GameConfig::SCREEN_LINES - 1));
    output::write_window_text(help_window, "--Press space to continue--");
    output::refresh_window(help_window);
    wait_for(' ');
    output::set_clear_on_refresh(Window::Stdscr, true);
    msg_str("");
    output::touch_window(Window::Stdscr);
    output::refresh_window(Window::Stdscr);
}

/// Describes a map glyph or monster letter selected by the player.
pub(crate) unsafe fn identify() {
    msg_str("what do you want identified? ");
    let ch = readchar();
    mpos = 0;
    if ch == ESCAPE {
        msg_str("");
        return;
    }

    let description = if (b'A' as c_int..=b'Z' as c_int).contains(&ch) {
        monsters[(ch - b'A' as c_int) as usize].m_name.to_owned()
    } else {
        IDENT_ITEMS
            .iter()
            .find(|item| item.ch as c_int == ch)
            .map_or("unknown character", |item| item.desc)
            .to_owned()
    };

    msg_str(&format!(
        "'{}': {}",
        output::format_key(ch as u8),
        description
    ));
}

#[cfg(test)]
mod tests {
    use super::{HELP_ENTRIES, IDENT_ITEMS};

    #[test]
    fn help_tables_preserve_expected_entries() {
        assert_eq!(HELP_ENTRIES.len(), 65);
        assert_eq!(HELP_ENTRIES.iter().filter(|entry| entry.print).count(), 49);
        assert_eq!(IDENT_ITEMS.len(), 18);

        let help = HELP_ENTRIES.iter().find(|entry| entry.ch == b'?').unwrap();
        assert_eq!(help.desc.as_bytes(), b"\tprints help");

        let amulet = IDENT_ITEMS.iter().find(|item| item.ch == b',').unwrap();
        assert_eq!(amulet.desc, "the Amulet of Yendor");
    }
}
