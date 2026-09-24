//! Runtime option handling and the option screen.
//!
//! Ported from `src/c/options.c` to Rust.
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

use crate::draw::{erase_lamp, look};
use crate::entity::player::{CThing, CThingMonster};
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

/// One configurable option: its prompt, the global it edits, and the
/// callbacks that render and read it on the option screen.
pub struct OPTION {
    o_name: *mut c_char,
    o_prompt: *mut c_char,
    o_opt: *mut c_void,
    o_putfunc: unsafe fn(*mut c_void),
    o_getfunc: unsafe fn(*mut c_void, Window) -> c_int,
}

unsafe extern "C" {
    static mut after: c_uchar;
    static mut file_name: [c_char; MAXSTR];
    static mut fight_flush: c_uchar;
    static mut fruit: [c_char; MAXSTR];
    static mut home: [c_char; MAXSTR];
    static mut inv_t_name: [*mut c_char; 3];
    static mut inv_type: c_int;
    static mut jump: c_uchar;
    static mut mpos: c_int;
    static mut passgo: c_uchar;
    static mut see_floor: c_uchar;
    static mut terse: c_uchar;
    static mut tombstone: c_uchar;
    static mut whoami: [c_char; MAXSTR];

    fn isalpha(c: c_int) -> c_int;
    fn isprint(c: c_int) -> c_int;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int;
    fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int;
    fn strlen(s: *const c_char) -> usize;
    fn toupper(c: c_int) -> c_int;
}

unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    crate::entity::player::thing_t(tp)
}

unsafe fn hero_pos() -> IVec2 {
    (*thing_t(crate::game::player_ptr())).t_pos
}

unsafe fn proom_ptr() -> Option<usize> {
    (*thing_t(crate::game::player_ptr())).t_room
}

unsafe fn option_list() -> [OPTION; 10] {
    [
        OPTION {
            o_name: c"terse".as_ptr() as *mut c_char,
            o_prompt: c"Terse output".as_ptr() as *mut c_char,
            o_opt: (&raw mut terse) as *mut c_void,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: c"flush".as_ptr() as *mut c_char,
            o_prompt: c"Flush typeahead during battle".as_ptr() as *mut c_char,
            o_opt: (&raw mut fight_flush) as *mut c_void,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: c"jump".as_ptr() as *mut c_char,
            o_prompt: c"Show position only at end of run".as_ptr() as *mut c_char,
            o_opt: (&raw mut jump) as *mut c_void,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: c"seefloor".as_ptr() as *mut c_char,
            o_prompt: c"Show the lamp-illuminated floor".as_ptr() as *mut c_char,
            o_opt: (&raw mut see_floor) as *mut c_void,
            o_putfunc: put_bool,
            o_getfunc: get_sf,
        },
        OPTION {
            o_name: c"passgo".as_ptr() as *mut c_char,
            o_prompt: c"Follow turnings in passageways".as_ptr() as *mut c_char,
            o_opt: (&raw mut passgo) as *mut c_void,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: c"tombstone".as_ptr() as *mut c_char,
            o_prompt: c"Print out tombstone when killed".as_ptr() as *mut c_char,
            o_opt: (&raw mut tombstone) as *mut c_void,
            o_putfunc: put_bool,
            o_getfunc: get_bool,
        },
        OPTION {
            o_name: c"inven".as_ptr() as *mut c_char,
            o_prompt: c"Inventory style".as_ptr() as *mut c_char,
            o_opt: (&raw mut inv_type) as *mut c_void,
            o_putfunc: put_inv_t,
            o_getfunc: get_inv_t,
        },
        OPTION {
            o_name: c"name".as_ptr() as *mut c_char,
            o_prompt: c"Name".as_ptr() as *mut c_char,
            o_opt: (&raw mut whoami) as *mut c_void,
            o_putfunc: put_str,
            o_getfunc: get_str,
        },
        OPTION {
            o_name: c"fruit".as_ptr() as *mut c_char,
            o_prompt: c"Fruit".as_ptr() as *mut c_char,
            o_opt: (&raw mut fruit) as *mut c_void,
            o_putfunc: put_str,
            o_getfunc: get_str,
        },
        OPTION {
            o_name: c"file".as_ptr() as *mut c_char,
            o_prompt: c"Save file".as_ptr() as *mut c_char,
            o_opt: (&raw mut file_name) as *mut c_void,
            o_putfunc: put_str,
            o_getfunc: get_str,
        },
    ]
}

unsafe fn paint(win: Window, s: &str) {
    output::write_window_text(win, s);
}

unsafe fn pr_optname_slot(op: &OPTION) {
    let prompt = CStr::from_ptr(op.o_prompt).to_string_lossy();
    let name = CStr::from_ptr(op.o_name).to_string_lossy();
    let out = format!("{} (\"{}\"): ", prompt, name);
    paint(Window::Stdscr, &out);
}

#[no_mangle]
pub unsafe extern "C" fn option() {
    let mut optlist = option_list();
    let mut retval: c_int;

    let options_window = Window::Stdscr;
    output::clear_window(options_window);
    for item in &mut optlist {
        pr_optname_slot(item);
        (item.o_putfunc)(item.o_opt);
        output::write_window_glyph(options_window, '\n');
    }

    output::move_window_cursor(options_window, IVec2::new(0, 0));
    for index in 0..optlist.len() {
        let item = &mut optlist[index];
        pr_optname_slot(item);
        retval = (item.o_getfunc)(item.o_opt, Window::Stdscr);
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

unsafe fn put_bool(vp: *mut c_void) {
    let bp = vp as *mut c_uchar;
    output::write_window_text(Window::Stdscr, if *bp != 0 { "True" } else { "False" });
}

unsafe fn put_str(vp: *mut c_void) {
    let sp = vp as *mut c_char;
    output::write_window_text(Window::Stdscr, &CStr::from_ptr(sp).to_string_lossy());
}

unsafe fn put_inv_t(vp: *mut c_void) {
    let ip = vp as *mut c_int;
    let idx = *ip as usize;
    if idx < unsafe { inv_t_name.len() } {
        output::write_window_text(
            Window::Stdscr,
            &CStr::from_ptr(inv_t_name[idx]).to_string_lossy(),
        );
    }
}

unsafe fn get_bool(vp: *mut c_void, win: Window) -> c_int {
    let bp = vp as *mut c_uchar;
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

unsafe fn get_sf(vp: *mut c_void, win: Window) -> c_int {
    let bp = vp as *mut c_uchar;
    let was_sf = *bp != 0;
    let retval = get_bool(vp, win);
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

/// Read a line of text into `vopt`, editing within `win`.
pub unsafe fn get_str(vopt: *mut c_void, win: Window) -> c_int {
    let opt = vopt as *mut c_char;
    let mut buf = [0 as c_char; MAXINP];
    let mut ptr = buf.as_mut_ptr();
    let mut c: c_int;

    let origin = output::window_cursor(win);
    output::refresh_window(win);
    loop {
        c = readchar();
        if c == '\n' as c_int || c == '\r' as c_int || c == ESCAPE {
            break;
        }
        if c == -1 {
            continue;
        }
        if c == input::erase_key() as c_int {
            if ptr > buf.as_mut_ptr() {
                ptr = ptr.sub(1);
            }
            continue;
        }
        if c == input::kill_key() as c_int {
            ptr = buf.as_mut_ptr();
            output::move_window_cursor(win, origin);
            continue;
        }
        if ptr >= buf.as_mut_ptr().add(MAXINP) || !(isprint(c) != 0 || c == ' ' as c_int) {
            continue;
        }
        *ptr = c as c_char;
        ptr = ptr.add(1);
        output::write_window_text(win, &output::format_key(c as u8));
    }

    *ptr = 0;
    if ptr > buf.as_mut_ptr() {
        let len = (ptr as usize - buf.as_ptr() as usize) as c_int;
        let mut tmp = [0 as c_char; MAXSTR];
        for i in 0..len as usize {
            tmp[i] = buf[i];
        }
        tmp[len as usize] = 0;
        std::ptr::copy_nonoverlapping(tmp.as_ptr(), opt, len as usize + 1);
    }

    let msg = if opt.is_null() {
        String::new()
    } else {
        CStr::from_ptr(opt).to_string_lossy().to_string()
    };
    let out = format!("{}\n", msg);
    output::move_window_cursor(win, origin);
    paint(win, &out);
    output::refresh_window(win);
    if win == Window::Stdscr {
        mpos += (ptr as usize - buf.as_ptr() as usize) as c_int;
    }
    if c == '-' as c_int {
        return MINUS;
    }
    if c == ESCAPE {
        return QUIT;
    }
    NORM
}

unsafe fn get_inv_t(vp: *mut c_void, win: Window) -> c_int {
    let ip = vp as *mut c_int;
    let mut bad = true;

    let origin = output::window_cursor(win);
    if *ip >= 0 && *ip < inv_t_name.len() as c_int {
        output::write_window_text(
            win,
            &CStr::from_ptr(inv_t_name[*ip as usize]).to_string_lossy(),
        );
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
    if *ip >= 0 && *ip < inv_t_name.len() as c_int {
        let name = CStr::from_ptr(inv_t_name[*ip as usize]).to_string_lossy();
        let out = format!("{}\n", name);
        output::move_window_cursor(win, origin);
        paint(win, &out);
    }
    NORM
}

#[no_mangle]
pub unsafe extern "C" fn parse_opts(str: *mut c_char) {
    let mut current = str;
    while !current.is_null() && *current != 0 {
        let mut p = current;
        while !p.is_null() && *p != 0 && isalpha(*p as c_int) == 0 {
            p = p.add(1);
        }
        if p.is_null() || *p == 0 {
            break;
        }
        let start = p;
        while !p.is_null() && *p != 0 && isalpha(*p as c_int) != 0 {
            p = p.add(1);
        }
        let len = p as usize - start as usize;

        let mut matched = false;
        for op in option_list().iter() {
            let name = CStr::from_ptr(op.o_name).to_bytes();
            if len == name.len() && strncmp(start, op.o_name, len) == 0 {
                if op.o_putfunc == put_bool {
                    let bp = op.o_opt as *mut c_uchar;
                    *bp = true as c_uchar;
                } else {
                    let mut value = p;
                    while !value.is_null() && *value == '=' as c_char {
                        value = value.add(1);
                    }
                    let start_ptr = if !value.is_null() && *value == '~' as c_char {
                        strcpy(op.o_opt as *mut c_char, home.as_ptr());
                        (op.o_opt as *mut c_char).add(strlen(home.as_ptr()))
                    } else {
                        op.o_opt as *mut c_char
                    };
                    let mut end = value;
                    while !end.is_null() && *end != 0 && *end != ',' as c_char {
                        end = end.add(1);
                    }
                    if op.o_putfunc == put_inv_t {
                        let mut tmp = value;
                        if !tmp.is_null()
                            && isalpha(*tmp as c_int) != 0
                            && *tmp as u8 >= b'a'
                            && *tmp as u8 <= b'z'
                        {
                            *tmp = toupper(*tmp as c_int) as c_char;
                        }
                        for i in 0..inv_t_name.len() {
                            if !value.is_null()
                                && !end.is_null()
                                && strncmp(value, inv_t_name[i], (end as usize - value as usize))
                                    == 0
                            {
                                inv_type = i as c_int;
                                break;
                            }
                        }
                    } else {
                        let limit = if end.is_null() {
                            0
                        } else {
                            end as usize - value as usize
                        };
                        if limit > 0 {
                            strucpy(start_ptr, value, limit as c_int);
                        }
                    }
                }
                matched = true;
                break;
            }
        }

        if !matched {
            while !p.is_null() && *p != 0 && !isalpha(*p as c_int) != 0 {
                p = p.add(1);
            }
            current = p;
            continue;
        }

        while !p.is_null() && *p != 0 && *p != ',' as c_char {
            p = p.add(1);
        }
        if !p.is_null() && *p == ',' as c_char {
            p = p.add(1);
        }
        current = p;
    }
}

#[no_mangle]
pub unsafe extern "C" fn strucpy(s1: *mut c_char, s2: *const c_char, len: c_int) {
    let mut remaining = len.max(0) as usize;
    if remaining > MAXINP {
        remaining = MAXINP;
    }
    let mut dst = s1;
    let mut src = s2;
    for _ in 0..remaining {
        if src.is_null() || *src == 0 {
            break;
        }
        if isprint(*src as c_int) != 0 || *src == ' ' as c_char {
            *dst = *src;
            dst = dst.add(1);
        }
        src = src.add(1);
    }
    *dst = 0;
}
