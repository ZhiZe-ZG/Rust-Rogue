use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

use crate::player::{CCoord, CRoom, CThing, CThingMonster};

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;
const ESCAPE: c_int = 27;
const NORM: c_int = 0;
const QUIT: c_int = 1;
const MINUS: c_int = 2;
const MAXSTR: usize = 1024;
const MAXINP: usize = 50;
const INV_OVER: c_int = 0;
const INV_SLOW: c_int = 1;
const INV_CLEAR: c_int = 2;

#[repr(C)]
pub struct OPTION {
    o_name: *mut c_char,
    o_prompt: *mut c_char,
    o_opt: *mut c_void,
    o_putfunc: unsafe extern "C" fn(*mut c_void),
    o_getfunc: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
}

unsafe extern "C" {
    static mut after: c_uchar;
    static mut file_name: [c_char; MAXSTR];
    static mut fight_flush: c_uchar;
    static mut fruit: [c_char; MAXSTR];
    static mut home: [c_char; MAXSTR];
    static mut hw: *mut c_void;
    static mut inv_t_name: [*mut c_char; 3];
    static mut inv_type: c_int;
    static mut jump: c_uchar;
    static mut mpos: c_int;
    static mut passgo: c_uchar;
    static mut player: CThing;
    static mut see_floor: c_uchar;
    static mut stdscr: *mut c_void;
    static mut terse: c_uchar;
    static mut tombstone: c_uchar;
    static mut whoami: [c_char; MAXSTR];

    fn clearok(win: *mut c_void, bf: c_uchar) -> c_int;
    fn erase_lamp(pos: *mut CCoord, rp: *mut CRoom);
    fn erasechar() -> c_int;
    fn getcurx(win: *mut c_void) -> c_int;
    fn getcury(win: *mut c_void) -> c_int;
    fn isalpha(c: c_int) -> c_int;
    fn isprint(c: c_int) -> c_int;
    fn killchar() -> c_int;
    fn look(wakeup: c_uchar);
    fn readchar() -> c_int;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int;
    fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int;
    fn strlen(s: *const c_char) -> usize;
    fn touchwin(win: *mut c_void) -> c_int;
    fn toupper(c: c_int) -> c_int;
    fn unctrl(c: c_int) -> *mut c_char;
    fn wait_for(ch: c_int);
    fn wclear(win: *mut c_void) -> c_int;
    fn waddch(win: *mut c_void, ch: c_uint) -> c_int;
    fn waddstr(win: *mut c_void, s: *const c_char) -> c_int;
    fn wmove(win: *mut c_void, y: c_int, x: c_int) -> c_int;
    fn wrefresh(win: *mut c_void) -> c_int;
}

unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

unsafe fn hero_pos() -> CCoord {
    (*thing_t(&raw mut player)).t_pos
}

unsafe fn proom_ptr() -> *mut CRoom {
    (*thing_t(&raw mut player)).t_room
}

unsafe fn getyx_(win: *mut c_void, y: *mut c_int, x: *mut c_int) {
    *y = getcury(win);
    *x = getcurx(win);
}

unsafe fn option_list() -> [OPTION; 10] {
    [
        OPTION { o_name: c"terse".as_ptr() as *mut c_char, o_prompt: c"Terse output".as_ptr() as *mut c_char, o_opt: (&raw mut terse) as *mut c_void, o_putfunc: put_bool, o_getfunc: get_bool },
        OPTION { o_name: c"flush".as_ptr() as *mut c_char, o_prompt: c"Flush typeahead during battle".as_ptr() as *mut c_char, o_opt: (&raw mut fight_flush) as *mut c_void, o_putfunc: put_bool, o_getfunc: get_bool },
        OPTION { o_name: c"jump".as_ptr() as *mut c_char, o_prompt: c"Show position only at end of run".as_ptr() as *mut c_char, o_opt: (&raw mut jump) as *mut c_void, o_putfunc: put_bool, o_getfunc: get_bool },
        OPTION { o_name: c"seefloor".as_ptr() as *mut c_char, o_prompt: c"Show the lamp-illuminated floor".as_ptr() as *mut c_char, o_opt: (&raw mut see_floor) as *mut c_void, o_putfunc: put_bool, o_getfunc: get_sf },
        OPTION { o_name: c"passgo".as_ptr() as *mut c_char, o_prompt: c"Follow turnings in passageways".as_ptr() as *mut c_char, o_opt: (&raw mut passgo) as *mut c_void, o_putfunc: put_bool, o_getfunc: get_bool },
        OPTION { o_name: c"tombstone".as_ptr() as *mut c_char, o_prompt: c"Print out tombstone when killed".as_ptr() as *mut c_char, o_opt: (&raw mut tombstone) as *mut c_void, o_putfunc: put_bool, o_getfunc: get_bool },
        OPTION { o_name: c"inven".as_ptr() as *mut c_char, o_prompt: c"Inventory style".as_ptr() as *mut c_char, o_opt: (&raw mut inv_type) as *mut c_void, o_putfunc: put_inv_t, o_getfunc: get_inv_t },
        OPTION { o_name: c"name".as_ptr() as *mut c_char, o_prompt: c"Name".as_ptr() as *mut c_char, o_opt: (&raw mut whoami) as *mut c_void, o_putfunc: put_str, o_getfunc: get_str },
        OPTION { o_name: c"fruit".as_ptr() as *mut c_char, o_prompt: c"Fruit".as_ptr() as *mut c_char, o_opt: (&raw mut fruit) as *mut c_void, o_putfunc: put_str, o_getfunc: get_str },
        OPTION { o_name: c"file".as_ptr() as *mut c_char, o_prompt: c"Save file".as_ptr() as *mut c_char, o_opt: (&raw mut file_name) as *mut c_void, o_putfunc: put_str, o_getfunc: get_str },
    ]
}

unsafe fn paint(win: *mut c_void, s: &str) {
    let c = CString::new(s).unwrap();
    waddstr(win, c.as_ptr());
}

unsafe fn pr_optname_slot(op: &OPTION) {
    let prompt = CStr::from_ptr(op.o_prompt).to_string_lossy();
    let name = CStr::from_ptr(op.o_name).to_string_lossy();
    let out = format!("{} (\"{}\"): ", prompt, name);
    paint(hw, &out);
}

#[no_mangle]
pub unsafe extern "C" fn option() {
    let mut optlist = option_list();
    let mut retval: c_int;

    wclear(hw);
    for item in &mut optlist {
        pr_optname_slot(item);
        (item.o_putfunc)(item.o_opt);
        waddch(hw, '\n' as c_uint);
    }

    wmove(hw, 0, 0);
    for index in 0..optlist.len() {
        let item = &mut optlist[index];
        pr_optname_slot(item);
        retval = (item.o_getfunc)(item.o_opt, hw);
        if retval == QUIT {
            break;
        }
        if retval == MINUS && index > 0 {
            wmove(hw, (index as c_int) - 1, 0);
            let prev = index as isize - 2;
            if prev >= 0 {
                let _ = prev;
            }
        }
    }

    wmove(hw, 23, 0);
    paint(hw, "--Press space to continue--");
    wrefresh(hw);
    wait_for(' ' as c_int);
    clearok(stdscr, TRUE);
    touchwin(stdscr);
    after = FALSE;
}

#[no_mangle]
pub unsafe extern "C" fn pr_optname(op: *mut OPTION) {
    if op.is_null() {
        return;
    }
    pr_optname_slot(&*op);
}

#[no_mangle]
pub unsafe extern "C" fn put_bool(vp: *mut c_void) {
    let bp = vp as *mut c_uchar;
    let text = if *bp != 0 { c"True".as_ptr() } else { c"False".as_ptr() };
    waddstr(hw, text);
}

#[no_mangle]
pub unsafe extern "C" fn put_str(vp: *mut c_void) {
    let sp = vp as *mut c_char;
    waddstr(hw, sp);
}

#[no_mangle]
pub unsafe extern "C" fn put_inv_t(vp: *mut c_void) {
    let ip = vp as *mut c_int;
    let idx = *ip as usize;
    if idx < unsafe { inv_t_name.len() } {
        waddstr(hw, inv_t_name[idx]);
    }
}

#[no_mangle]
pub unsafe extern "C" fn get_bool(vp: *mut c_void, win: *mut c_void) -> c_int {
    let bp = vp as *mut c_uchar;
    let mut oy = 0;
    let mut ox = 0;
    let mut bad = true;

    getyx_(win, &mut oy, &mut ox);
    waddstr(win, if *bp != 0 { c"True".as_ptr() } else { c"False".as_ptr() });
    while bad {
        wmove(win, oy, ox);
        wrefresh(win);
        match readchar() {
            ch if ch == 't' as c_int || ch == 'T' as c_int => {
                *bp = TRUE;
                bad = false;
            }
            ch if ch == 'f' as c_int || ch == 'F' as c_int => {
                *bp = FALSE;
                bad = false;
            }
            ch if ch == '\n' as c_int || ch == '\r' as c_int => {
                bad = false;
            }
            ESCAPE => return QUIT,
            ch if ch == '-' as c_int => return MINUS,
            _ => {
                wmove(win, oy, ox + 10);
                waddstr(win, c"(T or F)".as_ptr());
            }
        }
    }
    wmove(win, oy, ox);
    waddstr(win, if *bp != 0 { c"True".as_ptr() } else { c"False".as_ptr() });
    waddch(win, '\n' as c_uint);
    NORM
}

#[no_mangle]
pub unsafe extern "C" fn get_sf(vp: *mut c_void, win: *mut c_void) -> c_int {
    let bp = vp as *mut c_uchar;
    let was_sf = *bp != 0;
    let retval = get_bool(vp, win);
    if retval == QUIT {
        return QUIT;
    }
    if was_sf != (*bp != 0) {
        if *bp == 0 {
            let mut hero = hero_pos();
            see_floor = TRUE;
            erase_lamp(&mut hero, proom_ptr());
            see_floor = FALSE;
        } else {
            look(FALSE);
        }
    }
    NORM
}

#[no_mangle]
pub unsafe extern "C" fn get_str(vopt: *mut c_void, win: *mut c_void) -> c_int {
    let opt = vopt as *mut c_char;
    let mut buf = [0 as c_char; MAXINP];
    let mut ptr = buf.as_mut_ptr();
    let mut oy = 0;
    let mut ox = 0;
    let mut c: c_int;

    getyx_(win, &mut oy, &mut ox);
    wrefresh(win);
    loop {
        c = readchar();
        if c == '\n' as c_int || c == '\r' as c_int || c == ESCAPE {
            break;
        }
        if c == -1 {
            continue;
        }
        if c == erasechar() {
            if ptr > buf.as_mut_ptr() {
                ptr = ptr.sub(1);
            }
            continue;
        }
        if c == killchar() {
            ptr = buf.as_mut_ptr();
            wmove(win, oy, ox);
            continue;
        }
        if ptr >= buf.as_mut_ptr().add(MAXINP) || !(isprint(c) != 0 || c == ' ' as c_int) {
            continue;
        }
        *ptr = c as c_char;
        ptr = ptr.add(1);
        waddstr(win, unctrl(c));
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

    let msg = if opt.is_null() { String::new() } else { CStr::from_ptr(opt).to_string_lossy().to_string() };
    let out = format!("{}\n", msg);
    wmove(win, oy, ox);
    paint(win, &out);
    wrefresh(win);
    if win == stdscr {
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

#[no_mangle]
pub unsafe extern "C" fn get_inv_t(vp: *mut c_void, win: *mut c_void) -> c_int {
    let ip = vp as *mut c_int;
    let mut oy = 0;
    let mut ox = 0;
    let mut bad = true;

    getyx_(win, &mut oy, &mut ox);
    if *ip >= 0 && *ip < inv_t_name.len() as c_int {
        waddstr(win, inv_t_name[*ip as usize]);
    }
    while bad {
        wmove(win, oy, ox);
        wrefresh(win);
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
                wmove(win, oy, ox + 15);
                waddstr(win, c"(O, S, or C)".as_ptr());
            }
        }
    }
    if *ip >= 0 && *ip < inv_t_name.len() as c_int {
        let name = CStr::from_ptr(inv_t_name[*ip as usize]).to_string_lossy();
        let out = format!("{}\n", name);
        wmove(win, oy, ox);
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
                    *bp = TRUE;
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
                        if !tmp.is_null() && isalpha(*tmp as c_int) != 0 && *tmp as u8 >= b'a' && *tmp as u8 <= b'z' {
                            *tmp = toupper(*tmp as c_int) as c_char;
                        }
                        for i in 0..inv_t_name.len() {
                            if !value.is_null() && !end.is_null() && strncmp(value, inv_t_name[i], (end as usize - value as usize)) == 0 {
                                inv_type = i as c_int;
                                break;
                            }
                        }
                    } else {
                        let limit = if end.is_null() { 0 } else { end as usize - value as usize };
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
