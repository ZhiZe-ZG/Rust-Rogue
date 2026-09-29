//! Runtime option handling and the option screen.
//!
//! Ported from `src/c/options.c` to Rust.

use crate::draw::{erase_lamp, look};
use crate::ui::input::{self, readchar, wait_for};
use crate::ui::output;
use glam::IVec2;

const ESCAPE: i32 = 27;
const NORM: i32 = 0;
const QUIT: i32 = 1;
const MINUS: i32 = 2;
const MAXSTR: usize = 1024;
const MAXINP: usize = 50;
const INV_OVER: i32 = 0;
const INV_SLOW: i32 = 1;
const INV_CLEAR: i32 = 2;
/// Number of `inv_t_name` entries (the inventory display styles).
const INV_T_NAME_LEN: usize = 3;

/// Identifies which owned string an option edits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StrTarget {
    None,
    Name,
    Fruit,
    File,
}

/// Identifies a boolean global an option edits.
#[derive(Clone, Copy)]
enum BoolFlag {
    Terse,
    FightFlush,
    Jump,
    SeeFloor,
    Passgo,
    Tombstone,
}

impl BoolFlag {
    /// The current value of the flag.
    #[inline]
    unsafe fn get(self) -> bool {
        match self {
            BoolFlag::Terse => terse != 0,
            BoolFlag::FightFlush => fight_flush != 0,
            BoolFlag::Jump => jump != 0,
            BoolFlag::SeeFloor => see_floor != 0,
            BoolFlag::Passgo => passgo != 0,
            BoolFlag::Tombstone => tombstone != 0,
        }
    }

    /// Set the flag.
    #[inline]
    unsafe fn set(self, on: bool) {
        let value = on as u8;
        match self {
            BoolFlag::Terse => terse = value,
            BoolFlag::FightFlush => fight_flush = value,
            BoolFlag::Jump => jump = value,
            BoolFlag::SeeFloor => see_floor = value,
            BoolFlag::Passgo => passgo = value,
            BoolFlag::Tombstone => tombstone = value,
        }
    }
}

/// Which backing global an option edits, replacing the former type-erased
/// `*mut u8`/`*mut i32`. Each variant names the global, so the option screen
/// reads and writes it through typed accessors instead of a raw pointer.
#[derive(Clone, Copy)]
enum OptTarget {
    /// No backing global (string options edit via [`StrTarget`]).
    None,
    /// A boolean flag.
    Bool(BoolFlag),
    /// The inventory display style (`inv_type`).
    InvType,
}

impl OptTarget {
    /// The backing boolean flag, or `None` when this target is not boolean.
    #[inline]
    fn bool_flag(self) -> Option<BoolFlag> {
        match self {
            OptTarget::Bool(flag) => Some(flag),
            _ => None,
        }
    }
}

/// One configurable option: its prompt, the global it edits, and the
/// callbacks that render and read it on the option screen.
pub struct OPTION {
    o_name: &'static str,
    o_prompt: &'static str,
    o_target: OptTarget,
    o_str: StrTarget,
    o_putfunc: unsafe fn(&OPTION),
    o_getfunc: unsafe fn(&OPTION) -> i32,
}

use crate::game::globals::{
    after, fight_flush, inv_type, jump, mpos, passgo, see_floor, terse, tombstone,
};

unsafe fn hero_pos() -> IVec2 {
    crate::game::PLAYER.pos()
}

unsafe fn proom_ptr() -> Option<usize> {
    crate::game::PLAYER.room()
}

unsafe fn option_list() -> [OPTION; 10] {
    [
        OPTION {
            o_name: "terse",
            o_prompt: "Terse output",
            o_target: OptTarget::Bool(BoolFlag::Terse),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "flush",
            o_prompt: "Flush typeahead during battle",
            o_target: OptTarget::Bool(BoolFlag::FightFlush),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "jump",
            o_prompt: "Show position only at end of run",
            o_target: OptTarget::Bool(BoolFlag::Jump),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "seefloor",
            o_prompt: "Show the lamp-illuminated floor",
            o_target: OptTarget::Bool(BoolFlag::SeeFloor),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_sf,
        },
        OPTION {
            o_name: "passgo",
            o_prompt: "Follow turnings in passageways",
            o_target: OptTarget::Bool(BoolFlag::Passgo),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "tombstone",
            o_prompt: "Print out tombstone when killed",
            o_target: OptTarget::Bool(BoolFlag::Tombstone),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "inven",
            o_prompt: "Inventory style",
            o_target: OptTarget::InvType,
            o_str: StrTarget::None,
            o_putfunc: put_inv_t,
            o_getfunc: get_inv_t,
        },
        OPTION {
            o_name: "name",
            o_prompt: "Name",
            o_target: OptTarget::None,
            o_str: StrTarget::Name,
            o_putfunc: put_str,
            o_getfunc: get_str,
        },
        OPTION {
            o_name: "fruit",
            o_prompt: "Fruit",
            o_target: OptTarget::None,
            o_str: StrTarget::Fruit,
            o_putfunc: put_str,
            o_getfunc: get_str,
        },
        OPTION {
            o_name: "file",
            o_prompt: "Save file",
            o_target: OptTarget::None,
            o_str: StrTarget::File,
            o_putfunc: put_str,
            o_getfunc: get_str,
        },
    ]
}

unsafe fn paint(s: &str) {
    output::write_window_text(s);
}

unsafe fn str_target_value(target: StrTarget) -> String {
    match target {
        StrTarget::Name => crate::game::globals::whoami(),
        StrTarget::Fruit => crate::game::globals::fruit(),
        StrTarget::File => crate::game::globals::file_name(),
        StrTarget::None => String::new(),
    }
}

unsafe fn set_str_target(target: StrTarget, value: String) {
    match target {
        StrTarget::Name => crate::game::globals::set_whoami(value),
        StrTarget::Fruit => crate::game::globals::set_fruit(value),
        StrTarget::File => crate::game::globals::set_file_name(value),
        StrTarget::None => {}
    }
}

unsafe fn pr_optname_slot(op: &OPTION) {
    let out = format!("{} (\"{}\"): ", op.o_prompt, op.o_name);
    paint(&out);
}

pub unsafe fn option() {
    let mut optlist = option_list();
    let mut retval: i32;

    output::clear_window();
    for item in optlist.iter() {
        pr_optname_slot(item);
        (item.o_putfunc)(item);
        output::write_window_glyph('\n');
    }

    output::move_window_cursor(IVec2::new(0, 0));
    for index in 0..optlist.len() {
        let item = &optlist[index];
        pr_optname_slot(item);
        retval = (item.o_getfunc)(item);
        if retval == QUIT {
            break;
        }
        if retval == MINUS && index > 0 {
            output::move_window_cursor(IVec2::new(0, (index as i32) - 1));
            let prev = index as isize - 2;
            if prev >= 0 {
                let _ = prev;
            }
        }
    }

    output::move_window_cursor(IVec2::new(0, 23));
    paint("--Press space to continue--");
    output::refresh_window();
    wait_for(' ');
    output::set_clear_on_refresh(true);
    output::touch_window();
    after = false as u8;
}

unsafe fn put_bool(op: &OPTION) {
    let on = match op.o_target.bool_flag() {
        Some(flag) => flag.get(),
        None => false,
    };
    output::write_window_text(if on { "True" } else { "False" });
}

unsafe fn put_str(op: &OPTION) {
    let text = str_target_value(op.o_str);
    output::write_window_text(&text);
}

unsafe fn put_inv_t(_op: &OPTION) {
    let idx = inv_type as usize;
    if idx < INV_T_NAME_LEN {
        output::write_window_text(&crate::game::globals::inv_t_name(idx));
    }
}

unsafe fn get_bool(op: &OPTION) -> i32 {
    let Some(flag) = op.o_target.bool_flag() else {
        return NORM;
    };
    let mut bad = true;

    let origin = output::window_cursor();
    output::write_window_text(if flag.get() { "True" } else { "False" });
    while bad {
        output::move_window_cursor(origin);
        output::refresh_window();
        match readchar() {
            ch if ch == 't' as i32 || ch == 'T' as i32 => {
                flag.set(true);
                bad = false;
            }
            ch if ch == 'f' as i32 || ch == 'F' as i32 => {
                flag.set(false);
                bad = false;
            }
            ch if ch == '\n' as i32 || ch == '\r' as i32 => {
                bad = false;
            }
            ESCAPE => return QUIT,
            ch if ch == '-' as i32 => return MINUS,
            _ => {
                output::move_window_cursor(IVec2::new(origin.x + 10, origin.y));
                output::write_window_text("(T or F)");
            }
        }
    }
    output::move_window_cursor(origin);
    output::write_window_text(if flag.get() { "True" } else { "False" });
    output::write_window_glyph('\n');
    NORM
}

unsafe fn get_sf(op: &OPTION) -> i32 {
    let Some(flag) = op.o_target.bool_flag() else {
        return NORM;
    };
    let was_sf = flag.get();
    let retval = get_bool(op);
    if retval == QUIT {
        return QUIT;
    }
    if was_sf != flag.get() {
        if !flag.get() {
            let hero = hero_pos();
            see_floor = true as u8;
            erase_lamp(hero, proom_ptr());
            see_floor = false as u8;
        } else {
            look(false as u8);
        }
    }
    NORM
}

/// Reads a line of text starting from `initial`. Returns the
/// edited text on success, or `None` if the user pressed ESCAPE.
pub unsafe fn read_line(initial: &str) -> Option<String> {
    let mut buf: Vec<u8> = initial.as_bytes().to_vec();

    let origin = output::window_cursor();
    output::refresh_window();
    let mut c: i32;
    loop {
        c = readchar();
        if c == '\n' as i32 || c == '\r' as i32 || c == ESCAPE {
            break;
        }
        if c == -1 {
            continue;
        }
        if c == input::erase_key() as i32 {
            buf.pop();
            continue;
        }
        if c == input::kill_key() as i32 {
            buf.clear();
            output::move_window_cursor(origin);
            continue;
        }
        let printable = c as u8;
        if buf.len() >= MAXINP || !(printable.is_ascii_graphic() || printable == b' ') {
            continue;
        }
        buf.push(printable);
        output::write_window_text(&output::format_key(printable));
    }

    let text = String::from_utf8_lossy(&buf).into_owned();

    let out = format!("{}\n", text);
    output::move_window_cursor(origin);
    paint(&out);
    output::refresh_window();
    mpos += buf.len() as i32;

    if c == ESCAPE {
        None
    } else {
        Some(text)
    }
}

/// Read a line of text and store it in the option's
/// target. Returns the legacy `get_str` status code.
pub unsafe fn get_str(op: &OPTION) -> i32 {
    let initial = str_target_value(op.o_str);
    match read_line(&initial) {
        None => QUIT,
        Some(text) => {
            set_str_target(op.o_str, text);
            NORM
        }
    }
}

unsafe fn get_inv_t(_op: &OPTION) -> i32 {
    let mut bad = true;

    let origin = output::window_cursor();
    if inv_type >= 0 && inv_type < INV_T_NAME_LEN as i32 {
        output::write_window_text(&crate::game::globals::inv_t_name(inv_type as usize));
    }
    while bad {
        output::move_window_cursor(origin);
        output::refresh_window();
        match readchar() {
            ch if ch == 'o' as i32 || ch == 'O' as i32 => {
                inv_type = INV_OVER;
                bad = false;
            }
            ch if ch == 's' as i32 || ch == 'S' as i32 => {
                inv_type = INV_SLOW;
                bad = false;
            }
            ch if ch == 'c' as i32 || ch == 'C' as i32 => {
                inv_type = INV_CLEAR;
                bad = false;
            }
            ch if ch == '\n' as i32 || ch == '\r' as i32 => {
                bad = false;
            }
            ESCAPE => return QUIT,
            ch if ch == '-' as i32 => return MINUS,
            _ => {
                output::move_window_cursor(IVec2::new(origin.x + 15, origin.y));
                output::write_window_text("(O, S, or C)");
            }
        }
    }
    if inv_type >= 0 && inv_type < INV_T_NAME_LEN as i32 {
        let name = crate::game::globals::inv_t_name(inv_type as usize);
        let out = format!("{}\n", name);
        output::move_window_cursor(origin);
        paint(&out);
    }
    NORM
}

/// Parse a `ROGUEOPTS`-style string, applying each recognised option. The
/// string is processed as Rust `&str`; no C string calls are used.
pub unsafe fn parse_opts(s: &str) {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    let option_list = option_list();

    while i < bytes.len() {
        // Skip to the next alphabetic character.
        while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let name = &s[start..i];

        let mut matched = false;
        for op in option_list.iter() {
            if op.o_name == name {
                if op.o_str == StrTarget::None && op.o_name != "inven" {
                    // Boolean option: set it true.
                    if let OptTarget::Bool(flag) = op.o_target {
                        flag.set(true);
                    }
                } else if op.o_name == "inven" {
                    // Inventory style: skip '=' then a single letter.
                    while i < bytes.len() && bytes[i] == b'=' {
                        i += 1;
                    }
                    if i < bytes.len() {
                        let mut letter = bytes[i];
                        i += 1;
                        if letter.is_ascii_lowercase() {
                            letter = letter.to_ascii_uppercase();
                        }
                        // Advance to the end of the value token.
                        while i < bytes.len() && bytes[i] != b',' {
                            i += 1;
                        }
                        let start_idx = i.saturating_sub(1);
                        let value = &s[start_idx..start_idx + 1];
                        for idx in 0..INV_T_NAME_LEN {
                            if value
                                == &crate::game::globals::inv_t_name(idx)
                                    [..1.min(crate::game::globals::inv_t_name(idx).len())]
                            {
                                inv_type = idx as i32;
                                break;
                            }
                        }
                        let _ = letter;
                    }
                } else {
                    // String option: skip '=' then copy until ','.
                    while i < bytes.len() && bytes[i] == b'=' {
                        i += 1;
                    }
                    let mut value_start = i;
                    // `~` expands to the home directory.
                    let mut prefix = String::new();
                    if i < bytes.len() && bytes[i] == b'~' {
                        prefix = crate::game::globals::get_home();
                        value_start = i + 1;
                    }
                    let mut end = value_start;
                    while end < bytes.len() && bytes[end] != b',' {
                        end += 1;
                    }
                    let raw = &s[value_start..end];
                    let filtered: String = raw
                        .chars()
                        .filter(|ch| ch.is_ascii_graphic() || *ch == ' ')
                        .collect();
                    let new_value = format!("{}{}", prefix, filtered);
                    set_str_target(op.o_str, new_value);
                    i = end;
                }
                matched = true;
                break;
            }
        }

        if !matched {
            // Skip this unrecognised word's value.
            while i < bytes.len() && bytes[i] != b',' {
                i += 1;
            }
        }
        // Skip the trailing comma.
        if i < bytes.len() && bytes[i] == b',' {
            i += 1;
        }
    }
}

/// Filter `src` down to printable characters and spaces, capped at `MAXINP`.
pub fn filter_printable(src: &str) -> String {
    src.chars()
        .take(MAXINP)
        .filter(|ch| ch.is_ascii_graphic() || *ch == ' ')
        .collect()
}
