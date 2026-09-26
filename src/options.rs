//! Runtime option handling and the option screen.
//!
//! Ported from `src/c/options.c` to Rust.

use crate::draw::{erase_lamp, look};
use crate::entity::player::{Thing, ThingMonster};
use crate::ui::input::{self, readchar, wait_for};
use crate::ui::{output, Window};
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

/// Typed target for an option's backing global, replacing the former
/// type-erased `*mut u8`. Each variant carries a raw pointer to the exact
/// global it edits, so the option screen never has to guess the pointee type.
#[derive(Clone, Copy)]
enum OptTarget {
    /// No backing global (string options edit via [`StrTarget`]).
    None,
    /// A boolean flag stored in a global `u8`.
    Bool(*mut u8),
    /// The inventory display style, stored in a global `i32`.
    InvType(*mut i32),
}

impl OptTarget {
    /// The backing `u8` flag, or null when this target is not boolean.
    #[inline]
    fn bool_ptr(self) -> *mut u8 {
        match self {
            OptTarget::Bool(p) => p,
            _ => std::ptr::null_mut(),
        }
    }

    /// The backing `i32`, or null when this target is not the inventory style.
    #[inline]
    fn int_ptr(self) -> *mut i32 {
        match self {
            OptTarget::InvType(p) => p,
            _ => std::ptr::null_mut(),
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
    o_getfunc: unsafe fn(&OPTION, Window) -> i32,
}

use crate::globals::{after, fight_flush, inv_type, jump, mpos, passgo, see_floor, terse, tombstone};


unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

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
            o_target: OptTarget::Bool(&raw mut terse),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "flush",
            o_prompt: "Flush typeahead during battle",
            o_target: OptTarget::Bool(&raw mut fight_flush),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "jump",
            o_prompt: "Show position only at end of run",
            o_target: OptTarget::Bool(&raw mut jump),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "seefloor",
            o_prompt: "Show the lamp-illuminated floor",
            o_target: OptTarget::Bool(&raw mut see_floor),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_sf,
        },
        OPTION {
            o_name: "passgo",
            o_prompt: "Follow turnings in passageways",
            o_target: OptTarget::Bool(&raw mut passgo),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "tombstone",
            o_prompt: "Print out tombstone when killed",
            o_target: OptTarget::Bool(&raw mut tombstone),
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "inven",
            o_prompt: "Inventory style",
            o_target: OptTarget::InvType(&raw mut inv_type),
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

unsafe fn paint(win: Window, s: &str) {
    output::write_window_text(win, s);
}

unsafe fn str_target_value(target: StrTarget) -> String {
    match target {
        StrTarget::Name => crate::globals::whoami(),
        StrTarget::Fruit => crate::globals::fruit(),
        StrTarget::File => crate::globals::file_name(),
        StrTarget::None => String::new(),
    }
}

unsafe fn set_str_target(target: StrTarget, value: String) {
    match target {
        StrTarget::Name => crate::globals::set_whoami(value),
        StrTarget::Fruit => crate::globals::set_fruit(value),
        StrTarget::File => crate::globals::set_file_name(value),
        StrTarget::None => {}
    }
}

unsafe fn pr_optname_slot(op: &OPTION) {
    let out = format!("{} (\"{}\"): ", op.o_prompt, op.o_name);
    paint(Window::Stdscr, &out);
}

pub unsafe fn option() {
    let mut optlist = option_list();
    let mut retval: i32;

    let options_window = Window::Stdscr;
    output::clear_window(options_window);
    for item in optlist.iter() {
        pr_optname_slot(item);
        (item.o_putfunc)(item);
        output::write_window_glyph(options_window, '\n');
    }

    output::move_window_cursor(options_window, IVec2::new(0, 0));
    for index in 0..optlist.len() {
        let item = &optlist[index];
        pr_optname_slot(item);
        retval = (item.o_getfunc)(item, Window::Stdscr);
        if retval == QUIT {
            break;
        }
        if retval == MINUS && index > 0 {
            output::move_window_cursor(options_window, IVec2::new(0, (index as i32) - 1));
            let prev = index as isize - 2;
            if prev >= 0 {
                let _ = prev;
            }
        }
    }

    output::move_window_cursor(options_window, IVec2::new(0, 23));
    paint(Window::Stdscr, "--Press space to continue--");
    output::refresh_window(options_window);
    wait_for(' ');
    output::set_clear_on_refresh(Window::Stdscr, true);
    output::touch_window(Window::Stdscr);
    after = false as u8;
}

unsafe fn pr_optname(op: *mut OPTION) {
    if op.is_null() {
        return;
    }
    pr_optname_slot(&*op);
}

unsafe fn put_bool(op: &OPTION) {
    let bp = op.o_target.bool_ptr();
    output::write_window_text(Window::Stdscr, if *bp != 0 { "True" } else { "False" });
}

unsafe fn put_str(op: &OPTION) {
    let text = str_target_value(op.o_str);
    output::write_window_text(Window::Stdscr, &text);
}

unsafe fn put_inv_t(op: &OPTION) {
    let ip = op.o_target.int_ptr();
    let idx = *ip as usize;
    if idx < INV_T_NAME_LEN {
        output::write_window_text(Window::Stdscr, &crate::globals::inv_t_name(idx));
    }
}

unsafe fn get_bool(op: &OPTION, win: Window) -> i32 {
    let bp = op.o_target.bool_ptr();
    let mut bad = true;

    let origin = output::window_cursor(win);
    output::write_window_text(win, if *bp != 0 { "True" } else { "False" });
    while bad {
        output::move_window_cursor(win, origin);
        output::refresh_window(win);
        match readchar() {
            ch if ch == 't' as i32 || ch == 'T' as i32 => {
                *bp = true as u8;
                bad = false;
            }
            ch if ch == 'f' as i32 || ch == 'F' as i32 => {
                *bp = false as u8;
                bad = false;
            }
            ch if ch == '\n' as i32 || ch == '\r' as i32 => {
                bad = false;
            }
            ESCAPE => return QUIT,
            ch if ch == '-' as i32 => return MINUS,
            _ => {
                output::move_window_cursor(win, IVec2::new(origin.x + 10, origin.y));
                output::write_window_text(win, "(T or F)");
            }
        }
    }
    output::move_window_cursor(win, origin);
    output::write_window_text(win, if *bp != 0 { "True" } else { "False" });
    output::write_window_glyph(win, '\n');
    NORM
}

unsafe fn get_sf(op: &OPTION, win: Window) -> i32 {
    let bp = op.o_target.bool_ptr();
    let was_sf = *bp != 0;
    let retval = get_bool(op, win);
    if retval == QUIT {
        return QUIT;
    }
    if was_sf != (*bp != 0) {
        if *bp == 0 {
            let mut hero = hero_pos();
            see_floor = true as u8;
            erase_lamp(&mut hero, proom_ptr());
            see_floor = false as u8;
        } else {
            look(false as u8);
        }
    }
    NORM
}

/// Reads a line of text into `win`, starting from `initial`. Returns the
/// edited text on success, or `None` if the user pressed ESCAPE.
pub unsafe fn read_line(initial: &str, win: Window) -> Option<String> {
    let mut buf: Vec<u8> = initial.as_bytes().to_vec();

    let origin = output::window_cursor(win);
    output::refresh_window(win);
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
            output::move_window_cursor(win, origin);
            continue;
        }
        let printable = c as u8;
        if buf.len() >= MAXINP || !(printable.is_ascii_graphic() || printable == b' ') {
            continue;
        }
        buf.push(printable);
        output::write_window_text(win, &output::format_key(printable));
    }

    let text = String::from_utf8_lossy(&buf).into_owned();

    let out = format!("{}\n", text);
    output::move_window_cursor(win, origin);
    paint(win, &out);
    output::refresh_window(win);
    if win == Window::Stdscr {
        mpos += buf.len() as i32;
    }

    if c == ESCAPE {
        None
    } else {
        Some(text)
    }
}

/// Read a line of text, editing within `win`, and store it in the option's
/// target. Returns the legacy `get_str` status code.
pub unsafe fn get_str(op: &OPTION, win: Window) -> i32 {
    let initial = str_target_value(op.o_str);
    match read_line(&initial, win) {
        None => QUIT,
        Some(text) => {
            set_str_target(op.o_str, text);
            NORM
        }
    }
}

unsafe fn get_inv_t(op: &OPTION, win: Window) -> i32 {
    let ip = op.o_target.int_ptr();
    let mut bad = true;

    let origin = output::window_cursor(win);
    if *ip >= 0 && *ip < INV_T_NAME_LEN as i32 {
        output::write_window_text(win, &crate::globals::inv_t_name(*ip as usize));
    }
    while bad {
        output::move_window_cursor(win, origin);
        output::refresh_window(win);
        match readchar() {
            ch if ch == 'o' as i32 || ch == 'O' as i32 => {
                *ip = INV_OVER;
                bad = false;
            }
            ch if ch == 's' as i32 || ch == 'S' as i32 => {
                *ip = INV_SLOW;
                bad = false;
            }
            ch if ch == 'c' as i32 || ch == 'C' as i32 => {
                *ip = INV_CLEAR;
                bad = false;
            }
            ch if ch == '\n' as i32 || ch == '\r' as i32 => {
                bad = false;
            }
            ESCAPE => return QUIT,
            ch if ch == '-' as i32 => return MINUS,
            _ => {
                output::move_window_cursor(win, IVec2::new(origin.x + 15, origin.y));
                output::write_window_text(win, "(O, S, or C)");
            }
        }
    }
    if *ip >= 0 && *ip < INV_T_NAME_LEN as i32 {
        let name = crate::globals::inv_t_name(*ip as usize);
        let out = format!("{}\n", name);
        output::move_window_cursor(win, origin);
        paint(win, &out);
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
                    let bp = op.o_target.bool_ptr();
                    *bp = true as u8;
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
                            if value == &crate::globals::inv_t_name(idx)[..1.min(crate::globals::inv_t_name(idx).len())] {
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
                        prefix = crate::globals::get_home();
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