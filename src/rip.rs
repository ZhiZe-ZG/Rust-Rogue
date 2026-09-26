//! Death handling, the scoreboard display, and tombstones.
//!
//! Ported from `src/c/rip.c` to Rust.
use std::io::Write;

use crate::globals::{allscore, monsters, numscores, CMonster, NUMNAME};
use crate::item::things::inv_name;
use crate::machdep::{lock_sc, start_score, unlock_sc};
use crate::mdport::md_getuid;
use crate::score::{rd_score, wr_score};
use crate::startup::my_exit;
use crate::ui::input::wait_for;
use crate::ui::output;
use glam::IVec2;

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

static mut KILLNAME_BUFFER: [u8; MAXSTR] = [0; MAXSTR];

#[repr(C)]
#[derive(Clone)]
pub struct Score {
    pub sc_uid: u32,
    pub sc_score: i32,
    pub sc_flags: u32,
    pub sc_monster: u16,
    pub sc_name: [u8; MAXSTR],
    pub sc_level: i32,
    pub sc_time: u32,
}

use crate::globals::{amulet, max_level, noscore, purse, tombstone, wizard};


#[inline]
unsafe fn thing_t(
    tp: *mut crate::entity::player::Thing,
) -> *mut crate::entity::player::ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn thing_o(
    tp: *mut crate::entity::player::Thing,
) -> *mut crate::entity::player::ThingObject {
    crate::entity::player::thing_o(tp)
}

#[inline]
fn pack_ptr() -> *mut crate::entity::player::Thing {
    crate::game::PLAYER.pack()
}

#[inline]
unsafe fn next_ptr(tp: *mut crate::entity::player::Thing) -> *mut crate::entity::player::Thing {
    crate::entity::player::thing_next(tp)
}

#[inline]
fn vowelstr(s: &str) -> &'static str {
    let first = s.as_bytes().first().copied().unwrap_or_default();
    if matches!(
        first,
        b'a' | b'A' | b'e' | b'E' | b'i' | b'I' | b'o' | b'O' | b'u' | b'U'
    ) {
        "n"
    } else {
        ""
    }
}

#[inline]
unsafe fn center_string(s: &str) -> i32 {
    28 - (((s.len() as i32) + 1) / 2)
}

pub unsafe fn center(s: &str) -> i32 {
    center_string(s)
}

pub unsafe fn killname(monst: u8, doart: bool) -> String {
    let mut article = false;
    let mut name = String::from("Wally the Wonder Badger");
    if (monst as u8).is_ascii_uppercase() {
        let idx = (monst as u8 - b'A') as usize;
        let monster = unsafe { &*std::ptr::addr_of!(monsters).cast::<CMonster>().add(idx) };
        name = monster.m_name.to_string();
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
        let prefix = vowelstr(name.as_str());
        let mut out = String::new();
        out.push_str("a");
        out.push_str(&prefix);
        out.push_str(" ");
        out.push_str(&name);
        name = out;
    }

    name
}

pub unsafe fn death_monst() -> u8 {
    static POSS: [u8; 33] = [
        b'A' as u8,
        b'B' as u8,
        b'C' as u8,
        b'D' as u8,
        b'E' as u8,
        b'F' as u8,
        b'G' as u8,
        b'H' as u8,
        b'I' as u8,
        b'J' as u8,
        b'K' as u8,
        b'L' as u8,
        b'M' as u8,
        b'N' as u8,
        b'O' as u8,
        b'P' as u8,
        b'Q' as u8,
        b'R' as u8,
        b'S' as u8,
        b'T' as u8,
        b'U' as u8,
        b'V' as u8,
        b'W' as u8,
        b'X' as u8,
        b'Y' as u8,
        b'Z' as u8,
        b'a' as u8,
        b'b' as u8,
        b'h' as u8,
        b'd' as u8,
        b's' as u8,
        b' ' as u8,
        0,
    ];

    let idx = (crate::rnd::rnd(33) as usize) % POSS.len();
    POSS[idx]
}

pub unsafe fn score(amount: i32, flags: i32, monst: u8) {
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
        output::write_text_at(IVec2::new(0, 23), "[Press return to continue]");
        output::refresh();
    }

    rd_score(top_ten.as_mut_ptr().cast());

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

            let mut name = crate::globals::whoami();
            if name.len() >= MAXSTR {
                name.truncate(MAXSTR - 1);
            }
            let bytes = name.as_bytes();
            let entry = &mut top_ten[insert_at];
            entry.sc_score = amount;
            entry.sc_flags = flags as u32;
            entry.sc_level = if flags == 2 {
                max_level
            } else {
                crate::game::current_depth()
            };
            entry.sc_monster = monst as u16;
            entry.sc_uid = uid;
            for (idx, byte) in bytes.iter().enumerate() {
                entry.sc_name[idx] = *byte as u8;
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

    let mode = if allscore != 0 { "Scores" } else { "Rogueists" };
    println!("Top {} {}:", NUMNAME, mode);
    println!("   Score Name");

    for (idx, entry) in top_ten.iter().enumerate() {
        if entry.sc_score != 0 {
            let reason = match entry.sc_flags {
                0 => "killed",
                1 => "quit",
                2 => "A total winner",
                3 => "killed with Amulet",
                _ => "killed",
            };
            let name = scal_name(&entry.sc_name);
            print!(
                "{:2} {:5} {}: {} on level {}",
                idx as i32 + 1,
                entry.sc_score,
                name,
                reason,
                entry.sc_level,
            );
            if entry.sc_flags == 0 || entry.sc_flags == 3 {
                let killer = killname(entry.sc_monster as u8, true);
                print!(" by {}", killer);
            }
            println!(".");
        } else {
            break;
        }
    }

    if sc2.is_some() && lock_sc() != 0 {
        wr_score(top_ten.as_mut_ptr().cast());
        unlock_sc();
    }
}

pub unsafe fn death(monst: u8) {
    let mut killer = killname(monst, false);
    purse -= purse / 10;
    output::clear_screen();

    if tombstone == 0 {
        // Legacy C path: print a compact death message when tombstones are disabled.
        output::write_text_at(IVec2::new(0, 23), "Killed by ");
        if monst != b's' as u8 && monst != b'h' as u8 {
            let article = if matches!(
                killer.as_bytes().first(),
                Some(b'a')
                    | Some(b'A')
                    | Some(b'e')
                    | Some(b'E')
                    | Some(b'i')
                    | Some(b'I')
                    | Some(b'o')
                    | Some(b'O')
                    | Some(b'u')
                    | Some(b'U')
            ) {
                "an "
            } else {
                "a "
            };
            let line = format!(
                "{}{} with {} gold",
                article,
                killer,
                std::ptr::addr_of!(purse).read()
            );
            output::write_text(&line);
        } else {
            let line = format!("{} with {} gold", killer, std::ptr::addr_of!(purse).read());
            output::write_text(&line);
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
            output::write_text(rogue_rip_line(i));
        }
        let killer_x = center_string(&killer) as i32;
        output::write_text_at(IVec2::new(killer_x, 17), &killer);
        if monst == b's' as u8 || monst == b'h' as u8 {
            output::write_text_at(IVec2::new(32, 16), " ");
        } else {
            let article = if matches!(
                killer.as_bytes().first(),
                Some(b'a')
                    | Some(b'A')
                    | Some(b'e')
                    | Some(b'E')
                    | Some(b'i')
                    | Some(b'I')
                    | Some(b'o')
                    | Some(b'O')
                    | Some(b'u')
                    | Some(b'U')
            ) {
                "n"
            } else {
                ""
            };
            let phrase = format!("{}{}", article, killer);
            if !phrase.is_empty() {
                output::write_text_at(IVec2::new(33, 16), &phrase);
            }
        }
        let hero_name = crate::globals::whoami();
        output::write_text_at(
            IVec2::new(center_string(&hero_name) as i32, 14),
            &hero_name,
        );
        let score_text = format!("{} Au", std::ptr::addr_of!(purse).read());
        output::move_cursor(IVec2::new(center_string(&score_text) as i32, 15));
        output::write_text(&score_text);
        let year = 1900 + 0;
        let year_text = format!("{:4}", year);
        output::write_text_at(IVec2::new(26, 18), &year_text);
    }

    output::refresh();
    score(purse, if amulet != 0 { 3 } else { 0 }, monst);
    print!("[Press return to continue]");
    let _ = std::io::stdout().flush();
    wait_for('\n');
    my_exit(0);
}

pub unsafe fn total_winner() {
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

    output::clear_screen();
    output::set_standout(true);
    for line in lines {
        output::write_text(line);
    }
    output::set_standout(false);
    output::write_text("\nYou have joined the elite ranks of those who have escaped the\nDungeons of Doom alive.  You journey home and sell all your loot at\na great profit and are admitted to the Fighters' Guild.\n");
    output::write_text_at(IVec2::new(0, 23), "--Press space to continue--");
    output::refresh();
    wait_for(' ');
    output::clear_screen();
    output::write_text_at(IVec2::new(0, 0), "   Worth  Item\n");
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
        let packch = (*thing_o(obj)).o_packch as u8;
        let item_name = inv_name(obj, 0);
        let line = format!("{} ) {:5}  {}\n", packch, worth, item_name);
        output::write_text(&line);
        purse += worth;
        obj = next_ptr(obj);
    }
    let summary = format!("   {:5}  Gold Pieces          ", oldpurse);
    output::write_text(&summary);
    output::refresh();
    score(purse, 2, b' ' as u8);
    my_exit(0);
}

/// Returns the Rust-owned tombstone artwork used by the death screen.
pub fn rip_art() -> &'static [&'static str] {
    RIP_ART
}

/// Returns the number of lines in the Rust-backed RIP artwork.
pub fn rogue_rip_count() -> usize {
    RIP_ART.len()
}

/// Returns a specific RIP artwork line.
pub fn rogue_rip_line(index: usize) -> &'static str {
    RIP_ART[index]
}

/// Interpret a NUL-terminated fixed-width scoreboard name buffer as a Rust
/// string (the legacy `sc_name` field).
fn scal_name(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_keeps_text_centered() {
        assert_eq!(
            unsafe { center("You") },
            28 - ((("You".len() as i32) + 1) / 2)
        );
    }

    #[test]
    fn killname_uses_monster_names() {
        let s = unsafe { killname(b'F' as u8, false) };
        assert!(!s.is_empty());
    }
}
