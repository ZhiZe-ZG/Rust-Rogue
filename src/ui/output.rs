//! Message, status, and overlay output policy for the terminal UI.

use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::Mutex;

use crate::config::GameConfig;
use crate::game::PLAYER;
use crate::globals::{
    get_hungry_state, get_max_stats, get_mpos, get_purse, lower_msg_enabled, msg_esc_enabled,
    save_msg_enabled, set_huh_string, set_mpos, stat_msg_enabled,
};
use crate::ui::input::{readchar, wait_for};
use crate::ui::terminal as cur;
use crate::ui::Window;
use glam::IVec2;

const ESCAPE: i32 = 27;
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

fn split_message_at(text: &str, limit: usize) -> usize {
    if text.len() <= limit {
        return text.len();
    }

    text.char_indices()
        .map(|(idx, _)| idx)
        .take_while(|idx| *idx <= limit)
        .last()
        .filter(|idx| *idx > 0)
        .unwrap_or_else(|| text.chars().next().map_or(0, char::len_utf8))
}

/// Move the standard-screen cursor.
pub fn move_cursor(position: IVec2) {
    cur::move_cursor(position);
}

/// Write one glyph at the current cursor position.
pub fn write_glyph(glyph: char) {
    cur::write_glyph(glyph);
}

/// Move to a position and write one glyph.
pub fn write_glyph_at(position: IVec2, glyph: char) {
    cur::write_glyph_at(position, glyph);
}

/// Read the glyph currently displayed under the standard-screen cursor.
pub fn glyph_at_cursor() -> char {
    cur::glyph_at_cursor()
}

/// Read the glyph displayed at a position on the standard screen.
pub fn glyph_at(position: IVec2) -> char {
    cur::glyph_at(position)
}

/// Enable or disable standout output on the standard screen.
pub fn set_standout(enabled: bool) {
    cur::set_standout(enabled);
}

/// Flush pending standard-screen changes to the terminal.
pub fn refresh() {
    cur::refresh();
}

/// Clear the standard screen.
pub fn clear_screen() {
    cur::clear();
}

/// Clear from the cursor to the end of its line.
pub fn clear_to_end_of_line() {
    cur::clear_to_end_of_line();
}

/// Write UTF-8 text at the current cursor position.
pub fn write_text(text: &str) {
    cur::write_text(text);
}

/// Move to a position and write UTF-8 text.
pub fn write_text_at(position: IVec2, text: &str) {
    cur::write_text_at(position, text);
}

/// Control whether the terminal may leave the physical cursor after refresh.
///
/// The backend always leaves the cursor where it was; retained only for call
/// sites that mirrored the legacy `leaveok`.
pub fn set_leave_cursor(_window: Window, _enabled: bool) {}

/// Return the current cursor position.
pub fn window_cursor(_window: Window) -> IVec2 {
    cur::cursor_pos()
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

fn append_message(text: &str) {
    let mut remaining = text;
    while !remaining.is_empty() {
        let available = {
            let state = MESSAGE_STATE
                .lock()
                .unwrap_or_else(|lock| lock.into_inner());
            MAXMSG.saturating_sub(state.pending.len())
        };

        if available == 0 {
            endmsg();
            continue;
        }

        let split = split_message_at(remaining, available);
        let (chunk, rest) = remaining.split_at(split);
        {
            let mut state = MESSAGE_STATE
                .lock()
                .unwrap_or_else(|lock| lock.into_inner());
            state.pending.push_str(chunk);
            state.next_position = state.pending.len() as i32;
        }

        remaining = rest;
        if !remaining.is_empty() {
            endmsg();
        }
    }
}

fn display_message(text: &str) -> MessageResult {
    if text.is_empty() {
        move_cursor(IVec2::new(0, 0));
        clear_to_end_of_line();
        set_mpos(0);
        return MessageResult::Displayed;
    }

    append_message(text);
    endmsg()
}

/// Display a Rust-formatted message. Returns the message result (useful for
/// `--More--` escape detection).
#[inline]
pub fn msg_str(text: &str) -> MessageResult {
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
pub fn addmsg_str(text: &str) {
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
pub fn endmsg() -> MessageResult {
    let (mut pending, next_position) = {
        let mut state = MESSAGE_STATE
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        (std::mem::take(&mut state.pending), state.next_position)
    };

    if save_msg_enabled() {
        set_huh_string(&pending);
    }

    let mpos = get_mpos();
    if mpos != 0 {
        write_text_at(IVec2::new(mpos, 0), "--More--");
        refresh();

        if !msg_esc_enabled() {
            wait_for(' ');
        } else {
            loop {
                let ch = readchar();
                if ch == ' ' as i32 {
                    break;
                }
                if ch == ESCAPE {
                    pending.clear();
                    set_mpos(0);
                    let mut state = MESSAGE_STATE
                        .lock()
                        .unwrap_or_else(|lock| lock.into_inner());
                    state.next_position = 0;
                    return MessageResult::Escaped;
                }
            }
        }
    }

    if !lower_msg_enabled() && pending.len() > 1 {
        if let Some(first) = pending.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
    }

    write_text_at(IVec2::new(0, 0), &pending);
    clear_to_end_of_line();
    set_mpos(next_position);
    let mut state = MESSAGE_STATE
        .lock()
        .unwrap_or_else(|lock| lock.into_inner());
    state.next_position = 0;
    refresh();
    MessageResult::Displayed
}

// ─── Status line cache (single-threaded; atomics avoid `static mut`) ─────────

static HPWIDTH: AtomicI32 = AtomicI32::new(0);
static S_HUNGRY: AtomicI32 = AtomicI32::new(0);
static S_LVL: AtomicI32 = AtomicI32::new(0);
static S_PUR: AtomicI32 = AtomicI32::new(-1);
static S_HP: AtomicI32 = AtomicI32::new(0);
static S_ARM: AtomicI32 = AtomicI32::new(0);
static S_STR: AtomicU32 = AtomicU32::new(0);
static S_EXP: AtomicI32 = AtomicI32::new(0);

const STATE_NAMES: [&str; 4] = ["", "Hungry", "Weak", "Faint"];

#[cfg(not(test))]
pub fn status() {
    let pstats = PLAYER.stats();
    let level = crate::game::current_depth();
    let max_hp = pstats.max_hit_points;
    let temp = PLAYER.armor_value().unwrap_or(pstats.armor);

    let hungry_state = get_hungry_state();
    let purse = get_purse();
    let stat_msg = stat_msg_enabled();

    if S_HP.load(Ordering::Relaxed) == pstats.hit_points
        && S_EXP.load(Ordering::Relaxed) == pstats.experience
        && S_PUR.load(Ordering::Relaxed) == purse
        && S_ARM.load(Ordering::Relaxed) == temp
        && S_STR.load(Ordering::Relaxed) == pstats.strength
        && S_LVL.load(Ordering::Relaxed) == level
        && S_HUNGRY.load(Ordering::Relaxed) == hungry_state
        && !stat_msg
    {
        return;
    }

    S_ARM.store(temp, Ordering::Relaxed);
    let old_cursor = window_cursor(Window::Stdscr);
    if S_HP.load(Ordering::Relaxed) != max_hp {
        let mut temp_hp = max_hp;
        S_HP.store(max_hp, Ordering::Relaxed);
        let mut hpwidth = 0;
        while temp_hp != 0 {
            hpwidth += 1;
            temp_hp /= 10;
        }
        HPWIDTH.store(hpwidth, Ordering::Relaxed);
    }

    S_LVL.store(level, Ordering::Relaxed);
    S_PUR.store(purse, Ordering::Relaxed);
    S_HP.store(pstats.hit_points, Ordering::Relaxed);
    S_STR.store(pstats.strength, Ordering::Relaxed);
    S_EXP.store(pstats.experience, Ordering::Relaxed);
    S_HUNGRY.store(hungry_state, Ordering::Relaxed);

    let hpwidth = HPWIDTH.load(Ordering::Relaxed);
    let s_arm = S_ARM.load(Ordering::Relaxed);
    let max_stats = get_max_stats();
    let state_name = STATE_NAMES
        .get(hungry_state.max(0) as usize)
        .copied()
        .unwrap_or("");

    if stat_msg {
        move_cursor(IVec2::new(0, 0));
        msg_str(&format!(
            "Level: {}  Gold: {:<5}  Hp: {:>w$}({:>w$})  Str: {:>2}({})  Arm: {:<2}  Exp: {}/{}  {}",
            level,
            purse,
            pstats.hit_points,
            max_hp,
            pstats.strength,
            max_stats.strength,
            10 - s_arm,
            pstats.level,
            pstats.experience,
            state_name,
            w = hpwidth as usize,
        ));
    } else {
        move_cursor(IVec2::new(0, STATLINE));
        let line = format!(
            "Level: {}  Gold: {:<5}  Hp: {:>w$}({:>w$})  Str: {:>2}({})  Arm: {:<2}  Exp: {}/{}  {}",
            level,
            purse,
            pstats.hit_points,
            max_hp,
            pstats.strength,
            max_stats.strength,
            10 - s_arm,
            pstats.level,
            pstats.experience,
            state_name,
            w = hpwidth as usize,
        );
        write_text(&line);
    }

    clear_to_end_of_line();
    move_cursor(old_cursor);
}

#[cfg(not(test))]
pub fn show_win(message: &str) {
    let window = Window::Stdscr;
    move_window_cursor(window, IVec2::new(0, 0));
    write_window_text(window, message);
    touch_window(window);
    let hero = PLAYER.pos();
    move_window_cursor(window, IVec2::new(hero.x, hero.y));
    refresh_window(window);
    wait_for(' ');
    let standard_screen = Window::Stdscr;
    set_clear_on_refresh(standard_screen, true);
    touch_window(standard_screen);
}

#[cfg(test)]
pub fn endmsg() -> MessageResult {
    MessageResult::Displayed
}

#[cfg(test)]
pub fn status() {}

#[cfg(test)]
pub fn show_win(_message: &str) {}

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
