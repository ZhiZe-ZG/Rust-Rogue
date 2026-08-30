//! Process startup sequence, ported from `src/c/main.c`.

use std::ffi::{CStr, CString};
use std::io::Write;
use std::os::raw::{c_char, c_int, c_long, c_uchar, c_void};

use crate::player::CThing;

const MAXSTR: usize = 1024;
const NUMLINES: c_int = 24;
const NUMCOLS: c_int = 80;
const AFTER: c_int = 2;
const WANDERTIME: c_int = 70;
const SEEMONST: i16 = 0o040000;

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
    fn my_exit(status: c_int) -> !;
    fn new_level();
    fn newwin(lines: c_int, columns: c_int, begin_y: c_int, begin_x: c_int) -> *mut c_void;
    fn open_score();
    fn parse_opts(options: *mut c_char);
    fn playit();
    fn restore(file: *mut c_char, envp: *mut *mut c_char) -> c_uchar;
    fn rnd(range: c_int) -> c_int;
    fn runners();
    fn score(amount: c_int, flags: c_int, monst: c_char);
    fn set_seed(value: c_int);
    fn setup();
    fn start_daemon(func: *const c_void, arg: c_int, typ: c_int);
    fn stomach();
    fn strucpy(destination: *mut c_char, source: *mut c_char, length: c_int);
    fn swander();
    fn time(timer: *mut c_long) -> c_long;
    static mut stdscr: *mut c_void;
}

#[inline]
unsafe fn arg_at(argv: *mut *mut c_char, index: usize) -> *mut c_char {
    *argv.add(index)
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