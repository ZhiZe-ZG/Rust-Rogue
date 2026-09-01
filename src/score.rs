use crate::globals::{numscores, scoreboard};
use std::os::raw::{c_char, c_int, c_uint, c_ushort};

const MAXSTR: usize = 1024;
const SCORELINE_LEN: usize = 100;

#[repr(C)]
pub struct CFile {
    _private: [u8; 0],
}

#[repr(C)]
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
    fn rewind(stream: *mut CFile);
    fn fread(ptr: *mut u8, size: usize, n: usize, stream: *mut CFile) -> usize;
    fn fwrite(ptr: *const u8, size: usize, n: usize, stream: *mut CFile) -> usize;
    fn sscanf(buf: *const c_char, fmt: *const c_char, ...) -> c_int;
    fn snprintf(s: *mut c_char, n: usize, fmt: *const c_char, ...) -> c_int;
}

/// Reads the on-disk scoreboard into the caller-provided score array using the legacy file format.
#[no_mangle]
pub unsafe extern "C" fn rd_score(top_ten: *mut Score) {
    let mut scoreline = [0 as c_char; SCORELINE_LEN];

    if scoreboard.is_null() || top_ten.is_null() {
        return;
    }

    rewind(scoreboard);

    for i in 0..numscores as usize {
        let entry = top_ten.add(i);
        let _ = fread((*entry).sc_name.as_mut_ptr() as *mut u8, 1, MAXSTR, scoreboard);
        let _ = fread(scoreline.as_mut_ptr() as *mut u8, 1, SCORELINE_LEN, scoreboard);
        let _ = sscanf(
            scoreline.as_ptr(),
            c" %u %d %u %hu %d %x \n".as_ptr(),
            &mut (*entry).sc_uid,
            &mut (*entry).sc_score,
            &mut (*entry).sc_flags,
            &mut (*entry).sc_monster,
            &mut (*entry).sc_level,
            &mut (*entry).sc_time,
        );
    }

    rewind(scoreboard);
}

/// Serializes the caller-provided score array back into the legacy scoreboard file format.
#[no_mangle]
pub unsafe extern "C" fn wr_score(top_ten: *mut Score) {
    let mut scoreline = [0 as c_char; SCORELINE_LEN];

    if scoreboard.is_null() || top_ten.is_null() {
        return;
    }

    rewind(scoreboard);

    for i in 0..numscores as usize {
        let entry = top_ten.add(i);
        scoreline.fill(0);

        let _ = fwrite((*entry).sc_name.as_ptr() as *const u8, 1, MAXSTR, scoreboard);
        let _ = snprintf(
            scoreline.as_mut_ptr(),
            SCORELINE_LEN,
            c" %u %d %u %hu %d %x \n".as_ptr(),
            (*entry).sc_uid,
            (*entry).sc_score,
            (*entry).sc_flags,
            (*entry).sc_monster as c_uint,
            (*entry).sc_level,
            (*entry).sc_time,
        );
        let _ = fwrite(scoreline.as_ptr() as *const u8, 1, SCORELINE_LEN, scoreboard);
    }

    rewind(scoreboard);
}
