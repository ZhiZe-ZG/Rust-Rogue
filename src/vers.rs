//! Version and identity strings, ported from `src/c/vers.c`.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.
//!
//! The C original declared these in `vers.c` to force them to be loaded
//! before the version number, and therefore not to be written in saved
//! games.  The Rust equivalents keep the same `#[no_mangle]` C ABI so the
//! legacy save-file format and the version string used by the `?v` command
//! remain byte-for-byte identical.

use std::os::raw::c_char;
use std::sync::{Mutex, MutexGuard};

/// The release version string (`char *release` in vers.c).
///
/// Backed by an owned Rust `String` behind a `Mutex`; the save/restore layer
/// replaces it through [`set_release`], exactly like the C code reassigned the
/// pointer. Read it with [`release`] or [`with_release`].
static RELEASE: Mutex<String> = Mutex::new(String::new());

fn release_lock() -> MutexGuard<'static, String> {
    RELEASE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// The release version string (defaults to `"5.4.4"` until set).
pub fn release() -> String {
    let mut value = release_lock();
    if value.is_empty() {
        *value = "5.4.4".to_owned();
    }
    value.clone()
}

/// Run `operation` with the release version string borrowed.
pub fn with_release<R>(operation: impl FnOnce(&str) -> R) -> R {
    let mut value = release_lock();
    if value.is_empty() {
        *value = "5.4.4".to_owned();
    }
    operation(value.as_str())
}

/// Replace the release version string (used by save-file restore).
pub fn set_release(value: String) {
    *release_lock() = value;
}

/// Encryption/obfuscation string (`char encstr[]` in vers.c) used by the
/// legacy save-game identity.  Bytes match the C octal escapes exactly.
#[no_mangle]
pub static mut encstr: [c_char; 40] = [
    b'\xC0' as c_char,
    b'k' as c_char,
    b'|' as c_char,
    b'|' as c_char,
    b'`' as c_char,
    b'\xA9' as c_char,
    b'Y' as c_char,
    b'.' as c_char,
    b'\'' as c_char,
    b'\xC5' as c_char,
    b'\xD1' as c_char,
    b'\x81' as c_char,
    b'+' as c_char,
    b'\xBF' as c_char,
    b'~' as c_char,
    b'r' as c_char,
    b'"' as c_char,
    b']' as c_char,
    b'\xA0' as c_char,
    b'_' as c_char,
    b'\x93' as c_char,
    b'=' as c_char,
    b'1' as c_char,
    b'\xE1' as c_char,
    b')' as c_char,
    b'\x92' as c_char,
    b'\x8A' as c_char,
    b'\xA1' as c_char,
    b't' as c_char,
    b';' as c_char,
    b'\t' as c_char,
    b'$' as c_char,
    b'\xB8' as c_char,
    b'\xCC' as c_char,
    b'/' as c_char,
    b'<' as c_char,
    b'#' as c_char,
    b'\x81' as c_char,
    b'\xAC' as c_char,
    0,
];

/// Status-list obfuscation string (`char statlist[]` in vers.c).  Bytes
/// match the C octal escapes exactly.
#[no_mangle]
pub static mut statlist: [c_char; 38] = [
    b'\xED' as c_char,
    b'k' as c_char,
    b'l' as c_char,
    b'{' as c_char,
    b'+' as c_char,
    b'\x84' as c_char,
    b'\xAD' as c_char,
    b'\xCB' as c_char,
    b'i' as c_char,
    b'd' as c_char,
    b'J' as c_char,
    b'\xF1' as c_char,
    b'\x8C' as c_char,
    b'=' as c_char,
    b'4' as c_char,
    b':' as c_char,
    b'\xC9' as c_char,
    b'\xB9' as c_char,
    b'\xE1' as c_char,
    b'w' as c_char,
    b'K' as c_char,
    b'<' as c_char,
    b'\xCA' as c_char,
    b'\xD1' as c_char,
    b'\x8B' as c_char,
    b',' as c_char,
    b',' as c_char,
    b'7' as c_char,
    b'\xB9' as c_char,
    b'/' as c_char,
    b'R' as c_char,
    b'k' as c_char,
    b'%' as c_char,
    b'\x08' as c_char,
    b'\xCA' as c_char,
    b'\x0C' as c_char,
    b'\xA6' as c_char,
    0,
];

/// The version banner (`char version[]` in vers.c), written as the header
/// of saved games and shown by the `?v` command.  A symbol address to the
/// first byte is what `save.rs`/`restore` treat as the C `version` array.
#[no_mangle]
pub static mut version: [c_char; 28] = [
    b'r' as c_char,
    b'o' as c_char,
    b'g' as c_char,
    b'u' as c_char,
    b'e' as c_char,
    b' ' as c_char,
    b'(' as c_char,
    b'r' as c_char,
    b'o' as c_char,
    b'g' as c_char,
    b'u' as c_char,
    b'e' as c_char,
    b'f' as c_char,
    b'o' as c_char,
    b'r' as c_char,
    b'g' as c_char,
    b'e' as c_char,
    b')' as c_char,
    b' ' as c_char,
    b'0' as c_char,
    b'9' as c_char,
    b'/' as c_char,
    b'0' as c_char,
    b'5' as c_char,
    b'/' as c_char,
    b'0' as c_char,
    b'7' as c_char,
    0,
];
