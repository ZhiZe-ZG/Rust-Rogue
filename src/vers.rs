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
//! games.  The Rust equivalents use plain `u8` byte arrays so the legacy
//! save-file format and the version string used by the `?v` command remain
//! byte-for-byte identical.

use std::sync::{Mutex, MutexGuard};

/// The release version string (`char *release` in vers.c).
///
/// Backed by an owned Rust `String` behind a `Mutex`; the save/restore layer
/// replaces it through [`set_release`]. Read it with [`release`] or
/// [`with_release`].
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

/// Encryption/obfuscation bytes (`char encstr[]` in vers.c) used by the
/// legacy save-game identity.  Bytes match the C octal escapes exactly.
#[no_mangle]
pub static mut encstr: [u8; 40] = [
    0xC0, b'k', b'|', b'|', b'`', 0xA9, b'Y', b'.', b'\'', 0xC5, 0xD1, 0x81, b'+', 0xBF, b'~', b'r',
    b'"', b']', 0xA0, b'_', 0x93, b'=', b'1', 0xE1, b')', 0x92, 0x8A, 0xA1, b't', b';', b'\t', b'$',
    0xB8, 0xCC, b'/', b'<', b'#', 0x81, 0xAC, 0,
];

/// Status-list obfuscation bytes (`char statlist[]` in vers.c).  Bytes match
/// the C octal escapes exactly.
#[no_mangle]
pub static mut statlist: [u8; 38] = [
    0xED, b'k', b'l', b'{', b'+', 0x84, 0xAD, 0xCB, b'i', b'd', b'J', 0xF1, 0x8C, b'=', b'4', b':',
    0xC9, 0xB9, 0xE1, b'w', b'K', b'<', 0xCA, 0xD1, 0x8B, b',', b',', b'7', 0xB9, b'/', b'R', b'k',
    b'%', 0x08, 0xCA, 0x0C, 0xA6, 0,
];

/// The version banner (`char version[]` in vers.c), written as the header
/// of saved games and shown by the `?v` command.
#[no_mangle]
pub static mut version: [u8; 28] = *b"rogue (rogueforge) 09/05/07\0";