//! Message, status, and overlay output policy for the terminal UI.

use std::sync::atomic::Ordering;

#[cfg(not(test))]
use crate::config::GameConfig;
#[cfg(not(test))]
use crate::game::globals::{
    get_hungry_state, get_max_stats, get_mpos, get_purse, lower_msg_enabled, msg_esc_enabled,
    save_msg_enabled, set_huh_string, set_mpos, stat_msg_enabled,
};
#[cfg(not(test))]
use crate::game::PLAYER;
#[cfg(not(test))]
use crate::ui::input::{readchar, wait_for};
use crate::ui::state::UI;
use crate::ui::terminal as cur;
use glam::IVec2;

#[cfg(not(test))]
const ESCAPE: i32 = 27;
#[cfg(not(test))]
const MAXMSG: usize = GameConfig::SCREEN_COLS as usize - 9;
#[cfg(not(test))]
const STATLINE: i32 = 23;

/// Result of displaying or flushing a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MessageResult {
    Displayed,
    #[cfg(not(test))]
    Escaped,
}

#[cfg(not(test))]
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
pub(crate) fn move_cursor(position: IVec2) {
    cur::move_cursor(position);
}

/// Write one glyph at the current cursor position.
pub(crate) fn write_glyph(glyph: char) {
    cur::write_glyph(glyph);
}

/// Move to a position and write one glyph.
pub(crate) fn write_glyph_at(position: IVec2, glyph: char) {
    cur::write_glyph_at(position, glyph);
}

/// Read the glyph currently displayed under the standard-screen cursor.
pub(crate) fn glyph_at_cursor() -> char {
    cur::glyph_at_cursor()
}

/// Read the glyph displayed at a position on the standard screen.
pub(crate) fn glyph_at(position: IVec2) -> char {
    cur::glyph_at(position)
}

/// Enable or disable standout output on the standard screen.
pub(crate) fn set_standout(enabled: bool) {
    cur::set_standout(enabled);
}

/// Request that pending standard-screen changes be rendered at the next UI
/// daemon boundary, or before input blocks.
pub(crate) fn refresh() {
    UI.render_pending.store(true, Ordering::Release);
}

/// Render a pending frame, returning whether a frame was flushed.
pub(crate) fn render_pending() -> bool {
    if UI.render_pending.swap(false, Ordering::AcqRel) {
        cur::refresh();
        true
    } else {
        false
    }
}

/// Flush pending output before suspending the terminal or waiting for input.
pub(crate) fn flush_now() {
    UI.render_pending.store(true, Ordering::Release);
    render_pending();
}

/// Clear the standard screen.
pub(crate) fn clear_screen() {
    cur::clear();
}

/// Clear from the cursor to the end of its line.
pub(crate) fn clear_to_end_of_line() {
    cur::clear_to_end_of_line();
}

/// Write UTF-8 text at the current cursor position.
pub(crate) fn write_text(text: &str) {
    cur::write_text(text);
}

/// Move to a position and write UTF-8 text.
pub(crate) fn write_text_at(position: IVec2, text: &str) {
    cur::write_text_at(position, text);
}

/// Return the current cursor position.
pub(crate) fn window_cursor() -> IVec2 {
    cur::cursor_pos()
}

/// Clear the screen for the given (aliased) window.
pub(crate) fn clear_window() {
    clear_screen();
}

/// Move the (aliased) window cursor.
pub(crate) fn move_window_cursor(position: IVec2) {
    move_cursor(position);
}

/// Write one glyph to a window.
pub(crate) fn write_window_glyph(glyph: char) {
    write_glyph(glyph);
}

/// Write text to a window.
pub(crate) fn write_window_text(text: &str) {
    write_text(text);
}

/// Flush pending changes for a window.
pub(crate) fn refresh_window() {
    refresh();
}

/// Render a key byte in printable caret notation.
pub(crate) fn format_key(key: u8) -> String {
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
fn append_message(text: &str) {
    let mut remaining = text;
    while !remaining.is_empty() {
        let available = {
            let state = UI.message.lock().unwrap_or_else(|lock| lock.into_inner());
            MAXMSG.saturating_sub(state.pending.len())
        };

        if available == 0 {
            endmsg();
            continue;
        }

        let split = split_message_at(remaining, available);
        let (chunk, rest) = remaining.split_at(split);
        {
            let mut state = UI.message.lock().unwrap_or_else(|lock| lock.into_inner());
            state.pending.push_str(chunk);
            state.next_position = state.pending.len() as i32;
        }

        remaining = rest;
        if !remaining.is_empty() {
            endmsg();
        }
    }
}

#[cfg(not(test))]
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
pub(crate) fn msg_str(text: &str) -> MessageResult {
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
pub(crate) fn addmsg_str(text: &str) {
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
pub(crate) fn endmsg() -> MessageResult {
    let (mut pending, next_position) = {
        let mut state = UI.message.lock().unwrap_or_else(|lock| lock.into_inner());
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
                    let mut state = UI.message.lock().unwrap_or_else(|lock| lock.into_inner());
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
    let mut state = UI.message.lock().unwrap_or_else(|lock| lock.into_inner());
    state.next_position = 0;
    refresh();
    MessageResult::Displayed
}

// ─── Status line cache (single-threaded; atomics avoid `static mut`) ─────────

#[cfg(not(test))]
const STATE_NAMES: [&str; 4] = ["", "Hungry", "Weak", "Faint"];

#[cfg(not(test))]
pub(crate) fn status() {
    let pstats = PLAYER.stats();
    let level = crate::game::current_depth();
    let max_hp = pstats.max_hit_points;
    let temp = PLAYER.armor_value().unwrap_or(pstats.armor);

    let hungry_state = get_hungry_state();
    let purse = get_purse();
    let stat_msg = stat_msg_enabled();

    if UI.status_hp.load(Ordering::Relaxed) == pstats.hit_points
        && UI.status_experience.load(Ordering::Relaxed) == pstats.experience
        && UI.status_purse.load(Ordering::Relaxed) == purse
        && UI.status_armor.load(Ordering::Relaxed) == temp
        && UI.status_strength.load(Ordering::Relaxed) == pstats.strength
        && UI.status_level.load(Ordering::Relaxed) == level
        && UI.status_hungry.load(Ordering::Relaxed) == hungry_state
        && !stat_msg
    {
        return;
    }

    UI.status_armor.store(temp, Ordering::Relaxed);
    let old_cursor = window_cursor();
    if UI.status_hp.load(Ordering::Relaxed) != max_hp {
        let mut temp_hp = max_hp;
        UI.status_hp.store(max_hp, Ordering::Relaxed);
        let mut hpwidth = 0;
        while temp_hp != 0 {
            hpwidth += 1;
            temp_hp /= 10;
        }
        UI.hp_width.store(hpwidth, Ordering::Relaxed);
    }

    UI.status_level.store(level, Ordering::Relaxed);
    UI.status_purse.store(purse, Ordering::Relaxed);
    UI.status_hp.store(pstats.hit_points, Ordering::Relaxed);
    UI.status_strength.store(pstats.strength, Ordering::Relaxed);
    UI.status_experience
        .store(pstats.experience, Ordering::Relaxed);
    UI.status_hungry.store(hungry_state, Ordering::Relaxed);

    let hpwidth = UI.hp_width.load(Ordering::Relaxed);
    let s_arm = UI.status_armor.load(Ordering::Relaxed);
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
pub(crate) fn show_win(message: &str) {
    move_window_cursor(IVec2::new(0, 0));
    write_window_text(message);
    let hero = PLAYER.pos();
    move_window_cursor(IVec2::new(hero.x, hero.y));
    refresh_window();
    wait_for(' ');
}

#[cfg(test)]
pub(crate) fn endmsg() -> MessageResult {
    MessageResult::Displayed
}

#[cfg(test)]
pub(crate) fn status() {}

#[cfg(test)]
pub(crate) fn show_win(_message: &str) {}

#[cfg(test)]
mod tests {
    use super::{format_key, refresh, render_pending};

    #[test]
    fn formats_control_and_meta_keys() {
        assert_eq!(format_key(b'a'), "a");
        assert_eq!(format_key(0x01), "^A");
        assert_eq!(format_key(0x7f), "^?");
        assert_eq!(format_key(0x81), "M-^A");
    }

    #[test]
    fn refresh_requests_coalesce_until_rendered() {
        refresh();
        refresh();

        assert!(render_pending());
        assert!(!render_pending());
    }
}
