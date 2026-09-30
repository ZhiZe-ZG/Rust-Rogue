//! Keyboard input policy for the terminal UI.
//!
//! Crossterm decodes terminal escape sequences into [`KeyCode`] values. This
//! module keeps the small legacy integer adapter used by prompts and helpers,
//! while the command loop consumes complete [`KeyEvent`] values directly.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::terminal;

const ESCAPE: i32 = 27;
const ERR: i32 = -1;
const CTRL_C: i32 = 3;

const KEY_DOWN: i32 = 0o402;
const KEY_UP: i32 = 0o403;
const KEY_LEFT: i32 = 0o404;
const KEY_RIGHT: i32 = 0o405;
const KEY_HOME: i32 = 0o406;
const KEY_BACKSPACE: i32 = 0o407;
const KEY_NPAGE: i32 = 0o522;
const KEY_PPAGE: i32 = 0o523;
const KEY_END: i32 = 0o550;
pub(crate) const ERASE_KEY: u8 = 0x7f;
pub(crate) const KILL_KEY: u8 = 0x15;

fn curses_key_code(event: &KeyEvent) -> i32 {
    match event.code {
        KeyCode::Char(c) => {
            if event.modifiers.contains(KeyModifiers::CONTROL) {
                (c.to_ascii_lowercase() as u8 & 0x1f) as i32
            } else {
                c as u8 as i32
            }
        }
        KeyCode::Enter => b'\n' as i32,
        KeyCode::Esc => ESCAPE,
        KeyCode::Tab => b'\t' as i32,
        KeyCode::Backspace => KEY_BACKSPACE,
        KeyCode::Left => KEY_LEFT,
        KeyCode::Right => KEY_RIGHT,
        KeyCode::Up => KEY_UP,
        KeyCode::Down => KEY_DOWN,
        KeyCode::Home => KEY_HOME,
        KeyCode::End => KEY_END,
        KeyCode::PageUp => KEY_PPAGE,
        KeyCode::PageDown => KEY_NPAGE,
        _ => ERR,
    }
}

fn map_cooked_key(key: i32) -> i32 {
    match key {
        KEY_LEFT => b'h' as i32,
        KEY_DOWN => b'j' as i32,
        KEY_UP => b'k' as i32,
        KEY_RIGHT => b'l' as i32,
        KEY_HOME => b'y' as i32,
        KEY_PPAGE => b'u' as i32,
        KEY_END => b'b' as i32,
        KEY_NPAGE => b'n' as i32,
        _ => key & 0x7f,
    }
}

/// Read one command character for legacy prompt and helper call sites.
pub fn readchar() -> i32 {
    crate::ui::output::render_pending();
    let key = read_legacy_key();
    if key == CTRL_C {
        unsafe { crate::startup::quit() };
        return ESCAPE;
    }
    if key == ERR {
        return ESCAPE;
    }
    map_cooked_key(key)
}

fn read_legacy_key() -> i32 {
    terminal::UI
        .get_key_event()
        .map_or(ERR, |event| curses_key_code(&event))
}

/// Wait until the requested character is entered.
///
/// Newline accepts either LF or CR to accommodate terminal conventions.
pub fn wait_for(ch: char) {
    if ch == '\n' {
        while !crate::startup::exit_requested() {
            let input = readchar();
            if input == '\n' as i32 || input == '\r' as i32 {
                break;
            }
        }
    } else {
        while !crate::startup::exit_requested() && readchar() != ch as i32 {}
    }
}

/// Read a Crossterm event without flattening its key code or modifiers.
pub(crate) fn read_key_event() -> KeyEvent {
    crate::ui::output::render_pending();
    let Some(event) = terminal::UI.get_key_event() else {
        return KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    };

    if matches!(event.code, KeyCode::Char('c' | 'C'))
        && event.modifiers.contains(KeyModifiers::CONTROL)
    {
        unsafe { crate::startup::quit() };
        return KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    }

    event
}
