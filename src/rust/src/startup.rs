//! Process startup sequence, ported from `src/c/main.c`.

use std::ffi::{CStr, CString};
use std::io::Write;
use std::os::raw::{c_char, c_int, c_long, c_uchar, c_void};

use crate::io::msg_str;
use crate::player::{CCoord, CRoom, CThing, CThingMonster};
use crate::rnd::{rnd, set_seed};

const MAXSTR: usize = 1024;
const NUMLINES: c_int = 24;
const NUMCOLS: c_int = 80;
const AFTER: c_int = 2;
const WANDERTIME: c_int = 70;
const SEEMONST: i16 = 0o040000;

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;
const INV_CLEAR: c_int = 2;
const BUFSIZ: usize = 8192;
const SIGINT: c_int = 2;

/// Static buffer used by `leave()` to discard pending stdout output.
static mut LEAVE_BUF: [c_char; BUFSIZ] = [0; BUFSIZ];

unsafe extern "C" {
    static mut dnum: c_int;
    static mut file_name: [c_char; MAXSTR];
    static mut home: [c_char; MAXSTR];
    static mut LINES: c_int;
    static mut COLS: c_int;
    static mut hw: *mut c_void;
    static mut level: c_int;
    static master_mode_enabled: c_uchar;
    static mut noscore: c_int;
    static mut player: CThing;
    static mut purse: c_int;
    static mut seed: c_int;
    static mut whoami: [c_char; MAXSTR];
    static mut wizard: c_int;

    // ── Game-control globals from extern.c ────────────────────────────────
    static mut after: c_uchar;
    static mut count: c_int;
    static mut curscr: *mut c_void;
    static mut in_shell: c_uchar;
    static mut inv_type: c_int;
    static mut jump: c_uchar;
    static mut mpos: c_int;
    static mut oldpos: CCoord;
    static mut oldrp: *mut CRoom;
    static mut playing: c_uchar;
    static mut q_comm: c_uchar;
    static mut running: c_uchar;
    static mut see_floor: c_uchar;
    static mut stderr: *mut c_void;
    static mut stdout: *mut c_void;
    static mut terse: c_uchar;
    static mut to_death: c_uchar;

    fn death(monst: c_char);
    fn death_monst() -> c_char;
    fn doctor();
    fn fuse(func: *const c_void, arg: c_int, time: c_int, typ: c_int);
    fn getltchars();
    fn initscr() -> *mut c_void;
    fn endwin() -> c_int;
    fn idlok(window: *mut c_void, enabled: c_int) -> c_int;
    fn init_check();
    fn init_colors();
    fn init_materials();
    fn init_names();
    fn init_player();
    fn init_probs();
    fn init_stones();
    fn md_gethomedir() -> *mut c_char;
    fn md_getpid() -> c_int;
    fn md_getusername() -> *mut c_char;
    fn md_init();
    fn md_normaluser();
    fn new_level();
    fn newwin(lines: c_int, columns: c_int, begin_y: c_int, begin_x: c_int) -> *mut c_void;
    fn open_score();
    fn parse_opts(options: *mut c_char);
    fn restore(file: *mut c_char, envp: *mut *mut c_char) -> c_uchar;
    fn runners();
    fn score(amount: c_int, flags: c_int, monst: c_char);
    fn setup();
    fn start_daemon(func: *const c_void, arg: c_int, typ: c_int);
    fn stomach();
    fn strucpy(destination: *mut c_char, source: *mut c_char, length: c_int);
    fn swander();
    fn time(timer: *mut c_long) -> c_long;
    static mut stdscr: *mut c_void;

    // ── Terminal, curses, and machdep functions used by game control ──────
    fn baudrate() -> c_int;
    fn clear() -> c_int;
    fn clearok(win: *mut c_void, bf: c_uchar) -> c_int;
    fn clrtoeol() -> c_int;
    fn command();
    fn echo() -> c_int;
    fn exit(status: c_int) -> !;
    fn fflush(stream: *mut c_void) -> c_int;
    fn getcurx(win: *mut c_void) -> c_int;
    fn getcury(win: *mut c_void) -> c_int;
    fn isendwin() -> c_int;
    fn keypad(win: *mut c_void, bf: c_uchar) -> c_int;
    fn md_hasclreol() -> c_int;
    fn md_shellescape();
    fn md_tstpsignal();
    fn md_tstpresume();
    #[link_name = "move"]
    fn move_(y: c_int, x: c_int) -> c_int;
    fn mvaddstr(y: c_int, x: c_int, s: *const c_char) -> c_int;
    fn mvcur(y1: c_int, x1: c_int, y2: c_int, x2: c_int) -> c_int;
    fn mvprintw(y: c_int, x: c_int, fmt: *const c_char, ...) -> c_int;
    fn noecho() -> c_int;
    fn playltchars();
    fn printf(fmt: *const c_char, ...) -> c_int;
    fn putchar(c: c_int) -> c_int;
    fn raw() -> c_int;
    fn readchar() -> c_int;
    fn refresh() -> c_int;
    fn resetltchars();
    fn roomin(cp: *mut CCoord) -> *mut CRoom;
    fn setbuf(stream: *mut c_void, buf: *mut c_char);
    fn signal(sig: c_int, handler: usize) -> usize;
    fn status();
    fn wait_for(ch: c_int);
    fn wrefresh(win: *mut c_void) -> c_int;
}

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn arg_at(argv: *mut *mut c_char, index: usize) -> *mut c_char {
    *argv.add(index)
}

// ── Game control functions ported from src/c/main.c ─────────────────────────

/// endit:
/// Exit the program abnormally.
///
/// No globals used directly.
#[no_mangle]
pub unsafe extern "C" fn endit(sig: c_int) {
    let _ = sig;
    fatal(c"Okay, bye bye!\n".as_ptr() as *mut c_char);
}

/// fatal:
/// Exit the program, printing a message.
///
/// No globals used directly.
#[no_mangle]
pub unsafe extern "C" fn fatal(s: *mut c_char) {
    mvaddstr(LINES - 2, 0, s);
    refresh();
    endwin();
    my_exit(0);
}

/// roll:
/// Roll a number of dice.
///
/// No globals used directly (uses rnd()).
#[no_mangle]
pub unsafe extern "C" fn roll(mut number: c_int, sides: c_int) -> c_int {
    let mut dtotal = 0;

    while number > 0 {
        dtotal += rnd(sides) + 1;
        number -= 1;
    }
    dtotal
}

/// tstp:
/// Handle stop and start signals.
#[no_mangle]
pub unsafe extern "C" fn tstp(ignored: c_int) {
    let _ = ignored;

    /*
     * leave nicely
     */
    let oy = getcury(curscr);
    let ox = getcurx(curscr);
    mvcur(0, COLS - 1, LINES - 1, 0);
    endwin();
    resetltchars();
    fflush(stdout);
    md_tstpsignal();

    /*
     * start back up again
     */
    md_tstpresume();
    raw();
    noecho();
    keypad(stdscr, TRUE);
    playltchars();
    clearok(curscr, TRUE);
    wrefresh(curscr);
    let y = getcury(curscr);
    let x = getcurx(curscr);
    mvcur(y, x, oy, ox);
    move_(oy, ox);
    fflush(stdout);
}

/// playit:
/// The main loop of the program.  Loop until the game is over,
/// refreshing things and looking at the proper times.
///
/// Uses globals: terse, jump, see_floor, inv_type, oldpos, oldrp,
/// hero, playing, running.
#[no_mangle]
pub unsafe extern "C" fn playit() {
    /*
     * set up defaults for slow terminals
     */
    if baudrate() <= 1200 {
        terse = TRUE;
        jump = TRUE;
        see_floor = FALSE;
    }

    if md_hasclreol() != 0 {
        inv_type = INV_CLEAR;
    }

    /*
     * parse environment declaration of options
     */
    let c_options = std::env::var_os("ROGUEOPTS")
        .and_then(|value| CString::new(value.into_encoded_bytes()).ok());
    if let Some(options) = c_options.as_ref() {
        parse_opts(options.as_ptr() as *mut c_char);
    }

    oldpos = (*thing_t(&raw mut player)).t_pos;
    oldrp = roomin(&raw mut (*thing_t(&raw mut player)).t_pos);
    while playing != FALSE {
        command();              /* Command execution */
    }
    endit(0);
}

/// quit:
/// Have player make certain, then exit.
///
/// Uses globals: q_comm, mpos, purse, count, to_death.
#[no_mangle]
pub unsafe extern "C" fn quit(sig: c_int) {
    let _ = sig;

    /*
     * Reset the signal in case we got here via an interrupt
     */
    if q_comm == FALSE {
        mpos = 0;
    }
    let oy = getcury(curscr);
    let ox = getcurx(curscr);
    msg_str("really quit?");
    if readchar() == b'y' as c_int {
        signal(SIGINT, leave as usize);
        clear();
        mvprintw(
            LINES - 2,
            0,
            c"You quit with %d gold pieces".as_ptr(),
            purse,
        );
        move_(LINES - 1, 0);
        refresh();
        score(purse, 1, 0);
        my_exit(0);
    } else {
        move_(0, 0);
        clrtoeol();
        status();
        move_(oy, ox);
        refresh();
        mpos = 0;
        count = 0;
        to_death = FALSE;
    }
}

/// leave:
/// Leave quickly, but curteously.
#[no_mangle]
pub unsafe extern "C" fn leave(sig: c_int) {
    let _ = sig;

    setbuf(stdout, LEAVE_BUF.as_mut_ptr());   /* throw away pending output */

    if isendwin() == 0 {
        mvcur(0, COLS - 1, LINES - 1, 0);
        endwin();
    }

    putchar(b'\n' as c_int);
    my_exit(0);
}

/// shell:
/// Let them escape for a while.
///
/// Uses globals: in_shell, after.
#[no_mangle]
pub unsafe extern "C" fn shell() {
    /*
     * Set the terminal back to original mode
     */
    move_(LINES - 1, 0);
    refresh();
    endwin();
    resetltchars();
    putchar(b'\n' as c_int);
    in_shell = TRUE;
    after = FALSE;
    fflush(stdout);
    /*
     * Fork and do a shell
     */
    md_shellescape();

    printf(c"\n[Press return to continue]".as_ptr());
    fflush(stdout);
    noecho();
    raw();
    keypad(stdscr, TRUE);
    playltchars();
    in_shell = FALSE;
    wait_for(b'\n' as c_int);
    clearok(stdscr, TRUE);
}

/// my_exit:
/// Leave the process properly.
///
/// No globals used directly.
#[no_mangle]
pub unsafe extern "C" fn my_exit(st: c_int) -> ! {
    resetltchars();
    if !stdscr.is_null() {
        echo();
        endwin();
    }
    fflush(stdout);
    fflush(stderr);
    exit(st);
}

#[no_mangle]
pub unsafe extern "C" fn main(mut argc: c_int, mut argv: *mut *mut c_char, envp: *mut *mut c_char) -> c_int {
    md_init();

    if master_mode_enabled != 0 && argc >= 2 && *arg_at(argv, 1) == 0 {
        wizard = 1;
        player.t.t_flags |= SEEMONST;
        argv = argv.add(1);
        argc -= 1;
    }

    let home_dir = md_gethomedir();
    let home_len = CStr::from_ptr(home_dir).to_bytes_with_nul().len().min(MAXSTR);
    std::ptr::copy_nonoverlapping(home_dir, home.as_mut_ptr(), home_len);
    std::ptr::copy_nonoverlapping(home_dir, file_name.as_mut_ptr(), home_len);
    let save_name = b"rogue.save\0";
    let name_start = home_len.saturating_sub(1);
    std::ptr::copy_nonoverlapping(save_name.as_ptr() as *const c_char, file_name.as_mut_ptr().add(name_start), save_name.len());

    let options = std::env::var_os("ROGUEOPTS").and_then(|value| CString::new(value.into_encoded_bytes()).ok());
    if let Some(options) = options.as_ref() {
        parse_opts(options.as_ptr() as *mut c_char);
    }
    if options.is_none() || whoami[0] == 0 {
        let username = md_getusername();
        strucpy(whoami.as_mut_ptr(), username, CStr::from_ptr(username).to_bytes().len() as c_int);
    }

    let clock_seed = time(std::ptr::null_mut()) as c_int + md_getpid();
    dnum = if master_mode_enabled != 0 && wizard != 0 {
        std::env::var("SEED").ok().and_then(|value| value.parse().ok()).unwrap_or(clock_seed)
    } else {
        clock_seed
    };
    seed = dnum;
    set_seed(seed);
    open_score();
    md_normaluser();

    if argc == 2 {
        let argument = CStr::from_ptr(arg_at(argv, 1)).to_bytes();
        if argument == b"-s" {
            noscore = 1;
            score(0, -1, 0);
            return 0;
        }
        if argument == b"-d" {
            dnum = rnd(100);
            while dnum > 1 {
                dnum -= 1;
                rnd(100);
            }
            purse = rnd(100) + 1;
            level = rnd(100) + 1;
            initscr();
            getltchars();
            death(death_monst());
            return 0;
        }
    }

    init_check();
    if argc == 2 && restore(arg_at(argv, 1), envp) == 0 {
        my_exit(1);
    }

    if master_mode_enabled != 0 && wizard != 0 {
        print!("Hello {}, welcome to dungeon #{}", CStr::from_ptr(whoami.as_ptr()).to_string_lossy(), dnum);
    } else {
        print!("Hello {}, just a moment while I dig the dungeon...", CStr::from_ptr(whoami.as_ptr()).to_string_lossy());
    }
    std::io::stdout().flush().expect("failed to flush startup message");
    initscr();
    if LINES < NUMLINES || COLS < NUMCOLS {
        endwin();
        eprintln!("Sorry, the screen must be at least {}x{}", NUMLINES, NUMCOLS);
        eprintln!("Current terminal size: {}x{}", COLS, LINES);
        my_exit(1);
    }

    init_probs();
    init_player();
    init_names();
    init_colors();
    init_stones();
    init_materials();
    setup();
    hw = newwin(LINES, COLS, 0, 0);
    idlok(stdscr, 1);
    idlok(hw, 1);
    if master_mode_enabled != 0 {
        noscore = wizard;
    }
    new_level();
    start_daemon(runners as *const c_void, 0, AFTER);
    start_daemon(doctor as *const c_void, 0, AFTER);
    fuse(swander as *const c_void, 0, WANDERTIME, AFTER);
    start_daemon(stomach as *const c_void, 0, AFTER);
    playit();
    0
}