//! Keyboard input policy for the terminal UI.

use std::os::raw::c_int;

use crate::mdport::md_readchar;
use crate::startup::quit;

const ESCAPE: c_int = 27;

/// Read one command character, handling an interrupt as a quit request.
#[cfg(not(test))]
pub unsafe fn readchar() -> c_int {
    let ch = md_readchar();
    if ch == 3 {
        quit(0);
        return ESCAPE;
    }
    ch
}

/// Wait until the requested character is entered.
///
/// Newline accepts either LF or CR to accommodate terminal conventions.
#[cfg(not(test))]
pub unsafe fn wait_for(ch: c_int) {
    if ch == b'\n' as c_int {
        loop {
            let input = readchar();
            if input == b'\n' as c_int || input == b'\r' as c_int {
                break;
            }
        }
    } else {
        while readchar() != ch {}
    }
}

#[cfg(test)]
pub unsafe fn readchar() -> c_int {
    ESCAPE
}

#[cfg(test)]
pub unsafe fn wait_for(_ch: c_int) {}
