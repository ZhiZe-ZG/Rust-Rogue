//! Keyboard input policy for the terminal UI.
//!
//! Raw keys come from the safe terminal backend ([`terminal::getch`], which
//! already maps crossterm keys onto curses key codes). This module applies the
//! game's input policy: translating cooked cursor/keypad codes into the classic
//! rogue movement commands (`h j k l y u b n`, plus Ctrl-modified runs) and
//! turning an interrupt into a quit request.

use crate::ui::terminal;
use crate::ui::Window;

const ESCAPE: i32 = 27;
const ERR: i32 = -1;
const CTRL_C: i32 = 3;

// Curses key codes (mirrors ncurses `keys.h`), as produced by the backend.
const KEY_DOWN: i32 = 0o402; // 258
const KEY_UP: i32 = 0o403; // 259
const KEY_LEFT: i32 = 0o404; // 260
const KEY_RIGHT: i32 = 0o405; // 261
const KEY_HOME: i32 = 0o406; // 262
const KEY_NPAGE: i32 = 0o522; // 338
const KEY_PPAGE: i32 = 0o523; // 339
const KEY_LL: i32 = 0o545; // 357
const KEY_A1: i32 = 0o534; // 348
const KEY_A3: i32 = 0o536; // 350
const KEY_B2: i32 = 0o541; // 353
const KEY_C1: i32 = 0o542; // 354
const KEY_C3: i32 = 0o544; // 356
const KEY_END: i32 = 0o550; // 360
const KEY_B1: i32 = 353;
const KEY_B3: i32 = 354;
const KEY_A2: i32 = 355;
const KEY_C2: i32 = 356;
const KEY_SUP: i32 = 337;
const KEY_SDOWN: i32 = 336;
const KEY_SEND: i32 = 0o551; // 361
const KEY_SHOME: i32 = 0o552; // 362
const KEY_SLEFT: i32 = 0o553; // 363
const KEY_SNEXT: i32 = 0o556; // 366
const KEY_SPREVIOUS: i32 = 0o557; // 367
const KEY_SRIGHT: i32 = 0o560; // 368
const KEY_EOL: i32 = 0o600; // 384

// Escape-sequence parser modes (from the legacy `md_readchar`).
const M_NORMAL: i32 = 0;
const M_ESC: i32 = 1;
const M_KEYPAD: i32 = 2;
const M_TRAIL: i32 = 3;

/// `ctrl(c)`: return the control character for `c`.
#[inline]
fn ctrl(c: char) -> i32 {
    (c as u8 & 0x1f) as i32
}

/// `ctrl_upcase(c)`: `CTRL(toupper(c))`.
#[inline]
fn ctrl_upcase(c: i32) -> i32 {
    let up = (c as u8).to_ascii_uppercase();
    ctrl(up as char)
}

/// Read one raw key, translating cursor/keypad escape sequences into the
/// classic rogue movement commands.
///
/// Several terminal keycodes collide (the legacy port reused ncurses values),
/// so the cooked-key match below intentionally has unreachable arms.
#[allow(unreachable_patterns, unused_assignments)]
fn read_raw() -> i32 {
    let mut ch;
    let mut lastch = 0;
    let mut mode = M_NORMAL;
    let mut mode2 = M_NORMAL;

    loop {
        ch = terminal::getch();

        if ch == ERR {
            // Timed out waiting for a valid sequence: flush and treat as ESC.
            mode = M_NORMAL;
            terminal::nocbreak();
            terminal::raw();
            ch = ESCAPE;
            break;
        }

        if mode == M_TRAIL {
            // msys console: '^' prefix means modified.
            if ch == '^' as i32 {
                ch = ctrl_upcase(lastch);
            }
            // cygwin/telnet: '~' suffix means normal.
            if ch == '~' as i32 {
                ch = (lastch as u8).to_ascii_lowercase() as i32;
            }
            if mode2 == M_ESC {
                ch = ctrl_upcase(ch);
            }
            break;
        }

        if mode == M_ESC {
            if ch == ESCAPE {
                mode2 = M_ESC;
                continue;
            }
            if ch == 'F' as i32 || ch == 'O' as i32 || ch == '[' as i32 {
                mode = M_KEYPAD;
                continue;
            }

            // Cygwin / PuTTY: cooked cursor keys.
            match ch {
                KEY_LEFT => ch = ctrl('H'),
                KEY_RIGHT => ch = ctrl('L'),
                KEY_UP => ch = ctrl('K'),
                KEY_DOWN => ch = ctrl('J'),
                KEY_HOME => ch = ctrl('Y'),
                KEY_PPAGE => ch = ctrl('U'),
                KEY_NPAGE => ch = ctrl('N'),
                KEY_END => ch = ctrl('B'),
                _ => {}
            }
            break;
        }

        if mode == M_KEYPAD {
            match ch {
                // Interix: shift-left/shift-right.
                0x5E => ch = ctrl('H'), // '^'
                0x24 => ch = ctrl('L'), // '$'
                // Interix: home.
                0x48 => ch = 'y' as i32, // 'H'
                // Interix: ctrl-keypad.
                1 => ch = ctrl('K'),
                2 => ch = ctrl('J'),
                3 => ch = ctrl('L'),
                4 => ch = ctrl('H'),
                263 => ch = ctrl('Y'),
                19 => ch = ctrl('U'),
                20 => ch = ctrl('N'),
                21 => ch = ctrl('B'),
                // Cygwin: keypad 5.
                0x47 => ch = '.' as i32, // 'G'
                // Cygwin: ctrl-home/page.
                0x37 => {
                    // '7'
                    lastch = 'Y' as i32;
                    mode = M_TRAIL;
                }
                0x35 => {
                    // '5'
                    lastch = 'U' as i32;
                    mode = M_TRAIL;
                }
                0x36 => {
                    // '6'
                    lastch = 'N' as i32;
                    mode = M_TRAIL;
                }
                // Win32 telnet / PuTTY: home/end.
                0x31 => {
                    // '1'
                    lastch = 'y' as i32;
                    mode = M_TRAIL;
                }
                0x34 => {
                    // '4'
                    lastch = 'b' as i32;
                    mode = M_TRAIL;
                }
                // PuTTY ESC O sequences.
                0x44 => ch = ctrl('H'),  // 'D'
                0x43 => ch = ctrl('L'),  // 'C'
                0x41 => ch = ctrl('K'),  // 'A'
                0x42 => ch = ctrl('J'),  // 'B'
                0x74 => ch = 'h' as i32, // 't'
                0x76 => ch = 'l' as i32, // 'v'
                0x78 => ch = 'k' as i32, // 'x'
                0x72 => ch = 'j' as i32, // 'r'
                0x77 => ch = 'y' as i32, // 'w'
                0x79 => ch = 'u' as i32, // 'y'
                0x73 => ch = 'n' as i32, // 's'
                0x71 => ch = 'b' as i32, // 'q'
                0x75 => ch = '.' as i32, // 'u'
                _ => {}
            }

            if mode != M_KEYPAD {
                continue;
            }
        }

        if ch == ESCAPE {
            set_input_timeout(1);
            mode = M_ESC;
            continue;
        }

        // Handle cooked curses keys.
        match ch {
            KEY_LEFT => ch = 'h' as i32,
            KEY_DOWN => ch = 'j' as i32,
            KEY_UP => ch = 'k' as i32,
            KEY_RIGHT => ch = 'l' as i32,
            KEY_HOME => ch = 'y' as i32,
            KEY_PPAGE => ch = 'u' as i32,
            KEY_END => ch = 'b' as i32,
            KEY_LL => ch = 'b' as i32,
            KEY_NPAGE => ch = 'n' as i32,
            KEY_B1 => ch = 'h' as i32,
            KEY_C2 => ch = 'j' as i32,
            KEY_A2 => ch = 'k' as i32,
            KEY_B3 => ch = 'l' as i32,
            KEY_A1 => ch = 'y' as i32,
            KEY_A3 => ch = 'u' as i32,
            KEY_C1 => ch = 'b' as i32,
            KEY_C3 => ch = 'n' as i32,
            // next should be '.', but there is a problem with putty/linux
            KEY_B2 => ch = 'u' as i32,
            KEY_SRIGHT => ch = ctrl('L'),
            KEY_SLEFT => ch = ctrl('H'),
            KEY_SUP => ch = ctrl('K'),
            KEY_SDOWN => ch = ctrl('J'),
            KEY_SHOME => ch = ctrl('Y'),
            KEY_SPREVIOUS => ch = ctrl('U'),
            KEY_SEND => ch = ctrl('B'),
            KEY_SNEXT => ch = ctrl('N'),
            0x146 => ch = ctrl('K'),
            0x145 => ch = ctrl('J'),
            KEY_EOL => ch = ctrl('B'),
            _ => {}
        }

        break;
    }

    terminal::nocbreak();
    terminal::raw();

    ch & 0x7F
}

/// Read one command character, handling an interrupt as a quit request.
#[cfg(not(test))]
pub fn readchar() -> i32 {
    let ch = read_raw();
    if ch == CTRL_C {
        // `quit` is a legacy C entry point, so isolate the call in one block.
        unsafe { crate::startup::quit(0) };
        return ESCAPE;
    }
    ch
}

#[cfg(test)]
pub fn readchar() -> i32 {
    ESCAPE
}

/// Wait until the requested character is entered.
///
/// Newline accepts either LF or CR to accommodate terminal conventions.
#[cfg(not(test))]
pub fn wait_for(ch: char) {
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
pub fn wait_for(_ch: char) {}

/// Read one raw terminal key code before game-level key translation.
pub fn read_raw_key() -> i32 {
    terminal::getch()
}

pub fn set_escape_delay(milliseconds: i32) {
    terminal::set_escape_delay(milliseconds);
}

pub fn set_raw_mode(enabled: bool) {
    if enabled {
        terminal::raw();
    } else {
        terminal::nocbreak();
    }
}

pub fn set_echo(enabled: bool) {
    if enabled {
        terminal::echo();
    } else {
        terminal::noecho();
    }
}

/// Enable or disable keypad translation for the given screen.
///
/// The terminal backend has no keypad mode, so this is a no-op retained only
/// to keep call sites explicit.
#[allow(clippy::unused_self)]
pub fn set_keypad(_window: Window, _enabled: bool) {}

pub fn set_input_timeout(tenths: i32) {
    terminal::halfdelay(tenths);
}

pub fn erase_key() -> u8 {
    terminal::erasechar()
}

pub fn kill_key() -> u8 {
    terminal::killchar()
}

pub fn flush_pending() {
    terminal::flushinp();
}
