//! Command help and map-symbol identification.

use std::ffi::CStr;
use std::os::raw::{c_int, c_uchar, c_void};

use crate::globals::{hw, lower_msg, monsters, mpos};
use crate::ui::input::{readchar, wait_for};
use crate::ui::output::msg_str;
use crate::ui::{output, Position, Window};

const ESCAPE: c_int = 27;

struct HelpEntry {
    ch: u8,
    desc: &'static CStr,
    print: bool,
}

const fn help_entry(ch: u8, desc: &'static CStr, print: bool) -> HelpEntry {
    HelpEntry { ch, desc, print }
}

static HELP_ENTRIES: &[HelpEntry] = &[
    help_entry(b'?', c"\tprints help", true),
    help_entry(b'/', c"\tidentify object", true),
    help_entry(b'h', c"\tleft", true),
    help_entry(b'j', c"\tdown", true),
    help_entry(b'k', c"\tup", true),
    help_entry(b'l', c"\tright", true),
    help_entry(b'y', c"\tup & left", true),
    help_entry(b'u', c"\tup & right", true),
    help_entry(b'b', c"\tdown & left", true),
    help_entry(b'n', c"\tdown & right", true),
    help_entry(b'H', c"\trun left", false),
    help_entry(b'J', c"\trun down", false),
    help_entry(b'K', c"\trun up", false),
    help_entry(b'L', c"\trun right", false),
    help_entry(b'Y', c"\trun up & left", false),
    help_entry(b'U', c"\trun up & right", false),
    help_entry(b'B', c"\trun down & left", false),
    help_entry(b'N', c"\trun down & right", false),
    help_entry(0x08, c"\trun left until adjacent", false),
    help_entry(0x0a, c"\trun down until adjacent", false),
    help_entry(0x0b, c"\trun up until adjacent", false),
    help_entry(0x0c, c"\trun right until adjacent", false),
    help_entry(0x19, c"\trun up & left until adjacent", false),
    help_entry(0x15, c"\trun up & right until adjacent", false),
    help_entry(0x02, c"\trun down & left until adjacent", false),
    help_entry(0x16, c"\trun down & right until adjacent", false),
    help_entry(0, c"\t<SHIFT><dir>: run that way", true),
    help_entry(0, c"\t<CTRL><dir>: run till adjacent", true),
    help_entry(b'f', c"<dir>\tfight till death or near death", true),
    help_entry(b't', c"<dir>\tthrow something", true),
    help_entry(b'm', c"<dir>\tmove onto without picking up", true),
    help_entry(b'z', c"<dir>\tzap a wand in a direction", true),
    help_entry(b'^', c"<dir>\tidentify trap type", true),
    help_entry(b's', c"\tsearch for trap/secret door", true),
    help_entry(b'>', c"\tgo down a staircase", true),
    help_entry(b'<', c"\tgo up a staircase", true),
    help_entry(b'.', c"\trest for a turn", true),
    help_entry(b',', c"\tpick something up", true),
    help_entry(b'i', c"\tinventory", true),
    help_entry(b'I', c"\tinventory single item", true),
    help_entry(b'q', c"\tquaff potion", true),
    help_entry(b'r', c"\tread scroll", true),
    help_entry(b'e', c"\teat food", true),
    help_entry(b'w', c"\twield a weapon", true),
    help_entry(b'W', c"\twear armor", true),
    help_entry(b'T', c"\ttake armor off", true),
    help_entry(b'P', c"\tput on ring", true),
    help_entry(b'R', c"\tremove ring", true),
    help_entry(b'd', c"\tdrop object", true),
    help_entry(b'c', c"\tcall object", true),
    help_entry(b'a', c"\trepeat last command", true),
    help_entry(b')', c"\tprint current weapon", true),
    help_entry(b']', c"\tprint current armor", true),
    help_entry(b'=', c"\tprint current rings", true),
    help_entry(b'@', c"\tprint current stats", true),
    help_entry(b'D', c"\trecall what's been discovered", true),
    help_entry(b'o', c"\texamine/set options", true),
    help_entry(0x12, c"\tredraw screen", true),
    help_entry(0x10, c"\trepeat last message", true),
    help_entry(0x1b, c"\tcancel command", true),
    help_entry(b'S', c"\tsave game", true),
    help_entry(b'Q', c"\tquit", true),
    help_entry(b'!', c"\tshell escape", true),
    help_entry(b'F', c"<dir>\tfight till either of you dies", true),
    help_entry(b'v', c"\tprint version number", true),
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

unsafe extern "C" {
    static mut LINES: c_int;
    static mut COLS: c_int;
    static mut stdscr: *mut c_void;
}

/// Gives help for one command, or displays the complete printable command list.
pub(crate) unsafe fn help() {
    msg_str("character you want help for (* for all): ");
    let helpch = readchar() as u8;
    mpos = 0;

    if helpch != b'*' {
        output::move_cursor(Position::new(0, 0));
        if let Some(entry) = HELP_ENTRIES.iter().find(|entry| entry.ch == helpch) {
            lower_msg = true as c_uchar;
            msg_str(&format!(
                "{}{}",
                output::format_key(entry.ch),
                entry.desc.to_string_lossy()
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
    numprint = (numprint / 2).min(LINES - 1);

    let help_window = Window::from_raw(hw);
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
            Position::new(
                count % numprint,
                if count >= numprint { COLS / 2 } else { 0 },
            ),
        );
        if entry.ch != 0 {
            output::write_window_text(help_window, &output::format_key(entry.ch));
        }
        output::write_window_text(help_window, &entry.desc.to_string_lossy());
    }

    output::move_window_cursor(help_window, Position::new(LINES - 1, 0));
    output::write_window_text(help_window, "--Press space to continue--");
    output::refresh_window(help_window);
    wait_for(b' ' as c_int);
    let standard_screen = Window::from_raw(stdscr);
    output::set_clear_on_refresh(standard_screen, true);
    msg_str("");
    output::touch_window(standard_screen);
    output::refresh_window(standard_screen);
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
        CStr::from_ptr(monsters[(ch - b'A' as c_int) as usize].m_name)
            .to_string_lossy()
            .into_owned()
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
        assert_eq!(help.desc.to_bytes(), b"\tprints help");

        let amulet = IDENT_ITEMS.iter().find(|item| item.ch == b',').unwrap();
        assert_eq!(amulet.desc, "the Amulet of Yendor");
    }
}
