//! Keyboard input policy for the terminal UI.

use std::os::raw::c_int;

use crate::mdport::md_readchar;
use crate::startup::quit;
use crate::ui::{terminal, Window};

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

/// Read one raw terminal key code before game-level key translation.
pub fn read_raw_key() -> i32 {
    unsafe { terminal::getch() }
}

pub fn set_escape_delay(milliseconds: i32) {
    unsafe {
        terminal::set_escdelay(milliseconds);
    }
}

pub fn set_raw_mode(enabled: bool) {
    unsafe {
        if enabled {
            terminal::raw();
        } else {
            terminal::nocbreak();
        }
    }
}

pub fn set_echo(enabled: bool) {
    unsafe {
        if enabled {
            terminal::echo();
        } else {
            terminal::noecho();
        }
    }
}

pub fn set_keypad(window: Window, enabled: bool) {
    unsafe {
        terminal::keypad(window.as_raw(), u8::from(enabled));
    }
}

pub fn set_input_timeout(tenths: i32) {
    unsafe {
        terminal::halfdelay(tenths);
    }
}

pub fn erase_key() -> u8 {
    unsafe { terminal::erasechar() as u8 }
}

pub fn kill_key() -> u8 {
    unsafe { terminal::killchar() as u8 }
}

pub fn flush_pending() {
    unsafe {
        terminal::flushinp();
    }
}
