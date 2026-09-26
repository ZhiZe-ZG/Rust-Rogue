//! Single private FFI boundary for the portable C library calls.
//!
//! `Plan.md` Stage 1.2 requires all foreign calls to be declared in one place,
//! with game logic kept on native Rust types. This module is that place: it
//! holds the authoritative [`CFile`] handle and the libc/stdio/string/ctype
//! declarations that the ported modules used to re-declare locally (and, in the
//! case of `CFile`, three separate times).
//!
//! Only process-portable C library calls live here. The game-state globals
//! (which are Rust-internal compatibility residue) stay with their modules
//! until Stage 3 turns them into owned state, and the platform/signal layer
//! stays in `mdport`/`machdep`.

use std::os::raw::{c_char, c_int, c_long, c_uint};

/// Opaque C stdio stream handle.
///
/// The authoritative definition; the save/score/state code shares this one
/// type instead of declaring its own copy.
#[repr(C)]
pub struct CFile {
    _private: [u8; 0],
}

extern "C" {
    // ── String / ctype ───────────────────────────────────────────────────
    pub fn strlen(s: *const c_char) -> usize;
    pub fn strcmp(a: *const c_char, b: *const c_char) -> c_int;
    pub fn strncmp(a: *const c_char, b: *const c_char, n: usize) -> c_int;
    pub fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    pub fn strerror(errnum: c_int) -> *const c_char;
    pub fn snprintf(s: *mut c_char, n: usize, fmt: *const c_char, ...) -> c_int;
    pub fn sscanf(buf: *const c_char, fmt: *const c_char, ...) -> c_int;
    pub fn isalpha(c: c_int) -> c_int;
    pub fn isprint(c: c_int) -> c_int;
    pub fn isdigit(c: c_int) -> c_int;
    pub fn isupper(c: c_int) -> c_int;
    pub fn tolower(c: c_int) -> c_int;
    pub fn toupper(c: c_int) -> c_int;
    pub fn toascii(c: c_int) -> c_int;
    pub fn strchr(s: *const c_char, c: c_int) -> *mut c_char;
    pub fn atoi(s: *const c_char) -> c_int;
    pub fn sprintf(s: *mut c_char, fmt: *const c_char, ...) -> c_int;

    // ── Process / stdio ──────────────────────────────────────────────────
    pub fn putchar(c: c_int) -> c_int;
    pub fn perror(s: *const c_char);
    pub fn abort() -> !;
    pub fn exit(status: c_int) -> !;
    pub fn setbuf(stream: *mut CFile, buf: *mut c_char);
    pub fn signal(sig: c_int, handler: usize) -> usize;
    pub fn getuid() -> c_uint;
    pub fn time(timer: *mut c_long) -> c_long;

    // ── File I/O (legacy save/score formats) ─────────────────────────────
    pub fn fopen(path: *const c_char, mode: *const c_char) -> *mut CFile;
    pub fn fclose(stream: *mut CFile) -> c_int;
    pub fn fflush(stream: *mut CFile) -> c_int;
    pub fn rewind(stream: *mut CFile);
    pub fn fread(ptr: *mut u8, size: usize, n: usize, stream: *mut CFile) -> usize;
    pub fn fwrite(ptr: *const u8, size: usize, nmemb: usize, stream: *mut CFile) -> usize;
    pub fn access(path: *const c_char, mode: c_int) -> c_int;

    // ── Process stdout formatting (startup banner) ───────────────────────
    pub fn printf(fmt: *const c_char, ...) -> c_int;
    pub fn fprintf(stream: *mut CFile, fmt: *const c_char, ...) -> c_int;
    pub fn fgets(buf: *mut c_char, n: c_int, stream: *mut CFile) -> *mut c_char;
}
