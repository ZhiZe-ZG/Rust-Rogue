//! Runtime option handling and the option screen.
//!
//! Ported from `src/c/options.c` to Rust.
use std::os::raw::{c_int, c_uchar, c_void, c_void as c_void_t};

use crate::draw::{erase_lamp, look};
use crate::entity::player::{Thing, ThingMonster};
use crate::ui::input::{self, readchar, wait_for};
use crate::ui::{output, Window};
use glam::IVec2;

const ESCAPE: c_int = 27;
const NORM: c_int = 0;
const QUIT: c_int = 1;
const MINUS: c_int = 2;
const MAXSTR: usize = 1024;
const MAXINP: usize = 50;
const INV_OVER: c_int = 0;
const INV_SLOW: c_int = 1;
const INV_CLEAR: c_int = 2;
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

/// One configurable option: its prompt, the global it edits, and the
/// callbacks that render and read it on the option screen.
pub struct OPTION {
    o_name: &'static str,
    o_prompt: &'static str,
    o_opt: *mut c_void,
    o_str: StrTarget,
    o_putfunc: unsafe fn(&OPTION),
    o_getfunc: unsafe fn(&OPTION, Window) -> c_int,
}

unsafe extern "C" {
    static mut after: c_uchar;
    static mut fight_flush: c_uchar;
    static mut inv_type: c_int;
    static mut jump: c_uchar;
    static mut mpos: c_int;
    static mut passgo: c_uchar;
    static mut see_floor: c_uchar;
    static mut terse: c_uchar;
    static mut tombstone: c_uchar;
}

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
            o_opt: (&raw mut terse) as *mut c_void,
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "flush",
            o_prompt: "Flush typeahead during battle",
            o_opt: (&raw mut fight_flush) as *mut c_void,
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "jump",
            o_prompt: "Show position only at end of run",
            o_opt: (&raw mut jump) as *mut c_void,
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "seefloor",
            o_prompt: "Show the lamp-illuminated floor",
            o_opt: (&raw mut see_floor) as *mut c_void,
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_sf,
        },
        OPTION {
            o_name: "passgo",
            o_prompt: "Follow turnings in passageways",
            o_opt: (&raw mut passgo) as *mut c_void,
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "tombstone",
            o_prompt: "Print out tombstone when killed",
            o_opt: (&raw mut tombstone) as *mut c_void,
            o_str: StrTarget::None,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: "inven",
            o_prompt: "Inventory style",
            o_opt: (&raw mut inv_type) as *mut c_void,
            o_str: StrTarget::None,
            o_putfunc: put_inv_t,
            o_getfunc: get_inv_t,
        },
        OPTION {
            o_name: "name",
            o_prompt: "Name",
            o_opt: std::ptr::null_mut(),
            o_str: StrTarget::Name,
            o_putfunc: put_str,
            o_getfunc: get_str,
        },
        OPTION {
            o_name: "fruit",
            o_prompt: "Fruit",
            o_opt: std::ptr::null_mut(),
            o_str: StrTarget::Fruit,
            o_putfunc: put_str,
            o_getfunc: get_str,
        },
        OPTION {
            o_name: "file",
            o_prompt: "Save file",
            o_opt: std::ptr::null_mut(),
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

#[no_mangle]
pub unsafe extern "C" fn option() {
    let mut optlist = option_list();
    let mut retval: c_int;

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
    after = false as c_uchar;
}

unsafe fn pr_optname(op: *mut OPTION) {
    if op.is_null() {
        return;
    }
    pr_optname_slot(&*op);
}

unsafe fn put_bool(op: &OPTION) {
    let bp = op.o_opt as *mut c_uchar;
    output::write_window_text(Window::Stdscr, if *bp != 0 { "True" } else { "False" });
}

unsafe fn put_str(op: &OPTION) {
    let text = str_target_value(op.o_str);
    output::write_window_text(Window::Stdscr, &text);
}

unsafe fn put_inv_t(op: &OPTION) {
    let ip = op.o_opt as *mut c_int;
    let idx = *ip as usize;
    if idx < INV_T_NAME_LEN {
        output::write_window_text(Window::Stdscr, &crate::globals::inv_t_name(idx));
    }
}

unsafe fn get_bool(op: &OPTION, win: Window) -> c_int {
    let bp = op.o_opt as *mut c_uchar;
    let mut bad = true;

    let origin = output::window_cursor(win);
    output::write_window_text(win, if *bp != 0 { "True" } else { "False" });
    while bad {
        output::move_window_cursor(win, origin);
        output::refresh_window(win);
        match readchar() {
            ch if ch == 't' as c_int || ch == 'T' as c_int => {
                *bp = true as c_uchar;
                bad = false;
            }
            ch if ch == 'f' as c_int || ch == 'F' as c_int => {
                *bp = false as c_uchar;
                bad = false;
            }
            ch if ch == '\n' as c_int || ch == '\r' as c_int => {
                bad = false;
            }
            ESCAPE => return QUIT,
            ch if ch == '-' as c_int => return MINUS,
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

unsafe fn get_sf(op: &OPTION, win: Window) -> c_int {
    let bp = op.o_opt as *mut c_uchar;
    let was_sf = *bp != 0;
    let retval = get_bool(op, win);
    if retval == QUIT {
        return QUIT;
    }
    if was_sf != (*bp != 0) {
        if *bp == 0 {
            let mut hero = hero_pos();
            see_floor = true as c_uchar;
            erase_lamp(&mut hero, proom_ptr());
            see_floor = false as c_uchar;
        } else {
            look(false as c_uchar);
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
    let mut c: c_int;
    loop {
        c = readchar();
        if c == '\n' as c_int || c == '\r' as c_int || c == ESCAPE {
            break;
        }
        if c == -1 {
            continue;
        }
        if c == input::erase_key() as c_int {
            buf.pop();
            continue;
        }
        if c == input::kill_key() as c_int {
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
        mpos += buf.len() as c_int;
    }

    if c == ESCAPE {
        None
    } else {
        Some(text)
    }
}

/// Read a line of text, editing within `win`, and store it in the option's
/// target. Returns the legacy `get_str` status code.
pub unsafe fn get_str(op: &OPTION, win: Window) -> c_int {
    let initial = str_target_value(op.o_str);
    match read_line(&initial, win) {
        None => QUIT,
        Some(text) => {
            set_str_target(op.o_str, text);
            NORM
        }
    }
}

unsafe fn get_inv_t(op: &OPTION, win: Window) -> c_int {
    let ip = op.o_opt as *mut c_int;
    let mut bad = true;

    let origin = output::window_cursor(win);
    if *ip >= 0 && *ip < INV_T_NAME_LEN as c_int {
        output::write_window_text(win, &crate::globals::inv_t_name(*ip as usize));
    }
    while bad {
        output::move_window_cursor(win, origin);
        output::refresh_window(win);
        match readchar() {
            ch if ch == 'o' as c_int || ch == 'O' as c_int => {
                *ip = INV_OVER;
                bad = false;
            }
            ch if ch == 's' as c_int || ch == 'S' as c_int => {
                *ip = INV_SLOW;
                bad = false;
            }
            ch if ch == 'c' as c_int || ch == 'C' as c_int => {
                *ip = INV_CLEAR;
                bad = false;
            }
            ch if ch == '\n' as c_int || ch == '\r' as c_int => {
                bad = false;
            }
            ESCAPE => return QUIT,
            ch if ch == '-' as c_int => return MINUS,
            _ => {
                output::move_window_cursor(win, IVec2::new(origin.x + 15, origin.y));
                output::write_window_text(win, "(O, S, or C)");
            }
        }
    }
    if *ip >= 0 && *ip < INV_T_NAME_LEN as c_int {
        let name = crate::globals::inv_t_name(*ip as usize);
        let out = format!("{}\n", name);
        output::move_window_cursor(win, origin);
        paint(win, &out);
    }
    NORM
}

/// Parse a `ROGUEOPTS`-style string, applying each recognised option. The
/// string is processed as Rust `&str`; no C string calls are used.
#[no_mangle]
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
                    let bp = op.o_opt as *mut c_uchar;
                    *bp = true as c_uchar;
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
                                inv_type = idx as c_int;
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

// Silence the unused alias import warning while keeping the type name handy.
#[allow(dead_code)]
type Void = c_void_t;