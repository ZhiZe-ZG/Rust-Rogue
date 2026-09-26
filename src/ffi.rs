//! Single private FFI boundary for the portable C library calls.
//!
//! `Plan.md` Stage 1.2 requires all foreign calls to be declared in one place,
//! with game logic kept on native Rust types. This module is that place: it
//! holds the libc process/signal declarations that the ported modules used to
//! re-declare locally.
//!
//! File I/O is **not** part of this boundary anymore: save, score and lock
//! files are read and written with the Rust standard library (`std::fs`,
//! `std::io`). Only process-portable C library calls live here. There are no C
//! string helpers or C string types: byte/OS boundaries are expressed with Rust
//! `u8` pointers, and all string handling happens in Rust.

use std::os::raw::{c_int, c_long};

extern "C" {
    // ── Process / signal / time ──────────────────────────────────────────
    pub fn abort() -> !;
    pub fn exit(status: c_int) -> !;
    pub fn signal(sig: c_int, handler: usize) -> usize;
    pub fn time(timer: *mut c_long) -> c_long;
}

/// Builds a NUL-terminated byte buffer from a Rust string for use at the
/// byte-oriented C boundary (`unlink`, `chmod`, `execl`, …).
pub fn to_c_bytes(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 1);
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    out
}