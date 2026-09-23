//! Message, status, and overlay output policy for the terminal UI.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_uint};
use std::sync::Mutex;

use crate::config::GameConfig;
use crate::entity::player::{CStats, CThing, CThingMonster};
use crate::game::EQUIPMENT;
use crate::ui::input::{readchar, wait_for};
use crate::ui::terminal as cur;
use crate::ui::Window;
use glam::IVec2;

const ESCAPE: i32 = 27;
const MAXSTR: usize = 1024;
const MAXMSG: usize = GameConfig::SCREEN_COLS as usize - 9;
const STATLINE: i32 = 23;

/// Result of displaying or flushing a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageResult {
    Displayed,
    Escaped,
}

#[derive(Default)]
struct MessageState {
    pending: String,
    next_position: i32,
}

static MESSAGE_STATE: Mutex<MessageState> = Mutex::new(MessageState {
    pending: String::new(),
    next_position: 0,
});

unsafe extern "C" {
    static mut hungry_state: c_int;
    static mut huh: [c_char; MAXSTR];
    static mut max_stats: CStats;
    static mut mpos: c_int;
    static mut msg_esc: c_uchar;
    static mut player: CThing;
    static mut purse: c_int;
    static mut save_msg: c_uchar;
    static mut lower_msg: c_uchar;
    static mut stat_msg: c_uchar;
}

unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

/// Move the standard-screen cursor.
pub fn move_cursor(position: IVec2) {
    unsafe {
        cur::move_cursor(position);
    }
}

/// Write one glyph at the current cursor position.
pub fn write_glyph(glyph: char) {
    unsafe {
        cur::write_glyph(glyph);
    }
}

/// Move to a position and write one glyph.
pub fn write_glyph_at(position: IVec2, glyph: char) {
    unsafe {
        cur::write_glyph_at(position, glyph);
    }
}

/// Read the glyph currently displayed under the standard-screen cursor.
pub fn glyph_at_cursor() -> char {
    unsafe { cur::glyph_at_cursor() }
}

/// Read the glyph displayed at a position on the standard screen.
pub fn glyph_at(position: IVec2) -> char {
    unsafe { cur::glyph_at(position) }
}

/// Enable or disable standout output on the standard screen.
pub fn set_standout(enabled: bool) {
    unsafe { cur::set_standout(enabled) }
}

/// Flush pending standard-screen changes to the terminal.
pub fn refresh() {
    unsafe { cur::refresh() }
}

/// Clear the standard screen.
pub fn clear_screen() {
    unsafe { cur::clear() }
}

/// Clear from the cursor to the end of its line.
pub fn clear_to_end_of_line() {
    unsafe { cur::clear_to_end_of_line() }
}

/// Write UTF-8 text at the current cursor position.
pub fn write_text(text: &str) {
    unsafe { cur::write_text(text) }
}

/// Move to a position and write UTF-8 text.
pub fn write_text_at(position: IVec2, text: &str) {
    unsafe { cur::write_text_at(position, text) }
}

/// Control whether the terminal may leave the physical cursor after refresh.
///
/// The backend always leaves the cursor where it was; retained only for call
/// sites that mirrored the legacy `leaveok`.
pub fn set_leave_cursor(_window: Window, _enabled: bool) {}

/// Return the current cursor position.
pub fn window_cursor(_window: Window) -> IVec2 {
    unsafe { cur::cursor_pos() }
}

/// Clear the screen for the given (aliased) window.
pub fn clear_window(_window: Window) {
    clear_screen();
}

/// Move the (aliased) window cursor.
pub fn move_window_cursor(_window: Window, position: IVec2) {
    move_cursor(position);
}

/// Write one glyph to a window.
pub fn write_window_glyph(_window: Window, glyph: char) {
    write_glyph(glyph);
}

/// Write text to a window.
pub fn write_window_text(_window: Window, text: &str) {
    write_text(text);
}

/// Flush pending changes for a window.
pub fn refresh_window(_window: Window) {
    refresh();
}

/// Mark a window for repaint during its next refresh (no-op; single grid).
pub fn touch_window(_window: Window) {}

/// Request a full repaint of a window on its next refresh (no-op).
pub fn set_clear_on_refresh(_window: Window, _enabled: bool) {}

/// Enable or disable line optimization for a window (no-op).
pub fn set_line_optimization(_window: Window, _enabled: bool) {}

/// Enable or disable standout output for a window.
pub fn set_window_standout(_window: Window, enabled: bool) {
    set_standout(enabled);
}

/// Render a key byte in printable caret notation.
pub fn format_key(key: u8) -> String {
    match key {
        0x00..=0x1f => format!("^{}", (key + b'@') as char),
        0x7f => "^?".to_owned(),
        0x80..=0xff => {
            let low = key & 0x7f;
            if low < 0x20 {
                format!("M-^{}", (low + b'@') as char)
            } else {
                format!("M-{}", low as char)
            }
        }
        _ => (key as char).to_string(),
    }
}

#[cfg(not(test))]
unsafe fn append_message(text: &str) {
    let should_flush = {
        let state = MESSAGE_STATE
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        state.pending.len() + text.len() >= MAXMSG
    };
    if should_flush {
        endmsg();
    }

    let mut state = MESSAGE_STATE
        .lock()
        .unwrap_or_else(|lock| lock.into_inner());
    state.pending.push_str(text);
    state.next_position = state.pending.len() as i32;
}

#[cfg(not(test))]
unsafe fn display_message(text: &str) -> MessageResult {
    if text.is_empty() {
        move_cursor(IVec2::new(0, 0));
        clear_to_end_of_line();
        mpos = 0;
        return MessageResult::Displayed;
    }

    append_message(text);
    endmsg()
}

/// Display a Rust-formatted message, bypassing the legacy variadic `msg()`
/// shim. Returns the message result (useful for `--More--` escape detection).
#[inline]
pub unsafe fn msg_str(text: &str) -> MessageResult {
    #[cfg(not(test))]
    {
        display_message(text)
    }
    #[cfg(test)]
    {
        let _ = text;
        MessageResult::Displayed
    }
}

/// Append a Rust-formatted message segment.
#[inline]
pub unsafe fn addmsg_str(text: &str) {
    #[cfg(not(test))]
    {
        append_message(text);
    }
    #[cfg(test)]
    {
        let _ = text;
    }
}

/// Flush the pending message and handle pagination.
#[cfg(not(test))]
pub unsafe fn endmsg() -> MessageResult {
    let (mut pending, next_position) = {
        let mut state = MESSAGE_STATE
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        (std::mem::take(&mut state.pending), state.next_position)
    };

    if save_msg != false as c_uchar {
        let bytes = pending.as_bytes();
        let copy_len = bytes.len().min(MAXSTR - 1);
        std::ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), huh.as_mut_ptr(), copy_len);
        huh[copy_len] = 0;
    }

    if mpos != 0 {
        write_text_at(IVec2::new(mpos, 0), "--More--");
        refresh();

        if msg_esc == false as c_uchar {
            wait_for(' ');
        } else {
            loop {
                let ch = readchar();
                if ch == ' ' as i32 {
                    break;
                }
                if ch == ESCAPE {
                    pending.clear();
                    mpos = 0;
                    let mut state = MESSAGE_STATE
                        .lock()
                        .unwrap_or_else(|lock| lock.into_inner());
                    state.next_position = 0;
                    return MessageResult::Escaped;
                }
            }
        }
    }

    if lower_msg == false as c_uchar && pending.len() > 1 {
        if let Some(first) = pending.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
    }

    write_text_at(IVec2::new(0, 0), &pending);
    clear_to_end_of_line();
    mpos = next_position;
    let mut state = MESSAGE_STATE
        .lock()
        .unwrap_or_else(|lock| lock.into_inner());
    state.next_position = 0;
    refresh();
    MessageResult::Displayed
}

#[cfg(not(test))]
pub unsafe fn status() {
    let pstats = &mut (*thing_t(&raw mut player)).t_stats;
    let level = crate::game::current_depth();
    let max_hp = pstats.s_maxhp;
    let mut temp = if !EQUIPMENT.armor().is_null() {
        (*EQUIPMENT.armor()).o.o_arm
    } else {
        pstats.s_arm
    };

    static mut hpwidth: c_int = 0;
    static mut s_hungry: c_int = 0;
    static mut s_lvl: c_int = 0;
    static mut s_pur: c_int = -1;
    static mut s_hp: c_int = 0;
    static mut s_arm: c_int = 0;
    static mut s_str: c_uint = 0;
    static mut s_exp: c_int = 0;

    let state_name = [
        c"".as_ptr(),
        c"Hungry".as_ptr(),
        c"Weak".as_ptr(),
        c"Faint".as_ptr(),
    ];

    if s_hp == pstats.s_hpt
        && s_exp == pstats.s_exp
        && s_pur == purse
        && s_arm == temp
        && s_str == pstats.s_str
        && s_lvl == level
        && s_hungry == hungry_state
        && stat_msg == false as c_uchar
    {
        return;
    }

    s_arm = temp;
    let old_cursor = window_cursor(Window::Stdscr);
    if s_hp != max_hp {
        let mut temp_hp = max_hp;
        s_hp = max_hp;
        hpwidth = 0;
        while temp_hp != 0 {
            hpwidth += 1;
            temp_hp /= 10;
        }
    }

    s_lvl = level;
    s_pur = purse;
    s_hp = pstats.s_hpt;
    s_str = pstats.s_str;
    s_exp = pstats.s_exp;
    s_hungry = hungry_state;

    if stat_msg != false as c_uchar {
        move_cursor(IVec2::new(0, 0));
        msg_str(&format!(
            "Level: {}  Gold: {:<5}  Hp: {:>w$}({:>w$})  Str: {:>2}({})  Arm: {:<2}  Exp: {}/{}  {}",
            level,
            purse,
            pstats.s_hpt,
            max_hp,
            pstats.s_str,
            max_stats.s_str,
            10 - s_arm,
            pstats.s_lvl,
            pstats.s_exp,
            CStr::from_ptr(state_name[hungry_state as usize]).to_string_lossy(),
            w = hpwidth as usize,
        ));
    } else {
        move_cursor(IVec2::new(0, STATLINE));
        let line = format!(
            "Level: {}  Gold: {:<5}  Hp: {:>w$}({:>w$})  Str: {:>2}({})  Arm: {:<2}  Exp: {}/{}  {}",
            level,
            purse,
            pstats.s_hpt,
            max_hp,
            pstats.s_str,
            max_stats.s_str,
            10 - s_arm,
            pstats.s_lvl,
            pstats.s_exp,
            CStr::from_ptr(state_name[hungry_state as usize]).to_string_lossy(),
            w = hpwidth as usize,
        );
        write_text(&line);
    }

    clear_to_end_of_line();
    move_cursor(old_cursor);
}

#[cfg(not(test))]
pub unsafe fn show_win(message: &str) {
    let window = Window::Stdscr;
    move_window_cursor(window, IVec2::new(0, 0));
    write_window_text(window, message);
    touch_window(window);
    let hero = (*thing_t(&raw mut player)).t_pos;
    move_window_cursor(window, IVec2::new(hero.x, hero.y));
    refresh_window(window);
    wait_for(' ');
    let standard_screen = Window::Stdscr;
    set_clear_on_refresh(standard_screen, true);
    touch_window(standard_screen);
}

#[cfg(test)]
pub unsafe fn endmsg() -> MessageResult {
    MessageResult::Displayed
}

#[cfg(test)]
pub unsafe fn status() {}

#[cfg(test)]
pub unsafe fn show_win(_message: &str) {}

#[cfg(test)]
mod tests {
    use super::format_key;

    #[test]
    fn formats_control_and_meta_keys() {
        assert_eq!(format_key(b'a'), "a");
        assert_eq!(format_key(0x01), "^A");
        assert_eq!(format_key(0x7f), "^?");
        assert_eq!(format_key(0x81), "M-^A");
    }
}