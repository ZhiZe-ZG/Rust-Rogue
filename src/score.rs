//! Scoreboard file I/O.
//!
//! Ported from `src/c/score.c` to Rust; reads and writes the legacy on-disk
//! top-ten score-file format.
use crate::ffi::{fread, fwrite, rewind};
use crate::globals::{numscores, scoreboard};
use std::os::raw::{c_char, c_int, c_uint, c_ushort};

const MAXSTR: usize = 1024;
const SCORELINE_LEN: usize = 100;

/// On-disk scoreboard entry layout (legacy score-file representation).
#[repr(C)]
pub struct Score {
    pub sc_uid: c_uint,
    pub sc_score: c_int,
    pub sc_flags: c_uint,
    pub sc_monster: c_ushort,
    pub sc_name: [u8; MAXSTR],
    pub sc_level: c_int,
    pub sc_time: c_uint,
}

/// Parses a legacy scoreline (`" uid score flags monster level time "`) into
/// the six numeric fields. Missing fields keep their previous value.
fn parse_scoreline(line: &[u8]) -> Option<(u32, i32, u32, u16, i32, u32)> {
    let text = String::from_utf8_lossy(line);
    let mut it = text.split_whitespace();
    let uid = it.next()?.parse::<u32>().ok()?;
    let score = it.next()?.parse::<i32>().ok()?;
    let flags = it.next()?.parse::<u32>().ok()?;
    let monster = it.next()?.parse::<u16>().ok()?;
    let level = it.next()?.parse::<i32>().ok()?;
    let time = u32::from_str_radix(it.next()?, 16).ok()?;
    Some((uid, score, flags, monster, level, time))
}

/// Reads the on-disk scoreboard into the caller-provided score array using the legacy file format.
#[no_mangle]
pub unsafe extern "C" fn rd_score(top_ten: *mut Score) {
    let mut scoreline = [0u8; SCORELINE_LEN];

    if scoreboard.is_null() || top_ten.is_null() {
        return;
    }

    rewind(scoreboard);

    for i in 0..numscores as usize {
        let entry = top_ten.add(i);
        let _ = fread(
            (*entry).sc_name.as_mut_ptr(),
            1,
            MAXSTR,
            scoreboard,
        );
        let _ = fread(scoreline.as_mut_ptr(), 1, SCORELINE_LEN, scoreboard);
        if let Some((uid, score, flags, monster, level, time)) = parse_scoreline(&scoreline) {
            (*entry).sc_uid = uid;
            (*entry).sc_score = score;
            (*entry).sc_flags = flags;
            (*entry).sc_monster = monster;
            (*entry).sc_level = level;
            (*entry).sc_time = time;
        }
    }

    rewind(scoreboard);
}

/// Serializes the caller-provided score array back into the legacy scoreboard file format.
#[no_mangle]
pub unsafe extern "C" fn wr_score(top_ten: *mut Score) {
    let mut scoreline = [0u8; SCORELINE_LEN];

    if scoreboard.is_null() || top_ten.is_null() {
        return;
    }

    rewind(scoreboard);

    for i in 0..numscores as usize {
        let entry = top_ten.add(i);
        scoreline.fill(0);

        let _ = fwrite((*entry).sc_name.as_ptr(), 1, MAXSTR, scoreboard);
        let text = format!(
            " {} {} {} {} {} {:x} \n",
            (*entry).sc_uid,
            (*entry).sc_score,
            (*entry).sc_flags,
            (*entry).sc_monster,
            (*entry).sc_level,
            (*entry).sc_time,
        );
        let bytes = text.as_bytes();
        let copy_len = bytes.len().min(SCORELINE_LEN);
        scoreline[..copy_len].copy_from_slice(&bytes[..copy_len]);

        let _ = fwrite(scoreline.as_ptr(), 1, SCORELINE_LEN, scoreboard);
    }

    rewind(scoreboard);
}

// Keep the `c_char` import meaningful for platforms where it is only used in
// the struct's sibling types above.
#[allow(dead_code)]
type Char = c_char;