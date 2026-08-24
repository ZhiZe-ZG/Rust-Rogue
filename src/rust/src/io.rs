use std::ffi::{CStr, VaList};
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

use crate::player::{CStats, CThing, CThingMonster};

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;
const ESCAPE: c_int = 27;
const NUMCOLS: c_int = 80;
const MAXSTR: usize = 1024;
const MAXMSG: usize = (NUMCOLS as usize) - 9;
const STATLINE: c_int = 23;

static mut msgbuf: [c_char; 2 * MAXMSG + 1] = [0; 2 * MAXMSG + 1];
static mut newpos: c_int = 0;

unsafe extern "C" {
    static mut cur_armor: *mut CThing;
    static mut hungry_state: c_int;
    static mut huh: [c_char; MAXSTR];
    static mut level: c_int;
    static mut max_stats: CStats;
    static mut mpos: c_int;
    static mut msg_esc: c_uchar;
    static mut player: CThing;
    static mut purse: c_int;
    static mut save_msg: c_uchar;
    static mut lower_msg: c_uchar;
    static mut stat_msg: c_uchar;
    static mut hw: *mut c_void;
    static mut stdscr: *mut c_void;

    fn clearok(win: *mut c_void, bf: c_uchar) -> c_int;
    fn clrtoeol() -> c_int;
    fn getcurx(win: *mut c_void) -> c_int;
    fn getcury(win: *mut c_void) -> c_int;
    fn isalpha(c: c_int) -> c_int;
    fn islower(c: c_int) -> c_int;
    fn look(wakeup: c_uchar);
    #[link_name = "move"]
    fn move_(y: c_int, x: c_int);
    fn mvaddstr(y: c_int, x: c_int, s: *const c_char) -> c_int;
    fn md_readchar() -> c_int;
    fn printw(fmt: *const c_char, ...) -> c_int;
    fn quit(status: c_int) -> c_int;
    fn refresh() -> c_int;
    fn strcat(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strlen(s: *const c_char) -> usize;
    fn touchwin(win: *mut c_void) -> c_int;
    fn toupper(c: c_int) -> c_int;
    fn vsnprintf(s: *mut c_char, n: usize, fmt: *const c_char, ap: VaList<'_>) -> c_int;
    fn wmove(win: *mut c_void, y: c_int, x: c_int) -> c_int;
    fn waddstr(win: *mut c_void, s: *const c_char) -> c_int;
    fn wrefresh(win: *mut c_void) -> c_int;
}

unsafe fn getyx_(win: *mut c_void, y: *mut c_int, x: *mut c_int) {
    *y = getcury(win);
    *x = getcurx(win);
}

unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn doadd(fmt: *const c_char, args: VaList<'_>) {
    if fmt.is_null() {
        return;
    }

    let mut buf: [c_char; MAXSTR] = [0; MAXSTR];
    let len = vsnprintf(buf.as_mut_ptr(), buf.len(), fmt, args);
    if len <= 0 {
        return;
    }

    let text = CStr::from_ptr(buf.as_ptr());
    if strlen(msgbuf.as_ptr()) + text.to_bytes().len() >= MAXMSG {
        endmsg();
    }

    strcat(msgbuf.as_mut_ptr(), text.as_ptr());
    newpos = strlen(msgbuf.as_ptr()) as c_int;
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn msg(fmt: *const c_char, mut ap: ...) -> c_int {
    if fmt.is_null() || *fmt == 0 {
        move_(0, 0);
        clrtoeol();
        mpos = 0;
        return !ESCAPE;
    }

    doadd(fmt, ap);
    endmsg()
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn addmsg(fmt: *const c_char, mut ap: ...) {
    doadd(fmt, ap);
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn endmsg() -> c_int {
    if save_msg != FALSE {
        strcpy(huh.as_mut_ptr(), msgbuf.as_ptr());
    }

    if mpos != 0 {
        look(FALSE);
        mvaddstr(0, mpos, c"--More--".as_ptr());
        refresh();

        if msg_esc == FALSE {
            wait_for(' ' as c_int);
        } else {
            loop {
                let ch = readchar();
                if ch == ' ' as c_int {
                    break;
                }
                if ch == ESCAPE {
                    msgbuf[0] = 0;
                    mpos = 0;
                    newpos = 0;
                    return ESCAPE;
                }
            }
        }
    }

    if islower(msgbuf[0] as c_int) != 0 && lower_msg == FALSE && msgbuf[1] != 0 {
        msgbuf[0] = toupper(msgbuf[0] as c_int) as c_char;
    }

    mvaddstr(0, 0, msgbuf.as_ptr());
    clrtoeol();
    mpos = newpos;
    newpos = 0;
    msgbuf[0] = 0;
    refresh();
    !ESCAPE
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn step_ok(ch: c_int) -> c_int {
    match ch as u8 {
        b' ' | b'|' | b'-' => FALSE as c_int,
        _ => if isalpha(ch) != 0 { 0 } else { 1 },
    }
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn readchar() -> c_int {
    let ch = md_readchar();
    if ch == 3 {
        quit(0);
        return ESCAPE;
    }
    ch
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn status() {
    let mut oy = 0;
    let mut ox = 0;
    let pstats = &mut (*thing_t(&raw mut player)).t_stats;
    let max_hp = pstats.s_maxhp;
    let mut temp = if !cur_armor.is_null() {
        (*cur_armor).o.o_arm
    } else {
        pstats.s_arm
    };

    static mut hpwidth: c_int = 0;
    static mut s_hungry: c_int = 0;
    static mut s_lvl: c_int = 0;
    static mut s_pur: c_int = -1;
    static mut s_hp: c_int = 0;
    static mut s_arm: c_int = 0;
    static mut s_str: c_uint = 0;
    static mut s_exp: c_int = 0;

    let state_name = [
        c"".as_ptr(),
        c"Hungry".as_ptr(),
        c"Weak".as_ptr(),
        c"Faint".as_ptr(),
    ];

    if s_hp == pstats.s_hpt
        && s_exp == pstats.s_exp
        && s_pur == purse
        && s_arm == temp
        && s_str == pstats.s_str
        && s_lvl == level
        && s_hungry == hungry_state
        && stat_msg == FALSE
    {
        return;
    }

    s_arm = temp;
    getyx_(stdscr, &mut oy, &mut ox);
    if s_hp != max_hp {
        let mut temp_hp = max_hp;
        s_hp = max_hp;
        hpwidth = 0;
        while temp_hp != 0 {
            hpwidth += 1;
            temp_hp /= 10;
        }
    }

    s_lvl = level;
    s_pur = purse;
    s_hp = pstats.s_hpt;
    s_str = pstats.s_str;
    s_exp = pstats.s_exp;
    s_hungry = hungry_state;

    if stat_msg != FALSE {
        move_(0, 0);
        msg(
            c"Level: %d  Gold: %-5d  Hp: %*d(%*d)  Str: %2d(%d)  Arm: %-2d  Exp: %d/%ld  %s".as_ptr(),
            level,
            purse,
            hpwidth,
            pstats.s_hpt,
            hpwidth,
            max_hp,
            pstats.s_str,
            max_stats.s_str,
            10 - s_arm,
            pstats.s_lvl,
            pstats.s_exp,
            state_name[hungry_state as usize],
        );
    } else {
        move_(STATLINE, 0);
        printw(
            c"Level: %d  Gold: %-5d  Hp: %*d(%*d)  Str: %2d(%d)  Arm: %-2d  Exp: %d/%d  %s".as_ptr(),
            level,
            purse,
            hpwidth,
            pstats.s_hpt,
            hpwidth,
            max_hp,
            pstats.s_str,
            max_stats.s_str,
            10 - s_arm,
            pstats.s_lvl,
            pstats.s_exp,
            state_name[hungry_state as usize],
        );
    }

    // C API uses non-variadic `clrtoeol()` and cursor restoration.
    // The project expects the cursor to remain where it was after the status line update.
    let _ = &oy;
    let _ = &ox;
    clrtoeol();
    move_(oy, ox);
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn wait_for(ch: c_int) {
    if ch == b'\n' as c_int {
        loop {
            let c = readchar();
            if c == b'\n' as c_int || c == b'\r' as c_int {
                break;
            }
        }
    } else {
        while readchar() != ch {
            // spin until the expected character arrives
        }
    }
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn show_win(message: *const c_char) {
    let win = hw;
    wmove(win, 0, 0);
    waddstr(win, message);
    touchwin(win);
    let hero = (*thing_t(&raw mut player)).t_pos;
    wmove(win, hero.y, hero.x);
    wrefresh(win);
    wait_for(' ' as c_int);
    clearok(stdscr, TRUE);
    touchwin(stdscr);
}
