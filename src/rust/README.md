# Rust source notes

This directory holds the first Rust migration steps for Rogue.

## Files

- `Cargo.toml` — Cargo package definition for the Rust side of the migration.
- `src/lib.rs` — Root module file that exposes the Rust submodules.
- `src/rip.rs` — Rust implementation of the tombstone artwork used by the death screen.
  It exposes `rip_art()` for Rust callers and `rogue_rip_count()` / `rogue_rip_line()` for C FFI.
- `src/save.rs` — Rust implementation of the save-file entrypoint.
  It bridges the selected save logic to the existing C helpers via FFI.
