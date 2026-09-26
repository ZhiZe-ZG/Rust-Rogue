//! Scoreboard file I/O.
//!
//! Ported from `src/c/score.c` to Rust; reads and writes the legacy on-disk
//! top-ten score-file format.
use crate::globals::{numscores, scoreboard};
use std::io::{Read, Seek, SeekFrom, Write};

const MAXSTR: usize = 1024;
const SCORELINE_LEN: usize = 100;

/// On-disk scoreboard entry layout (legacy score-file representation).
#[repr(C)]
pub struct Score {
    pub sc_uid: u32,
    pub sc_score: i32,
    pub sc_flags: u32,
    pub sc_monster: u16,
    pub sc_name: [u8; MAXSTR],
    pub sc_level: i32,
    pub sc_time: u32,
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
pub unsafe fn rd_score(top_ten: *mut Score) {
    let mut scoreline = [0u8; SCORELINE_LEN];

    if top_ten.is_null() {
        return;
    }

    let Some(file) = scoreboard.as_mut() else {
        return;
    };

    let _ = file.seek(SeekFrom::Start(0));

    for i in 0..numscores as usize {
        let entry = top_ten.add(i);
        let _ = file.read_exact(std::slice::from_raw_parts_mut(
            (*entry).sc_name.as_mut_ptr(),
            MAXSTR,
        ));
        let _ = file.read_exact(&mut scoreline);
        if let Some((uid, score, flags, monster, level, time)) = parse_scoreline(&scoreline) {
            (*entry).sc_uid = uid;
            (*entry).sc_score = score;
            (*entry).sc_flags = flags;
            (*entry).sc_monster = monster;
            (*entry).sc_level = level;
            (*entry).sc_time = time;
        }
    }

    let _ = file.seek(SeekFrom::Start(0));
}

/// Serializes the caller-provided score array back into the legacy scoreboard file format.
pub unsafe fn wr_score(top_ten: *mut Score) {
    let mut scoreline = [0u8; SCORELINE_LEN];

    if top_ten.is_null() {
        return;
    }

    let Some(file) = scoreboard.as_mut() else {
        return;
    };

    let _ = file.seek(SeekFrom::Start(0));

    for i in 0..numscores as usize {
        let entry = top_ten.add(i);
        scoreline.fill(0);

        let _ = file.write_all(std::slice::from_raw_parts((*entry).sc_name.as_ptr(), MAXSTR));
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

        let _ = file.write_all(&scoreline);
    }

    let _ = file.flush();
    let _ = file.seek(SeekFrom::Start(0));
}

// Keep the `u8` import meaningful for platforms where it is only used in
// the struct's sibling types above.
#[allow(dead_code)]
type Char = u8;