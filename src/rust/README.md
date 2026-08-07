# Rust Source

This directory is reserved for incremental Rust ports of Rogue subsystems.

Suggested layout:
- `src/rust/Cargo.toml`
- `src/rust/src/lib.rs`
- `src/rust/include/` for C FFI headers

Keep the C executable as the host during early migration, and link Rust as a static library.
