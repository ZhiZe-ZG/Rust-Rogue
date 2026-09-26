//! Single private FFI boundary for the portable C library calls.
//!
//! `Plan.md` Stage 1.2 requires all foreign calls to be declared in one place,
//! with game logic kept on native Rust types. This module is that place: it
//! holds the authoritative [`CFile`] handle and the libc process/stdio
//! declarations that the ported modules used to re-declare locally.
//!
//! Only process-portable C library calls live here. There are no C string
//! helpers or C string types: byte/OS boundaries are expressed with Rust
//! `u8` pointers, and all string handling happens in Rust.

use std::os::raw::{c_int, c_long, c_uint};

/// Opaque C stdio stream handle.
///
/// The authoritative definition; the save/score/state code shares this one
/// type instead of declaring its own copy.
#[repr(C)]
pub struct CFile {
    _private: [u8; 0],
}

extern "C" {
    // ── Process / stdio ──────────────────────────────────────────────────
    pub fn putchar(c: c_int) -> c_int;
    pub fn abort() -> !;
    pub fn exit(status: c_int) -> !;
    pub fn setbuf(stream: *mut CFile, buf: *mut u8);
    pub fn signal(sig: c_int, handler: usize) -> usize;
    pub fn getuid() -> c_uint;
    pub fn time(timer: *mut c_long) -> c_long;

    // ── File I/O (legacy save/score formats) ─────────────────────────────
    //
    // Paths/modes are passed as NUL-terminated byte buffers (`*const u8`);
    // callers build them with [`to_c_bytes`] and keep them alive for the call.
    pub fn fopen(path: *const u8, mode: *const u8) -> *mut CFile;
    pub fn fclose(stream: *mut CFile) -> c_int;
    pub fn fflush(stream: *mut CFile) -> c_int;
    pub fn rewind(stream: *mut CFile);
    pub fn fread(ptr: *mut u8, size: usize, n: usize, stream: *mut CFile) -> usize;
    pub fn fwrite(ptr: *const u8, size: usize, nmemb: usize, stream: *mut CFile) -> usize;
    pub fn access(path: *const u8, mode: c_int) -> c_int;
}

/// Builds a NUL-terminated byte buffer from a Rust string for use at the
/// byte-oriented C boundary (`fopen`, `access`, …).
pub fn to_c_bytes(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 1);
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    out
}