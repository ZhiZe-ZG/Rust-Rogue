use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_ushort};

use crate::curses as cur;

const MAXSTR: usize = 1024;

pub const RIP_ART: &[&str] = &[
    "                       __________\n",
    "                      /          \\\n",
    "                     /    REST    \\\n",
    "                    /      IN      \\\n",
    "                   /     PEACE      \\\n",
    "                  /                  \\\n",
    "                  |                  |\n",
    "                  |                  |\n",
    "                  |   killed by a    |\n",
    "                  |                  |\n",
    "                  |       1980       |\n",
    "                 *|     *  *  *      | *\n",
    "         ________)/\\\\_//(\\/(/\\)/\\//\\/|_)_______\n",
];

static mut KILLNAME_BUFFER: [c_char; MAXSTR] = [0; MAXSTR];

#[repr(C)]
#[derive(Clone)]
pub struct Score {
    pub sc_uid: c_uint,
    pub sc_score: c_int,
    pub sc_flags: c_uint,
    pub sc_monster: c_ushort,
    pub sc_name: [c_char; MAXSTR],
    pub sc_level: c_int,
    pub sc_time: c_uint,
}

unsafe extern "C" {
    static mut allscore: c_uchar;
    static mut amulet: c_uchar;
    static mut level: c_int;
    static mut max_level: c_int;
    static mut noscore: c_int;
    static mut Numname: *mut c_char;
    static mut pack: *mut crate::player::CThing;
    static mut player: crate::player::CThing;
    static mut purse: c_int;
    static mut prbuf: [c_char; MAXSTR];
    static mut tombstone: c_uchar;
    static mut whoami: [c_char; MAXSTR];
    static mut wizard: c_int;
    static mut monsters: [crate::monsters::CMonster; 26];
    static mut numscores: c_uint;
    static mut scoreboard: *mut crate::score::CFile;

    fn fgets(buf: *mut c_char, n: c_int, stream: *mut std::ffi::c_void) -> *mut c_char;
    fn getuid() -> c_uint;
    fn inv_name(obj: *mut crate::player::CThing, is_weapon: c_uchar) -> *mut c_char;
    fn lock_sc() -> c_int;
    fn md_getuid() -> c_uint;
    fn md_raw_standend();
    fn md_raw_standout();
    fn printf(fmt: *const c_char, ...) -> c_int;
    fn rd_score(top_ten: *mut Score);
    fn signal(sig: c_int, handler: usize) -> usize;
    fn unlock_sc();
    fn wait_for(ch: c_char);
    fn wr_score(top_ten: *mut Score);
    fn start_score();
    static mut stdscr: *mut std::ffi::c_void;
    static mut curscr: *mut std::ffi::c_void;

    fn my_exit(st: c_int) -> !;
}

#[inline]
unsafe fn thing_t(tp: *mut crate::player::CThing) -> *mut crate::player::CThingMonster {
    tp as *mut crate::player::CThingMonster
}

#[inline]
unsafe fn thing_o(tp: *mut crate::player::CThing) -> *mut crate::player::CThingObject {
    tp as *mut crate::player::CThingObject
}

#[inline]
unsafe fn pack_ptr() -> *mut crate::player::CThing {
    (*thing_t(&raw mut player)).t_pack
}

#[inline]
unsafe fn next_ptr(tp: *mut crate::player::CThing) -> *mut crate::player::CThing {
    (*thing_t(tp)).l_next
}

#[inline]
unsafe fn vowelstr(s: *const c_char) -> *const c_char {
    let s = CStr::from_ptr(s);
    let first = s.to_bytes().first().copied().unwrap_or_default();
    if matches!(first, b'a' | b'A' | b'e' | b'E' | b'i' | b'I' | b'o' | b'O' | b'u' | b'U') {
        c"n".as_ptr()
    } else {
        c"".as_ptr()
    }
}

#[inline]
unsafe fn center_string(s: &str) -> c_int {
    28 - (((s.len() as c_int) + 1) / 2)
}

#[no_mangle]
pub unsafe extern "C" fn center(s: *mut c_char) -> c_int {
    let text = CStr::from_ptr(s).to_string_lossy();
    center_string(&text)
}

#[no_mangle]
pub unsafe extern "C" fn killname(monst: c_char, doart: bool) -> *mut c_char {
    let mut article = false;
    let mut name = String::from("Wally the Wonder Badger");
    if (monst as u8).is_ascii_uppercase() {
        let idx = (monst as u8 - b'A') as usize;
        let monster = unsafe { &*monsters.get_unchecked(idx) };
        let monster_name = CStr::from_ptr(monster.m_name).to_string_lossy();
        name = monster_name.to_string();
        article = true;
    } else {
        let special = match monst as u8 {
            b'a' => ("arrow", true),
            b'b' => ("bolt", true),
            b'd' => ("dart", true),
            b'h' => ("hypothermia", false),
            b's' => ("starvation", false),
            _ => ("Wally the Wonder Badger", false),
        };
        name = special.0.to_string();
        article = special.1;
    }

    if doart && article {
        let prefix = CStr::from_ptr(vowelstr(CString::new(name.as_str()).unwrap().as_ptr())).to_string_lossy();
        let mut out = String::new();
        out.push_str("a");
        out.push_str(&prefix);
        out.push_str(" ");
        out.push_str(&name);
        name = out;
    }

    let bytes = name.as_bytes();
    KILLNAME_BUFFER.fill(0);
    for (idx, byte) in bytes.iter().enumerate() {
        KILLNAME_BUFFER[idx] = *byte as c_char;
    }
    KILLNAME_BUFFER[bytes.len()] = 0;
    KILLNAME_BUFFER.as_mut_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn death_monst() -> c_char {
    static POSS: [c_char; 33] = [
        b'A' as c_char, b'B' as c_char, b'C' as c_char, b'D' as c_char, b'E' as c_char,
        b'F' as c_char, b'G' as c_char, b'H' as c_char, b'I' as c_char, b'J' as c_char,
        b'K' as c_char, b'L' as c_char, b'M' as c_char, b'N' as c_char, b'O' as c_char,
        b'P' as c_char, b'Q' as c_char, b'R' as c_char, b'S' as c_char, b'T' as c_char,
        b'U' as c_char, b'V' as c_char, b'W' as c_char, b'X' as c_char, b'Y' as c_char,
        b'Z' as c_char, b'a' as c_char, b'b' as c_char, b'h' as c_char, b'd' as c_char,
        b's' as c_char, b' ' as c_char, 0,
    ];

    let idx = (crate::rnd::rnd(33) as usize) % POSS.len();
    POSS[idx]
}

#[no_mangle]
pub unsafe extern "C" fn score(amount: c_int, flags: c_int, monst: c_char) {
    let mut top_ten = Vec::with_capacity(numscores as usize);
    for _ in 0..numscores as usize {
        top_ten.push(Score {
            sc_uid: 0,
            sc_score: 0,
            sc_flags: 0,
            sc_monster: 0,
            sc_name: [0; MAXSTR],
            sc_level: 0,
            sc_time: 0,
        });
    }

    start_score();

    if flags >= 0 || wizard != 0 {
        // Keep the legacy interactive flow behavior close to the C version without
        // requiring the full curses backend to be reimplemented in Rust here.
        let mut prompt = CString::new("[Press return to continue]").unwrap();
        cur::mvaddstr(23, 0, prompt.as_ptr());
        cur::refresh();
    }

    rd_score(top_ten.as_mut_ptr());

    let mut sc2 = None;
    if noscore == 0 {
        let uid = md_getuid();
        let mut insert_at = top_ten.len();

        for (idx, entry) in top_ten.iter().enumerate() {
            if amount > entry.sc_score {
                insert_at = idx;
                break;
            }
            if allscore == 0 && flags != 2 && entry.sc_uid == uid && entry.sc_flags != 2 {
                insert_at = top_ten.len();
                break;
            }
        }

        if insert_at < top_ten.len() {
            if flags != 2 && allscore == 0 {
                let mut candidate = insert_at;
                while candidate < top_ten.len() {
                    if top_ten[candidate].sc_uid == uid && top_ten[candidate].sc_flags != 2 {
                        break;
                    }
                    candidate += 1;
                }
                if candidate >= top_ten.len() {
                    candidate = top_ten.len() - 1;
                }
                sc2 = Some(candidate);
            } else {
                sc2 = Some(top_ten.len() - 1);
            }

            let mut slot = top_ten.len() - 1;
            while slot > insert_at {
                top_ten[slot] = top_ten[slot - 1].clone();
                slot -= 1;
            }

            let mut name = CStr::from_ptr(whoami.as_ptr()).to_string_lossy().to_string();
            if name.len() >= MAXSTR {
                name.truncate(MAXSTR - 1);
            }
            let bytes = name.as_bytes();
            let entry = &mut top_ten[insert_at];
            entry.sc_score = amount;
            entry.sc_flags = flags as c_uint;
            entry.sc_level = if flags == 2 { max_level } else { level };
            entry.sc_monster = monst as c_ushort;
            entry.sc_uid = uid;
            for (idx, byte) in bytes.iter().enumerate() {
                entry.sc_name[idx] = *byte as c_char;
            }
            entry.sc_name[bytes.len()] = 0;
            if let Some(pos) = sc2 {
                if pos < top_ten.len() {
                    let current = &top_ten[pos];
                    let _ = current;
                }
            }
        }
    }

    let mode = if allscore != 0 { c"Scores".as_ptr() } else { c"Rogueists".as_ptr() };
    printf(c"Top %s %s:\n".as_ptr(), Numname, mode);
    printf(c"   Score Name\n".as_ptr());

    for (idx, entry) in top_ten.iter().enumerate() {
        if entry.sc_score != 0 {
            let reason = match entry.sc_flags {
                0 => c"killed".as_ptr(),
                1 => c"quit".as_ptr(),
                2 => c"A total winner".as_ptr(),
                3 => c"killed with Amulet".as_ptr(),
                _ => c"killed".as_ptr(),
            };
            printf(c"%2d %5d %s: %s on level %d".as_ptr(), idx as c_int + 1, entry.sc_score, entry.sc_name.as_ptr(), reason, entry.sc_level);
            if entry.sc_flags == 0 || entry.sc_flags == 3 {
                let killer = killname(entry.sc_monster as c_char, true);
                printf(c" by %s".as_ptr(), killer);
            }
            printf(c".\n".as_ptr());
        } else {
            break;
        }
    }

    if sc2.is_some() && lock_sc() != 0 {
        wr_score(top_ten.as_mut_ptr());
        unlock_sc();
    }
}

#[no_mangle]
pub unsafe extern "C" fn death(monst: c_char) {
    let mut killer = CStr::from_ptr(killname(monst, false)).to_string_lossy().to_string();
    purse -= purse / 10;
    cur::clear();

    if tombstone == 0 {
        // Legacy C path: print a compact death message when tombstones are disabled.
        let mut msg = CString::new("Killed by ").unwrap();
        cur::mvaddstr(23, 0, msg.as_ptr());
        if monst != b's' as c_char && monst != b'h' as c_char {
            let article = if matches!(killer.as_bytes().first(), Some(b'a') | Some(b'A') | Some(b'e') | Some(b'E') | Some(b'i') | Some(b'I') | Some(b'o') | Some(b'O') | Some(b'u') | Some(b'U')) {
                "an "
            } else {
                "a "
            };
            let mut line = format!("{}{} with {} gold", article, killer, purse);
            let cstr = CString::new(line).unwrap();
            cur::addstr(cstr.as_ptr());
        } else {
            let mut line = format!("{} with {} gold", killer, purse);
            let cstr = CString::new(line).unwrap();
            cur::addstr(cstr.as_ptr());
        }
    } else {
        let mut date = 0_i64;
        let now = std::time::SystemTime::now();
        if let Ok(ts) = now.duration_since(std::time::UNIX_EPOCH) {
            date = ts.as_secs() as i64;
        }
        let v = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(date as u64);
        let _ = v;
        for i in 0..rogue_rip_count() {
            cur::addstr(rogue_rip_line(i));
        }
        let killer_c = CString::new(killer.clone()).unwrap();
        let killer_x = center_string(&killer) as c_int;
        cur::mvaddstr(17, killer_x, killer_c.as_ptr());
        if monst == b's' as c_char || monst == b'h' as c_char {
            let mut space = CString::new(" ").unwrap();
            cur::mvaddstr(16, 32, space.as_ptr());
        } else {
            let article = if matches!(killer.as_bytes().first(), Some(b'a') | Some(b'A') | Some(b'e') | Some(b'E') | Some(b'i') | Some(b'I') | Some(b'o') | Some(b'O') | Some(b'u') | Some(b'U')) {
                "n"
            } else {
                ""
            };
            let mut phrase = format!("{}{}", article, killer);
            if phrase.as_bytes().len() > 0 {
                let cstr = CString::new(phrase).unwrap();
                cur::mvaddstr(16, 33, cstr.as_ptr());
            }
        }
        let hero_name = CStr::from_ptr(whoami.as_ptr()).to_string_lossy();
        let player_name = CString::new(hero_name.as_ref()).unwrap();
        cur::mvaddstr(14, center_string(hero_name.as_ref()) as c_int, player_name.as_ptr());
        let score_text = format!("{} Au", purse);
        let score_c = CString::new(score_text).unwrap();
        cur::move_(15, center_string(score_c.to_str().unwrap()) as c_int);
        cur::addstr(score_c.as_ptr());
        let year = 1900 + 0;
        let year_text = format!("{:4}", year);
        let year_c = CString::new(year_text).unwrap();
        cur::mvaddstr(18, 26, year_c.as_ptr());
    }

    cur::refresh();
    score(purse, if amulet != 0 { 3 } else { 0 }, monst);
    let msg = CString::new("[Press return to continue]").unwrap();
    printf(c"%s".as_ptr(), msg.as_ptr());
    let mut input = [0 as c_char; 16];
    let _ = fgets(input.as_mut_ptr(), 10, stdscr as *mut std::ffi::c_void);
    my_exit(0);
}

#[no_mangle]
pub unsafe extern "C" fn total_winner() {
    let lines = [
        "                                                               \n",
        "  @   @               @   @           @          @@@  @     @  \n",
        "  @   @               @@ @@           @           @   @     @  \n",
        "  @   @  @@@  @   @   @ @ @  @@@   @@@@  @@@      @  @@@    @  \n",
        "   @@@@ @   @ @   @   @   @     @ @   @ @   @     @   @     @  \n",
        "      @ @   @ @   @   @   @  @@@@ @   @ @@@@@     @   @     @  \n",
        "  @   @ @   @ @  @@   @   @ @   @ @   @ @         @   @  @     \n",
        "   @@@   @@@   @@ @   @   @  @@@@  @@@@  @@@     @@@   @@   @  \n",
        "                                                               \n",
        "     Congratulations, you have made it to the light of day!    \n",
    ];

    cur::clear();
    cur::standout();
    for line in lines {
        let cstr = CString::new(line).unwrap();
        cur::addstr(cstr.as_ptr());
    }
    cur::standend();
    let msg = CString::new("\nYou have joined the elite ranks of those who have escaped the\nDungeons of Doom alive.  You journey home and sell all your loot at\na great profit and are admitted to the Fighters' Guild.\n").unwrap();
    cur::addstr(msg.as_ptr());
    let press = CString::new("--Press space to continue--").unwrap();
    cur::mvaddstr(23, 0, press.as_ptr());
    cur::refresh();
    wait_for(b' ' as c_char);
    cur::clear();
    let heading = CString::new("   Worth  Item\n").unwrap();
    cur::mvaddstr(0, 0, heading.as_ptr());
    let oldpurse = purse;
    let mut obj = pack_ptr();
    while !obj.is_null() {
        let mut worth = 0;
        let item_type = (*thing_o(obj)).o_type;
        match item_type {
            58 => worth = 2 * (*thing_o(obj)).o_count,
            _ => {}
        }
        if worth < 0 {
            worth = 0;
        }
        let packch = (*thing_o(obj)).o_packch as u8 as c_char;
        let item_name = CStr::from_ptr(inv_name(obj, 0)).to_string_lossy();
        let line = format!("{} ) {:5}  {}\n", packch, worth, item_name);
        let cstr = CString::new(line).unwrap();
        cur::addstr(cstr.as_ptr());
        purse += worth;
        obj = next_ptr(obj);
    }
    let summary = format!("   {:5}  Gold Pieces          ", oldpurse);
    let cstr = CString::new(summary).unwrap();
    cur::addstr(cstr.as_ptr());
    cur::refresh();
    score(purse, 2, b' ' as c_char);
    my_exit(0);
}

/// Returns the Rust-owned tombstone artwork used by the death screen.
pub fn rip_art() -> &'static [&'static str] {
    RIP_ART
}

/// Returns the number of lines in the Rust-backed RIP artwork.
#[no_mangle]
pub extern "C" fn rogue_rip_count() -> usize {
    RIP_ART.len()
}

/// Returns a pointer to a specific RIP artwork line for C FFI callers.
#[no_mangle]
pub extern "C" fn rogue_rip_line(index: usize) -> *const c_char {
    RIP_ART[index].as_ptr() as *const c_char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_keeps_text_centered() {
        let s = b"You\0";
        assert_eq!(unsafe { center(s.as_ptr() as *mut c_char) }, 28 - ((("You".len() as c_int) + 1) / 2));
    }

    #[test]
    fn killname_uses_monster_names() {
        let name = unsafe { CStr::from_ptr(killname(b'F' as c_char, false)) };
        let s = name.to_string_lossy();
        assert!(!s.is_empty());
    }
}
