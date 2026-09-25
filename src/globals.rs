//! Global game data shared across the ported C modules.
//!
//! Mirrors the process-wide storage the original C code declared in
//! `extern.c` and `init.c`: material tables, monster state, and other globals.
use crate::config::GameConfig;
use crate::entity::player::{
    Thing as PlayerCThing,
};
use crate::entity::stats::Stats;
use glam::IVec2;
use std::os::raw::{c_char, c_int, c_uchar, c_uint};

const MAXSTR: usize = 1024;

const fn fill_c_string<const N: usize>(s: &str) -> [c_char; N] {
    let bytes = s.as_bytes();
    let mut out = [0 as c_char; N];
    let mut i = 0usize;
    while i < bytes.len() && i < N {
        out[i] = bytes[i] as c_char;
        i += 1;
    }
    out
}

const fn dmg_string(s: &str) -> [c_char; 13] {
    fill_c_string::<13>(s)
}

const MAXPOTIONS: usize = 14;
const MAXRINGS: usize = 14;
const MAXSCROLLS: usize = 18;
const MAXSTICKS: usize = 14;
const MAXARMORS: usize = 8;
const MAXWEAPONS: usize = 9;
const NUMTHINGS: usize = 7;
const MAXMONSTERS: usize = 26;

/// Stat-table entry for a monster kind, using native Rust types (no C ABI).
#[derive(Copy, Clone)]
pub struct CMonster {
    pub m_name: &'static str,
    pub m_carry: i32,
    pub m_flags: i16,
    pub m_stats: Stats,
}

/// Item information table entry, using native Rust types (no C ABI).
#[derive(Clone)]
pub struct CObjInfo {
    pub oi_name: &'static str,
    pub oi_prob: i32,
    pub oi_worth: i32,
    pub oi_guess: Option<String>,
    pub oi_know: bool,
}

pub type CThing = PlayerCThing;
pub type CThingMonster = crate::entity::player::ThingMonster;
pub type CThingObject = crate::entity::player::ThingObject;

#[no_mangle]
pub static mut allscore: c_uchar = 1; // ALLSCORES is enabled in the standard build
#[no_mangle]
pub static mut after: bool = false;
#[no_mangle]
pub static mut again: bool = false;
#[no_mangle]
pub static mut noscore: c_int = 0;
#[no_mangle]
pub static mut seenstairs: c_uchar = 0;
#[no_mangle]
pub static mut amulet: bool = false;
#[no_mangle]
pub static mut door_stop: bool = false;
#[no_mangle]
pub static mut fight_flush: c_uchar = 0;
#[no_mangle]
pub static mut firstmove: bool = false;
#[no_mangle]
pub static mut got_ltc: bool = false;
#[no_mangle]
pub static mut has_hit: bool = false;
#[no_mangle]
pub static mut in_shell: c_uchar = 0;
#[no_mangle]
pub static mut inv_describe: c_uchar = 1;
#[no_mangle]
pub static mut jump: bool = false;
#[no_mangle]
pub static mut kamikaze: bool = false;
#[no_mangle]
pub static mut lower_msg: c_uchar = 0;
#[no_mangle]
pub static mut move_on: bool = false;
#[no_mangle]
pub static mut msg_esc: bool = false;
#[no_mangle]
pub static mut passgo: c_uchar = 0;
#[no_mangle]
pub static mut playing: bool = true;
#[no_mangle]
pub static mut q_comm: c_uchar = 0;
#[no_mangle]
pub static mut running: bool = false;
#[no_mangle]
pub static mut save_msg: c_uchar = 1;
#[no_mangle]
pub static mut see_floor: bool = true;
#[no_mangle]
pub static mut stat_msg: c_uchar = 0;
#[no_mangle]
pub static mut terse: bool = false;
#[no_mangle]
pub static mut to_death: bool = false;
#[no_mangle]
pub static mut tombstone: c_uchar = 1;
#[no_mangle]
pub static master_mode_enabled: c_uchar = 1;
#[no_mangle]
pub static mut wizard: c_int = 0;
#[no_mangle]
pub static mut pack_used: [c_uchar; 26] = [0; 26];

#[no_mangle]
pub static mut dir_ch: c_char = 0;
#[no_mangle]
pub static mut file_name: [c_char; MAXSTR] = [0; MAXSTR];
#[no_mangle]
pub static mut huh: [c_char; MAXSTR] = [0; MAXSTR];
#[no_mangle]
pub static mut p_colors: [*mut c_char; MAXPOTIONS] = [std::ptr::null_mut(); MAXPOTIONS];
#[no_mangle]
pub static mut prbuf: [c_char; 2 * MAXSTR] = [0; 2 * MAXSTR];
#[no_mangle]
pub static mut r_stones: [*mut c_char; MAXRINGS] = [std::ptr::null_mut(); MAXRINGS];
#[no_mangle]
pub static mut runch: c_char = 0;
#[no_mangle]
pub static mut s_names: [*mut c_char; MAXSCROLLS] = [std::ptr::null_mut(); MAXSCROLLS];
#[no_mangle]
pub static mut take: c_char = 0;
#[no_mangle]
pub static mut whoami: [c_char; MAXSTR] = [0; MAXSTR];
#[no_mangle]
pub static mut ws_made: [*mut c_char; MAXSTICKS] = [std::ptr::null_mut(); MAXSTICKS];
#[no_mangle]
pub static mut ws_type: [*mut c_char; MAXSTICKS] = [std::ptr::null_mut(); MAXSTICKS];
#[no_mangle]
pub static mut orig_dsusp: c_int = 0;
#[no_mangle]
pub static mut fruit: [c_char; MAXSTR] = fill_c_string("slime-mold");
#[no_mangle]
pub static mut home: [c_char; MAXSTR] = [0; MAXSTR];
#[no_mangle]
pub static mut inv_t_name: [*mut c_char; 3] = [
    b"Overwrite\0".as_ptr() as *mut c_char,
    b"Slow\0".as_ptr() as *mut c_char,
    b"Clear\0".as_ptr() as *mut c_char,
];
#[no_mangle]
pub static mut l_last_comm: c_char = 0;
#[no_mangle]
pub static mut l_last_dir: c_char = 0;
#[no_mangle]
pub static mut last_comm: c_char = 0;
#[no_mangle]
pub static mut last_dir: c_char = 0;
#[no_mangle]
pub static mut tr_name: [*mut c_char; GameConfig::TRAP_KIND_COUNT as usize] = [
    b"a trapdoor\0".as_ptr() as *mut c_char,
    b"an arrow trap\0".as_ptr() as *mut c_char,
    b"a sleeping gas trap\0".as_ptr() as *mut c_char,
    b"a beartrap\0".as_ptr() as *mut c_char,
    b"a teleport trap\0".as_ptr() as *mut c_char,
    b"a poison dart trap\0".as_ptr() as *mut c_char,
    b"a rust trap\0".as_ptr() as *mut c_char,
    b"a mysterious trap\0".as_ptr() as *mut c_char,
];
#[no_mangle]
pub static mut numscores: c_uint = 10; // NUMSCORES from config.h
#[no_mangle]
pub static mut Numname: *mut c_char = b"Ten\0".as_ptr() as *mut c_char; // NUMNAME from config.h
#[no_mangle]
pub static mut n_objs: c_int = 0;
#[no_mangle]
pub static mut ntraps: c_int = 0;
#[no_mangle]
pub static mut hungry_state: c_int = 0;
#[no_mangle]
pub static mut inpack: c_int = 0;
#[no_mangle]
pub static mut inv_type: c_int = 0;
#[no_mangle]
pub static mut max_hit: c_int = 0;
#[no_mangle]
pub static mut max_level: c_int = 0;
#[no_mangle]
pub static mut mpos: c_int = 0;
#[no_mangle]
pub static mut no_food: c_int = 0;
#[no_mangle]
pub static mut a_class: [c_int; MAXARMORS] = [8, 7, 7, 6, 5, 4, 4, 3];
#[no_mangle]
pub static mut count: c_int = 0;
#[no_mangle]
pub static mut scoreboard: *mut crate::score::CFile = std::ptr::null_mut();
#[no_mangle]
pub static mut food_left: c_int = 0;
#[no_mangle]
pub static mut lastscore: c_int = -1;
#[no_mangle]
pub static mut no_command: c_int = 0;
#[no_mangle]
pub static mut no_move: c_int = 0;
#[no_mangle]
pub static mut purse: c_int = 0;
#[no_mangle]
pub static mut quiet: c_int = 0;
#[no_mangle]
pub static mut vf_hit: c_int = 0;
#[no_mangle]
pub static mut dnum: c_int = 0;
#[no_mangle]
pub static mut seed: c_int = 0;
#[no_mangle]
pub static mut e_levels: [c_int; 21] = [
    10, 20, 40, 80, 160, 320, 640, 1300, 2600, 5200, 13000, 26000, 50000, 100000, 200000, 400000,
    800000, 2000000, 4000000, 8000000, 0,
];
#[no_mangle]
pub static mut delta: IVec2 = IVec2 { x: 0, y: 0 };
#[no_mangle]
pub static mut oldpos: IVec2 = IVec2 { x: 0, y: 0 };
#[no_mangle]
pub static mut l_last_pick: *mut CThing = std::ptr::null_mut();
#[no_mangle]
pub static mut last_pick: *mut CThing = std::ptr::null_mut();

#[no_mangle]
pub static mut max_stats: Stats = Stats {
    strength: 16,
    experience: 0,
    level: 1,
    armor: 10,
    hit_points: 12,
    damage: dmg_string("1x4"),
    max_hit_points: 12,
};
#[no_mangle]
pub static mut oldrp: Option<usize> = None;
#[no_mangle]
pub static mut monsters: [CMonster; MAXMONSTERS] = [
    CMonster {
        m_name: "aquator",
        m_carry: 0,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 20,
            level: 5,
            armor: 2,
            hit_points: 1,
            damage: dmg_string("0x0/0x0"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "bat",
        m_carry: 0,
        m_flags: 0o000200,
        m_stats: Stats {
            strength: 10,
            experience: 1,
            level: 1,
            armor: 3,
            hit_points: 1,
            damage: dmg_string("1x2"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "centaur",
        m_carry: 15,
        m_flags: 0,
        m_stats: Stats {
            strength: 10,
            experience: 17,
            level: 4,
            armor: 4,
            hit_points: 1,
            damage: dmg_string("1x2/1x5/1x5"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "dragon",
        m_carry: 100,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 5000,
            level: 10,
            armor: -1,
            hit_points: 1,
            damage: dmg_string("1x8/1x8/3x10"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "emu",
        m_carry: 0,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 2,
            level: 1,
            armor: 7,
            hit_points: 1,
            damage: dmg_string("1x2"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "venus flytrap",
        m_carry: 0,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 80,
            level: 8,
            armor: 3,
            hit_points: 1,
            damage: dmg_string("%%%x0"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "griffin",
        m_carry: 20,
        m_flags: 0o004000 | 0o000200 | 0o000100,
        m_stats: Stats {
            strength: 10,
            experience: 2000,
            level: 13,
            armor: 2,
            hit_points: 1,
            damage: dmg_string("4x3/3x5"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "hobgoblin",
        m_carry: 0,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 3,
            level: 1,
            armor: 5,
            hit_points: 1,
            damage: dmg_string("1x8"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "ice monster",
        m_carry: 0,
        m_flags: 0,
        m_stats: Stats {
            strength: 10,
            experience: 5,
            level: 1,
            armor: 9,
            hit_points: 1,
            damage: dmg_string("0x0"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "jabberwock",
        m_carry: 70,
        m_flags: 0,
        m_stats: Stats {
            strength: 10,
            experience: 3000,
            level: 15,
            armor: 6,
            hit_points: 1,
            damage: dmg_string("2x12/2x4"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "kestrel",
        m_carry: 0,
        m_flags: 0o004000 | 0o000200,
        m_stats: Stats {
            strength: 10,
            experience: 1,
            level: 1,
            armor: 7,
            hit_points: 1,
            damage: dmg_string("1x4"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "leprechaun",
        m_carry: 0,
        m_flags: 0,
        m_stats: Stats {
            strength: 10,
            experience: 10,
            level: 3,
            armor: 8,
            hit_points: 1,
            damage: dmg_string("1x1"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "medusa",
        m_carry: 40,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 200,
            level: 8,
            armor: 2,
            hit_points: 1,
            damage: dmg_string("3x4/3x4/2x5"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "nymph",
        m_carry: 100,
        m_flags: 0,
        m_stats: Stats {
            strength: 10,
            experience: 37,
            level: 3,
            armor: 9,
            hit_points: 1,
            damage: dmg_string("0x0"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "orc",
        m_carry: 15,
        m_flags: 0o000040,
        m_stats: Stats {
            strength: 10,
            experience: 5,
            level: 1,
            armor: 6,
            hit_points: 1,
            damage: dmg_string("1x8"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "phantom",
        m_carry: 0,
        m_flags: 0o000200,
        m_stats: Stats {
            strength: 10,
            experience: 120,
            level: 8,
            armor: 3,
            hit_points: 1,
            damage: dmg_string("4x4"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "quagga",
        m_carry: 0,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 15,
            level: 3,
            armor: 3,
            hit_points: 1,
            damage: dmg_string("1x5/1x5"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "rattlesnake",
        m_carry: 0,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 9,
            level: 2,
            armor: 3,
            hit_points: 1,
            damage: dmg_string("1x6"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "snake",
        m_carry: 0,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 2,
            level: 1,
            armor: 5,
            hit_points: 1,
            damage: dmg_string("1x3"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "troll",
        m_carry: 50,
        m_flags: 0o000100 | 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 120,
            level: 6,
            armor: 4,
            hit_points: 1,
            damage: dmg_string("1x8/1x8/2x6"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "black unicorn",
        m_carry: 0,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 190,
            level: 7,
            armor: -2,
            hit_points: 1,
            damage: dmg_string("1x9/1x9/2x9"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "vampire",
        m_carry: 20,
        m_flags: 0o000100 | 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 350,
            level: 8,
            armor: 1,
            hit_points: 1,
            damage: dmg_string("1x10"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "wraith",
        m_carry: 0,
        m_flags: 0,
        m_stats: Stats {
            strength: 10,
            experience: 55,
            level: 5,
            armor: 4,
            hit_points: 1,
            damage: dmg_string("1x6"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "xeroc",
        m_carry: 30,
        m_flags: 0,
        m_stats: Stats {
            strength: 10,
            experience: 100,
            level: 7,
            armor: 7,
            hit_points: 1,
            damage: dmg_string("4x4"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "yeti",
        m_carry: 30,
        m_flags: 0,
        m_stats: Stats {
            strength: 10,
            experience: 50,
            level: 4,
            armor: 6,
            hit_points: 1,
            damage: dmg_string("1x6/1x6"),
            max_hit_points: 0,
        },
    },
    CMonster {
        m_name: "zombie",
        m_carry: 0,
        m_flags: 0o004000,
        m_stats: Stats {
            strength: 10,
            experience: 6,
            level: 2,
            armor: 8,
            hit_points: 1,
            damage: dmg_string("1x8"),
            max_hit_points: 0,
        },
    },
];

#[no_mangle]
pub static mut things: [CObjInfo; NUMTHINGS] = [
    CObjInfo {
        oi_name: "potion",
        oi_prob: 0,
        oi_worth: 26,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "scroll",
        oi_prob: 0,
        oi_worth: 36,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "food",
        oi_prob: 0,
        oi_worth: 16,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "weapon",
        oi_prob: 0,
        oi_worth: 7,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "armor",
        oi_prob: 0,
        oi_worth: 7,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "ring",
        oi_prob: 0,
        oi_worth: 4,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "stick",
        oi_prob: 0,
        oi_worth: 4,
        oi_guess: None,
        oi_know: false,
    },
];

#[no_mangle]
pub static mut arm_info: [CObjInfo; MAXARMORS] = [
    CObjInfo {
        oi_name: "leather armor",
        oi_prob: 20,
        oi_worth: 20,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "ring mail",
        oi_prob: 15,
        oi_worth: 25,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "studded leather armor",
        oi_prob: 15,
        oi_worth: 20,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "scale mail",
        oi_prob: 13,
        oi_worth: 30,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "chain mail",
        oi_prob: 12,
        oi_worth: 75,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "splint mail",
        oi_prob: 10,
        oi_worth: 80,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "banded mail",
        oi_prob: 10,
        oi_worth: 90,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "plate mail",
        oi_prob: 5,
        oi_worth: 150,
        oi_guess: None,
        oi_know: false,
    },
];

#[no_mangle]
pub static mut pot_info: [CObjInfo; MAXPOTIONS] = [
    CObjInfo {
        oi_name: "confusion",
        oi_prob: 7,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "hallucination",
        oi_prob: 8,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "poison",
        oi_prob: 8,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "gain strength",
        oi_prob: 13,
        oi_worth: 150,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "see invisible",
        oi_prob: 3,
        oi_worth: 100,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "healing",
        oi_prob: 13,
        oi_worth: 130,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "monster detection",
        oi_prob: 6,
        oi_worth: 130,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "magic detection",
        oi_prob: 6,
        oi_worth: 105,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "raise level",
        oi_prob: 2,
        oi_worth: 250,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "extra healing",
        oi_prob: 5,
        oi_worth: 200,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "haste self",
        oi_prob: 5,
        oi_worth: 190,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "restore strength",
        oi_prob: 13,
        oi_worth: 130,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "blindness",
        oi_prob: 5,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "levitation",
        oi_prob: 6,
        oi_worth: 75,
        oi_guess: None,
        oi_know: false,
    },
];

#[no_mangle]
pub static mut ring_info: [CObjInfo; MAXRINGS] = [
    CObjInfo {
        oi_name: "protection",
        oi_prob: 9,
        oi_worth: 400,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "add strength",
        oi_prob: 9,
        oi_worth: 400,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "sustain strength",
        oi_prob: 5,
        oi_worth: 280,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "searching",
        oi_prob: 10,
        oi_worth: 420,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "see invisible",
        oi_prob: 10,
        oi_worth: 310,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "adornment",
        oi_prob: 1,
        oi_worth: 10,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "aggravate monster",
        oi_prob: 10,
        oi_worth: 10,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "dexterity",
        oi_prob: 8,
        oi_worth: 440,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "increase damage",
        oi_prob: 8,
        oi_worth: 400,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "regeneration",
        oi_prob: 4,
        oi_worth: 460,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "slow digestion",
        oi_prob: 9,
        oi_worth: 240,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "teleportation",
        oi_prob: 5,
        oi_worth: 30,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "stealth",
        oi_prob: 7,
        oi_worth: 470,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "maintain armor",
        oi_prob: 5,
        oi_worth: 380,
        oi_guess: None,
        oi_know: false,
    },
];

#[no_mangle]
pub static mut scr_info: [CObjInfo; MAXSCROLLS] = [
    CObjInfo {
        oi_name: "monster confusion",
        oi_prob: 7,
        oi_worth: 140,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "magic mapping",
        oi_prob: 4,
        oi_worth: 150,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "hold monster",
        oi_prob: 2,
        oi_worth: 180,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "sleep",
        oi_prob: 3,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "enchant armor",
        oi_prob: 7,
        oi_worth: 160,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "identify potion",
        oi_prob: 10,
        oi_worth: 80,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "identify scroll",
        oi_prob: 10,
        oi_worth: 80,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "identify weapon",
        oi_prob: 6,
        oi_worth: 80,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "identify armor",
        oi_prob: 7,
        oi_worth: 100,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "identify ring, wand or staff",
        oi_prob: 10,
        oi_worth: 115,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "scare monster",
        oi_prob: 3,
        oi_worth: 200,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "food detection",
        oi_prob: 2,
        oi_worth: 60,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "teleportation",
        oi_prob: 5,
        oi_worth: 165,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "enchant weapon",
        oi_prob: 8,
        oi_worth: 150,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "create monster",
        oi_prob: 4,
        oi_worth: 75,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "remove curse",
        oi_prob: 7,
        oi_worth: 105,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "aggravate monsters",
        oi_prob: 3,
        oi_worth: 20,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "protect armor",
        oi_prob: 2,
        oi_worth: 250,
        oi_guess: None,
        oi_know: false,
    },
];

#[no_mangle]
pub static mut weap_info: [CObjInfo; MAXWEAPONS + 1] = [
    CObjInfo {
        oi_name: "mace",
        oi_prob: 11,
        oi_worth: 8,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "long sword",
        oi_prob: 11,
        oi_worth: 15,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "short bow",
        oi_prob: 12,
        oi_worth: 15,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "arrow",
        oi_prob: 12,
        oi_worth: 1,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "dagger",
        oi_prob: 8,
        oi_worth: 3,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "two handed sword",
        oi_prob: 10,
        oi_worth: 75,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "dart",
        oi_prob: 12,
        oi_worth: 2,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "shuriken",
        oi_prob: 12,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "spear",
        oi_prob: 12,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "",
        oi_prob: 0,
        oi_worth: 0,
        oi_guess: None,
        oi_know: false,
    },
];

#[no_mangle]
pub static mut ws_info: [CObjInfo; MAXSTICKS] = [
    CObjInfo {
        oi_name: "light",
        oi_prob: 12,
        oi_worth: 250,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "invisibility",
        oi_prob: 6,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "lightning",
        oi_prob: 3,
        oi_worth: 330,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "fire",
        oi_prob: 3,
        oi_worth: 330,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "cold",
        oi_prob: 3,
        oi_worth: 330,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "polymorph",
        oi_prob: 15,
        oi_worth: 310,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "magic missile",
        oi_prob: 10,
        oi_worth: 170,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "haste monster",
        oi_prob: 10,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "slow monster",
        oi_prob: 11,
        oi_worth: 350,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "drain life",
        oi_prob: 9,
        oi_worth: 300,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "nothing",
        oi_prob: 1,
        oi_worth: 5,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "teleport away",
        oi_prob: 6,
        oi_worth: 340,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "teleport to",
        oi_prob: 6,
        oi_worth: 50,
        oi_guess: None,
        oi_know: false,
    },
    CObjInfo {
        oi_name: "cancellation",
        oi_prob: 5,
        oi_worth: 280,
        oi_guess: None,
        oi_know: false,
    },
];

// ─── Safe accessors for globals consumed by the UI layer ─────────────────────
//
// The statics above remain `static mut` (they are still shared with the
// save/restore layer), but UI code reads and writes them through these small
// wrappers so it does not have to open `unsafe` blocks itself.

/// The player's hunger state (`0` = not hungry).
#[inline]
pub fn get_hungry_state() -> i32 {
    unsafe { hungry_state }
}

/// The player's current gold.
#[inline]
pub fn get_purse() -> i32 {
    unsafe { purse }
}

/// The current message cursor column used for `--More--` pagination.
#[inline]
pub fn get_mpos() -> i32 {
    unsafe { mpos }
}

/// Set the message cursor column.
#[inline]
pub fn set_mpos(value: i32) {
    unsafe {
        mpos = value;
    }
}

/// Whether the player may leave the message display; when false only the first
/// message line is shown.
#[inline]
pub fn msg_esc_enabled() -> bool {
    unsafe { msg_esc }
}

/// Whether messages should be saved into the `huh` history buffer.
#[inline]
pub fn save_msg_enabled() -> bool {
    unsafe { save_msg != 0 }
}

/// Whether messages are displayed in lower case.
#[inline]
pub fn lower_msg_enabled() -> bool {
    unsafe { lower_msg != 0 }
}

/// Whether the status line is being shown through the message line.
#[inline]
pub fn stat_msg_enabled() -> bool {
    unsafe { stat_msg != 0 }
}

/// The player's maximum stats (the "max_stats" table).
#[inline]
pub fn get_max_stats() -> Stats {
    unsafe { max_stats }
}

/// Copy `text` into the `huh` message-history buffer (NUL-terminated, capped at
/// `MAXSTR - 1` bytes).
pub fn set_huh_string(text: &str) {
    unsafe {
        let bytes = text.as_bytes();
        let copy_len = bytes.len().min(MAXSTR - 1);
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr().cast::<c_char>(),
            huh.as_mut_ptr(),
            copy_len,
        );
        huh[copy_len] = 0;
    }
}
