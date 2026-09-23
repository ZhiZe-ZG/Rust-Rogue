//! Keyboard input policy for the terminal UI.

use crate::mdport::md_readchar;
use crate::startup::quit;
use crate::ui::{terminal, Window};

const ESCAPE: i32 = 27;

/// Read one command character, handling an interrupt as a quit request.
#[cfg(not(test))]
pub unsafe fn readchar() -> i32 {
    let ch = md_readchar();
    if ch == 3 {
        quit(0);
        return ESCAPE;
    }
    ch
}

#[cfg(test)]
pub unsafe fn readchar() -> i32 {
    ESCAPE
}

/// Wait until the requested character is entered.
///
/// Newline accepts either LF or CR to accommodate terminal conventions.
#[cfg(not(test))]
pub unsafe fn wait_for(ch: char) {
    if ch == '\n' {
        loop {
            let input = readchar();
            if input == '\n' as i32 || input == '\r' as i32 {
                break;
            }
        }
    } else {
        while readchar() != ch as i32 {}
    }
}

#[cfg(test)]
pub unsafe fn wait_for(_ch: char) {}

/// Read one raw terminal key code before game-level key translation.
pub fn read_raw_key() -> i32 {
    unsafe { terminal::getch() }
}

pub fn set_escape_delay(milliseconds: i32) {
    unsafe {
        terminal::set_escape_delay(milliseconds);
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

/// Enable or disable keypad translation for the given screen.
///
/// The terminal backend has no keypad mode, so this is a no-op retained only
/// to keep call sites explicit.
#[allow(clippy::unused_self)]
pub fn set_keypad(_window: Window, _enabled: bool) {}

pub fn set_input_timeout(tenths: i32) {
    unsafe {
        terminal::halfdelay(tenths);
    }
}

pub fn erase_key() -> u8 {
    unsafe { terminal::erasechar() }
}

pub fn kill_key() -> u8 {
    unsafe { terminal::killchar() }
}

pub fn flush_pending() {
    unsafe {
        terminal::flushinp();
    }
}