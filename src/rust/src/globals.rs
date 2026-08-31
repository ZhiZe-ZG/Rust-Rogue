use crate::player::{
    CCoord as PlayerCCoord, CPlace as PlayerCPlace, CRoom as PlayerCRoom,
    CStats as PlayerCStats, CThing as PlayerCThing,
};
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

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

const fn set_help(
    entries: &mut [CHList; 80],
    idx: usize,
    ch: c_char,
    desc: *mut c_char,
    print: c_uchar,
) {
    entries[idx] = CHList { h_ch: ch, h_desc: desc, h_print: print };
}

const MAXPOTIONS: usize = 14;
const MAXRINGS: usize = 14;
const MAXSCROLLS: usize = 18;
const MAXSTICKS: usize = 14;
const MAXARMORS: usize = 8;
const MAXWEAPONS: usize = 9;
const NUMTHINGS: usize = 7;
const MAXROOMS: usize = 9;
const MAXPASS: usize = 13;
const MAXMONSTERS: usize = 26;
const MAXTRAPS: usize = 8;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CHList {
    pub h_ch: c_char,
    pub h_desc: *mut c_char,
    pub h_print: c_uchar,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CMonsterState {
    pub m_name: *mut c_char,
    pub m_carry: c_int,
    pub m_flags: c_short,
    pub m_stats: CStats,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CObjInfo {
    pub oi_name: *mut c_char,
    pub oi_prob: c_int,
    pub oi_worth: c_int,
    pub oi_guess: *mut c_char,
    pub oi_know: c_uchar,
}

pub type CCoord = PlayerCCoord;
pub type CStats = PlayerCStats;
pub type CRoom = PlayerCRoom;
pub type CThing = PlayerCThing;
pub type CThingMonster = crate::player::CThingMonster;
pub type CThingObject = crate::player::CThingObject;
pub type CPlace = PlayerCPlace;

#[no_mangle]
pub static mut after: c_uchar = 0;
#[no_mangle]
pub static mut again: c_uchar = 0;
#[no_mangle]
pub static mut noscore: c_int = 0;
#[no_mangle]
pub static mut seenstairs: c_uchar = 0;
#[no_mangle]
pub static mut amulet: c_uchar = 0;
#[no_mangle]
pub static mut door_stop: c_uchar = 0;
#[no_mangle]
pub static mut fight_flush: c_uchar = 0;
#[no_mangle]
pub static mut firstmove: c_uchar = 0;
#[no_mangle]
pub static mut got_ltc: c_uchar = 0;
#[no_mangle]
pub static mut has_hit: c_uchar = 0;
#[no_mangle]
pub static mut in_shell: c_uchar = 0;
#[no_mangle]
pub static mut inv_describe: c_uchar = 1;
#[no_mangle]
pub static mut jump: c_uchar = 0;
#[no_mangle]
pub static mut kamikaze: c_uchar = 0;
#[no_mangle]
pub static mut lower_msg: c_uchar = 0;
#[no_mangle]
pub static mut move_on: c_uchar = 0;
#[no_mangle]
pub static mut msg_esc: c_uchar = 0;
#[no_mangle]
pub static mut passgo: c_uchar = 0;
#[no_mangle]
pub static mut playing: c_uchar = 1;
#[no_mangle]
pub static mut q_comm: c_uchar = 0;
#[no_mangle]
pub static mut running: c_uchar = 0;
#[no_mangle]
pub static mut save_msg: c_uchar = 1;
#[no_mangle]
pub static mut see_floor: c_uchar = 1;
#[no_mangle]
pub static mut stat_msg: c_uchar = 0;
#[no_mangle]
pub static mut terse: c_uchar = 0;
#[no_mangle]
pub static mut to_death: c_uchar = 0;
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
pub static mut tr_name: [*mut c_char; MAXTRAPS] = [
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
pub static mut level: c_int = 1;
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
    10, 20, 40, 80, 160, 320, 640, 1300, 2600, 5200, 13000, 26000, 50000,
    100000, 200000, 400000, 800000, 2000000, 4000000, 8000000, 0,
];
#[no_mangle]
pub static mut delta: CCoord = CCoord { x: 0, y: 0 };
#[no_mangle]
pub static mut oldpos: CCoord = CCoord { x: 0, y: 0 };
#[no_mangle]
pub static mut stairs: CCoord = CCoord { x: 0, y: 0 };
#[no_mangle]
pub static mut cur_armor: *mut CThing = std::ptr::null_mut();
#[no_mangle]
pub static mut cur_ring: [*mut CThing; 2] = [std::ptr::null_mut(); 2];
#[no_mangle]
pub static mut cur_weapon: *mut CThing = std::ptr::null_mut();
#[no_mangle]
pub static mut l_last_pick: *mut CThing = std::ptr::null_mut();
#[no_mangle]
pub static mut last_pick: *mut CThing = std::ptr::null_mut();
#[no_mangle]
pub static mut lvl_obj: *mut CThing = std::ptr::null_mut();
#[no_mangle]
pub static mut mlist: *mut CThing = std::ptr::null_mut();
#[no_mangle]
pub static mut player: CThing = CThing { t: crate::player::CThingMonster {
    l_next: std::ptr::null_mut(),
    l_prev: std::ptr::null_mut(),
    t_pos: CCoord { x: 0, y: 0 },
    t_turn: 0,
    t_type: 0,
    t_disguise: 0,
    t_oldch: 0,
    t_dest: std::ptr::null_mut(),
    t_flags: 0,
    t_stats: CStats {
        s_str: 0,
        s_exp: 0,
        s_lvl: 0,
        s_arm: 0,
        s_hpt: 0,
        s_dmg: [0; 13],
        s_maxhp: 0,
    },
    t_room: std::ptr::null_mut(),
    t_pack: std::ptr::null_mut(),
    t_reserved: 0,
} };

    const fn make_helpstr() -> [CHList; 80] {
        let mut entries = [CHList { h_ch: 0, h_desc: std::ptr::null_mut(), h_print: 0 }; 80];
        let mut i = 0usize;

        set_help(&mut entries, i, b'?' as c_char, b"\tprints help\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'/' as c_char, b"\tidentify object\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'h' as c_char, b"\tleft\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'j' as c_char, b"\tdown\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'k' as c_char, b"\tup\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'l' as c_char, b"\tright\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'y' as c_char, b"\tup & left\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'u' as c_char, b"\tup & right\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'b' as c_char, b"\tdown & left\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'n' as c_char, b"\tdown & right\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'H' as c_char, b"\trun left\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, b'J' as c_char, b"\trun down\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, b'K' as c_char, b"\trun up\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, b'L' as c_char, b"\trun right\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, b'Y' as c_char, b"\trun up & left\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, b'U' as c_char, b"\trun up & right\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, b'B' as c_char, b"\trun down & left\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, b'N' as c_char, b"\trun down & right\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, 0x08 as c_char, b"\trun left until adjacent\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, 0x0a as c_char, b"\trun down until adjacent\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, 0x0b as c_char, b"\trun up until adjacent\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, 0x0c as c_char, b"\trun right until adjacent\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, 0x19 as c_char, b"\trun up & left until adjacent\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, 0x15 as c_char, b"\trun up & right until adjacent\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, 0x02 as c_char, b"\trun down & left until adjacent\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, 0x16 as c_char, b"\trun down & right until adjacent\0".as_ptr() as *mut c_char, 0); i += 1;
        set_help(&mut entries, i, 0, b"\t<SHIFT><dir>: run that way\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, 0, b"\t<CTRL><dir>: run till adjacent\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'f' as c_char, b"<dir>\tfight till death or near death\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b't' as c_char, b"<dir>\tthrow something\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'm' as c_char, b"<dir>\tmove onto without picking up\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'z' as c_char, b"<dir>\tzap a wand in a direction\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'^' as c_char, b"<dir>\tidentify trap type\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b's' as c_char, b"\tsearch for trap/secret door\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'>' as c_char, b"\tgo down a staircase\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'<' as c_char, b"\tgo up a staircase\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'.' as c_char, b"\trest for a turn\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b',' as c_char, b"\tpick something up\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'i' as c_char, b"\tinventory\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'I' as c_char, b"\tinventory single item\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'q' as c_char, b"\tquaff potion\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'r' as c_char, b"\tread scroll\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'e' as c_char, b"\teat food\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'w' as c_char, b"\twield a weapon\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'W' as c_char, b"\twear armor\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'T' as c_char, b"\ttake armor off\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'P' as c_char, b"\tput on ring\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'R' as c_char, b"\tremove ring\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'd' as c_char, b"\tdrop object\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'c' as c_char, b"\tcall object\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'a' as c_char, b"\trepeat last command\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b')' as c_char, b"\tprint current weapon\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b']' as c_char, b"\tprint current armor\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'=' as c_char, b"\tprint current rings\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'@' as c_char, b"\tprint current stats\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'D' as c_char, b"\trecall what's been discovered\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'o' as c_char, b"\texamine/set options\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, 0x12 as c_char, b"\tredraw screen\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, 0x10 as c_char, b"\trepeat last message\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, 0x1b as c_char, b"\tcancel command\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'S' as c_char, b"\tsave game\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'Q' as c_char, b"\tquit\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'!' as c_char, b"\tshell escape\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'F' as c_char, b"<dir>\tfight till either of you dies\0".as_ptr() as *mut c_char, 1); i += 1;
        set_help(&mut entries, i, b'v' as c_char, b"\tprint version number\0".as_ptr() as *mut c_char, 1); i += 1;
        entries[i] = CHList { h_ch: 0, h_desc: std::ptr::null_mut(), h_print: 0 };
        entries
    }

    #[no_mangle]
    pub static mut helpstr: [CHList; 80] = make_helpstr();
#[no_mangle]
pub static mut hw: *mut std::ffi::c_void = std::ptr::null_mut();
#[no_mangle]
pub static mut max_stats: CStats = CStats {
    s_str: 16,
    s_exp: 0,
    s_lvl: 1,
    s_arm: 10,
    s_hpt: 12,
    s_dmg: dmg_string("1x4"),
    s_maxhp: 12,
};
#[no_mangle]
pub static mut oldrp: *mut CRoom = std::ptr::null_mut();
#[no_mangle]
pub static mut rooms: [CRoom; MAXROOMS] = [
    CRoom {
        r_pos: CCoord { x: 0, y: 0 },
        r_max: CCoord { x: 0, y: 0 },
        r_gold: CCoord { x: 0, y: 0 },
        r_goldval: 0,
        r_flags: 0,
        r_nexits: 0,
        r_exit: [CCoord { x: 0, y: 0 }; 12],
    }; MAXROOMS
];
#[no_mangle]
pub static mut passages: [CRoom; MAXPASS] = [
    CRoom {
        r_pos: CCoord { x: 0, y: 0 },
        r_max: CCoord { x: 0, y: 0 },
        r_gold: CCoord { x: 0, y: 0 },
        r_goldval: 0,
        r_flags: 0,
        r_nexits: 0,
        r_exit: [CCoord { x: 0, y: 0 }; 12],
    }; MAXPASS
];
#[no_mangle]
pub static mut monsters: [CMonsterState; MAXMONSTERS] = [
    CMonsterState { m_name: b"aquator\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 20, s_lvl: 5, s_arm: 2, s_hpt: 1, s_dmg: dmg_string("0x0/0x0"), s_maxhp: 0 } },
    CMonsterState { m_name: b"bat\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o000200, m_stats: CStats { s_str: 10, s_exp: 1, s_lvl: 1, s_arm: 3, s_hpt: 1, s_dmg: dmg_string("1x2"), s_maxhp: 0 } },
    CMonsterState { m_name: b"centaur\0".as_ptr() as *mut c_char, m_carry: 15, m_flags: 0, m_stats: CStats { s_str: 10, s_exp: 17, s_lvl: 4, s_arm: 4, s_hpt: 1, s_dmg: dmg_string("1x2/1x5/1x5"), s_maxhp: 0 } },
    CMonsterState { m_name: b"dragon\0".as_ptr() as *mut c_char, m_carry: 100, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 5000, s_lvl: 10, s_arm: -1, s_hpt: 1, s_dmg: dmg_string("1x8/1x8/3x10"), s_maxhp: 0 } },
    CMonsterState { m_name: b"emu\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 2, s_lvl: 1, s_arm: 7, s_hpt: 1, s_dmg: dmg_string("1x2"), s_maxhp: 0 } },
    CMonsterState { m_name: b"venus flytrap\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 80, s_lvl: 8, s_arm: 3, s_hpt: 1, s_dmg: dmg_string("%%%x0"), s_maxhp: 0 } },
    CMonsterState { m_name: b"griffin\0".as_ptr() as *mut c_char, m_carry: 20, m_flags: 0o004000 | 0o000200 | 0o000100, m_stats: CStats { s_str: 10, s_exp: 2000, s_lvl: 13, s_arm: 2, s_hpt: 1, s_dmg: dmg_string("4x3/3x5"), s_maxhp: 0 } },
    CMonsterState { m_name: b"hobgoblin\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 3, s_lvl: 1, s_arm: 5, s_hpt: 1, s_dmg: dmg_string("1x8"), s_maxhp: 0 } },
    CMonsterState { m_name: b"ice monster\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0, m_stats: CStats { s_str: 10, s_exp: 5, s_lvl: 1, s_arm: 9, s_hpt: 1, s_dmg: dmg_string("0x0"), s_maxhp: 0 } },
    CMonsterState { m_name: b"jabberwock\0".as_ptr() as *mut c_char, m_carry: 70, m_flags: 0, m_stats: CStats { s_str: 10, s_exp: 3000, s_lvl: 15, s_arm: 6, s_hpt: 1, s_dmg: dmg_string("2x12/2x4"), s_maxhp: 0 } },
    CMonsterState { m_name: b"kestrel\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000 | 0o000200, m_stats: CStats { s_str: 10, s_exp: 1, s_lvl: 1, s_arm: 7, s_hpt: 1, s_dmg: dmg_string("1x4"), s_maxhp: 0 } },
    CMonsterState { m_name: b"leprechaun\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0, m_stats: CStats { s_str: 10, s_exp: 10, s_lvl: 3, s_arm: 8, s_hpt: 1, s_dmg: dmg_string("1x1"), s_maxhp: 0 } },
    CMonsterState { m_name: b"medusa\0".as_ptr() as *mut c_char, m_carry: 40, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 200, s_lvl: 8, s_arm: 2, s_hpt: 1, s_dmg: dmg_string("3x4/3x4/2x5"), s_maxhp: 0 } },
    CMonsterState { m_name: b"nymph\0".as_ptr() as *mut c_char, m_carry: 100, m_flags: 0, m_stats: CStats { s_str: 10, s_exp: 37, s_lvl: 3, s_arm: 9, s_hpt: 1, s_dmg: dmg_string("0x0"), s_maxhp: 0 } },
    CMonsterState { m_name: b"orc\0".as_ptr() as *mut c_char, m_carry: 15, m_flags: 0o000040, m_stats: CStats { s_str: 10, s_exp: 5, s_lvl: 1, s_arm: 6, s_hpt: 1, s_dmg: dmg_string("1x8"), s_maxhp: 0 } },
    CMonsterState { m_name: b"phantom\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o000200, m_stats: CStats { s_str: 10, s_exp: 120, s_lvl: 8, s_arm: 3, s_hpt: 1, s_dmg: dmg_string("4x4"), s_maxhp: 0 } },
    CMonsterState { m_name: b"quagga\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 15, s_lvl: 3, s_arm: 3, s_hpt: 1, s_dmg: dmg_string("1x5/1x5"), s_maxhp: 0 } },
    CMonsterState { m_name: b"rattlesnake\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 9, s_lvl: 2, s_arm: 3, s_hpt: 1, s_dmg: dmg_string("1x6"), s_maxhp: 0 } },
    CMonsterState { m_name: b"snake\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 2, s_lvl: 1, s_arm: 5, s_hpt: 1, s_dmg: dmg_string("1x3"), s_maxhp: 0 } },
    CMonsterState { m_name: b"troll\0".as_ptr() as *mut c_char, m_carry: 50, m_flags: 0o000100 | 0o004000, m_stats: CStats { s_str: 10, s_exp: 120, s_lvl: 6, s_arm: 4, s_hpt: 1, s_dmg: dmg_string("1x8/1x8/2x6"), s_maxhp: 0 } },
    CMonsterState { m_name: b"black unicorn\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 190, s_lvl: 7, s_arm: -2, s_hpt: 1, s_dmg: dmg_string("1x9/1x9/2x9"), s_maxhp: 0 } },
    CMonsterState { m_name: b"vampire\0".as_ptr() as *mut c_char, m_carry: 20, m_flags: 0o000100 | 0o004000, m_stats: CStats { s_str: 10, s_exp: 350, s_lvl: 8, s_arm: 1, s_hpt: 1, s_dmg: dmg_string("1x10"), s_maxhp: 0 } },
    CMonsterState { m_name: b"wraith\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0, m_stats: CStats { s_str: 10, s_exp: 55, s_lvl: 5, s_arm: 4, s_hpt: 1, s_dmg: dmg_string("1x6"), s_maxhp: 0 } },
    CMonsterState { m_name: b"xeroc\0".as_ptr() as *mut c_char, m_carry: 30, m_flags: 0, m_stats: CStats { s_str: 10, s_exp: 100, s_lvl: 7, s_arm: 7, s_hpt: 1, s_dmg: dmg_string("4x4"), s_maxhp: 0 } },
    CMonsterState { m_name: b"yeti\0".as_ptr() as *mut c_char, m_carry: 30, m_flags: 0, m_stats: CStats { s_str: 10, s_exp: 50, s_lvl: 4, s_arm: 6, s_hpt: 1, s_dmg: dmg_string("1x6/1x6"), s_maxhp: 0 } },
    CMonsterState { m_name: b"zombie\0".as_ptr() as *mut c_char, m_carry: 0, m_flags: 0o004000, m_stats: CStats { s_str: 10, s_exp: 6, s_lvl: 2, s_arm: 8, s_hpt: 1, s_dmg: dmg_string("1x8"), s_maxhp: 0 } },
];

#[no_mangle]
pub static mut things: [CObjInfo; NUMTHINGS] = [
    CObjInfo { oi_name: b"potion\0".as_ptr() as *mut c_char, oi_prob: 0, oi_worth: 26, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"scroll\0".as_ptr() as *mut c_char, oi_prob: 0, oi_worth: 36, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"food\0".as_ptr() as *mut c_char, oi_prob: 0, oi_worth: 16, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"weapon\0".as_ptr() as *mut c_char, oi_prob: 0, oi_worth: 7, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"armor\0".as_ptr() as *mut c_char, oi_prob: 0, oi_worth: 7, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"ring\0".as_ptr() as *mut c_char, oi_prob: 0, oi_worth: 4, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"stick\0".as_ptr() as *mut c_char, oi_prob: 0, oi_worth: 4, oi_guess: std::ptr::null_mut(), oi_know: 0 },
];

#[no_mangle]
pub static mut arm_info: [CObjInfo; MAXARMORS] = [
    CObjInfo { oi_name: b"leather armor\0".as_ptr() as *mut c_char, oi_prob: 20, oi_worth: 20, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"ring mail\0".as_ptr() as *mut c_char, oi_prob: 15, oi_worth: 25, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"studded leather armor\0".as_ptr() as *mut c_char, oi_prob: 15, oi_worth: 20, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"scale mail\0".as_ptr() as *mut c_char, oi_prob: 13, oi_worth: 30, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"chain mail\0".as_ptr() as *mut c_char, oi_prob: 12, oi_worth: 75, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"splint mail\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 80, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"banded mail\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 90, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"plate mail\0".as_ptr() as *mut c_char, oi_prob: 5, oi_worth: 150, oi_guess: std::ptr::null_mut(), oi_know: 0 },
];

#[no_mangle]
pub static mut pot_info: [CObjInfo; MAXPOTIONS] = [
    CObjInfo { oi_name: b"confusion\0".as_ptr() as *mut c_char, oi_prob: 7, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"hallucination\0".as_ptr() as *mut c_char, oi_prob: 8, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"poison\0".as_ptr() as *mut c_char, oi_prob: 8, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"gain strength\0".as_ptr() as *mut c_char, oi_prob: 13, oi_worth: 150, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"see invisible\0".as_ptr() as *mut c_char, oi_prob: 3, oi_worth: 100, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"healing\0".as_ptr() as *mut c_char, oi_prob: 13, oi_worth: 130, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"monster detection\0".as_ptr() as *mut c_char, oi_prob: 6, oi_worth: 130, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"magic detection\0".as_ptr() as *mut c_char, oi_prob: 6, oi_worth: 105, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"raise level\0".as_ptr() as *mut c_char, oi_prob: 2, oi_worth: 250, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"extra healing\0".as_ptr() as *mut c_char, oi_prob: 5, oi_worth: 200, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"haste self\0".as_ptr() as *mut c_char, oi_prob: 5, oi_worth: 190, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"restore strength\0".as_ptr() as *mut c_char, oi_prob: 13, oi_worth: 130, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"blindness\0".as_ptr() as *mut c_char, oi_prob: 5, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"levitation\0".as_ptr() as *mut c_char, oi_prob: 6, oi_worth: 75, oi_guess: std::ptr::null_mut(), oi_know: 0 },
];

#[no_mangle]
pub static mut ring_info: [CObjInfo; MAXRINGS] = [
    CObjInfo { oi_name: b"protection\0".as_ptr() as *mut c_char, oi_prob: 9, oi_worth: 400, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"add strength\0".as_ptr() as *mut c_char, oi_prob: 9, oi_worth: 400, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"sustain strength\0".as_ptr() as *mut c_char, oi_prob: 5, oi_worth: 280, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"searching\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 420, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"see invisible\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 310, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"adornment\0".as_ptr() as *mut c_char, oi_prob: 1, oi_worth: 10, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"aggravate monster\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 10, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"dexterity\0".as_ptr() as *mut c_char, oi_prob: 8, oi_worth: 440, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"increase damage\0".as_ptr() as *mut c_char, oi_prob: 8, oi_worth: 400, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"regeneration\0".as_ptr() as *mut c_char, oi_prob: 4, oi_worth: 460, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"slow digestion\0".as_ptr() as *mut c_char, oi_prob: 9, oi_worth: 240, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"teleportation\0".as_ptr() as *mut c_char, oi_prob: 5, oi_worth: 30, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"stealth\0".as_ptr() as *mut c_char, oi_prob: 7, oi_worth: 470, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"maintain armor\0".as_ptr() as *mut c_char, oi_prob: 5, oi_worth: 380, oi_guess: std::ptr::null_mut(), oi_know: 0 },
];

#[no_mangle]
pub static mut scr_info: [CObjInfo; MAXSCROLLS] = [
    CObjInfo { oi_name: b"monster confusion\0".as_ptr() as *mut c_char, oi_prob: 7, oi_worth: 140, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"magic mapping\0".as_ptr() as *mut c_char, oi_prob: 4, oi_worth: 150, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"hold monster\0".as_ptr() as *mut c_char, oi_prob: 2, oi_worth: 180, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"sleep\0".as_ptr() as *mut c_char, oi_prob: 3, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"enchant armor\0".as_ptr() as *mut c_char, oi_prob: 7, oi_worth: 160, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"identify potion\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 80, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"identify scroll\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 80, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"identify weapon\0".as_ptr() as *mut c_char, oi_prob: 6, oi_worth: 80, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"identify armor\0".as_ptr() as *mut c_char, oi_prob: 7, oi_worth: 100, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"identify ring, wand or staff\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 115, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"scare monster\0".as_ptr() as *mut c_char, oi_prob: 3, oi_worth: 200, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"food detection\0".as_ptr() as *mut c_char, oi_prob: 2, oi_worth: 60, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"teleportation\0".as_ptr() as *mut c_char, oi_prob: 5, oi_worth: 165, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"enchant weapon\0".as_ptr() as *mut c_char, oi_prob: 8, oi_worth: 150, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"create monster\0".as_ptr() as *mut c_char, oi_prob: 4, oi_worth: 75, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"remove curse\0".as_ptr() as *mut c_char, oi_prob: 7, oi_worth: 105, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"aggravate monsters\0".as_ptr() as *mut c_char, oi_prob: 3, oi_worth: 20, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"protect armor\0".as_ptr() as *mut c_char, oi_prob: 2, oi_worth: 250, oi_guess: std::ptr::null_mut(), oi_know: 0 },
];

#[no_mangle]
pub static mut weap_info: [CObjInfo; MAXWEAPONS + 1] = [
    CObjInfo { oi_name: b"mace\0".as_ptr() as *mut c_char, oi_prob: 11, oi_worth: 8, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"long sword\0".as_ptr() as *mut c_char, oi_prob: 11, oi_worth: 15, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"short bow\0".as_ptr() as *mut c_char, oi_prob: 12, oi_worth: 15, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"arrow\0".as_ptr() as *mut c_char, oi_prob: 12, oi_worth: 1, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"dagger\0".as_ptr() as *mut c_char, oi_prob: 8, oi_worth: 3, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"two handed sword\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 75, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"dart\0".as_ptr() as *mut c_char, oi_prob: 12, oi_worth: 2, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"shuriken\0".as_ptr() as *mut c_char, oi_prob: 12, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"spear\0".as_ptr() as *mut c_char, oi_prob: 12, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: std::ptr::null_mut(), oi_prob: 0, oi_worth: 0, oi_guess: std::ptr::null_mut(), oi_know: 0 },
];

#[no_mangle]
pub static mut ws_info: [CObjInfo; MAXSTICKS] = [
    CObjInfo { oi_name: b"light\0".as_ptr() as *mut c_char, oi_prob: 12, oi_worth: 250, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"invisibility\0".as_ptr() as *mut c_char, oi_prob: 6, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"lightning\0".as_ptr() as *mut c_char, oi_prob: 3, oi_worth: 330, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"fire\0".as_ptr() as *mut c_char, oi_prob: 3, oi_worth: 330, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"cold\0".as_ptr() as *mut c_char, oi_prob: 3, oi_worth: 330, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"polymorph\0".as_ptr() as *mut c_char, oi_prob: 15, oi_worth: 310, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"magic missile\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 170, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"haste monster\0".as_ptr() as *mut c_char, oi_prob: 10, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"slow monster\0".as_ptr() as *mut c_char, oi_prob: 11, oi_worth: 350, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"drain life\0".as_ptr() as *mut c_char, oi_prob: 9, oi_worth: 300, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"nothing\0".as_ptr() as *mut c_char, oi_prob: 1, oi_worth: 5, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"teleport away\0".as_ptr() as *mut c_char, oi_prob: 6, oi_worth: 340, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"teleport to\0".as_ptr() as *mut c_char, oi_prob: 6, oi_worth: 50, oi_guess: std::ptr::null_mut(), oi_know: 0 },
    CObjInfo { oi_name: b"cancellation\0".as_ptr() as *mut c_char, oi_prob: 5, oi_worth: 280, oi_guess: std::ptr::null_mut(), oi_know: 0 },
];

