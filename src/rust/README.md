# Rust source notes

This directory contains the Rust static library used by the C game via FFI.

## Layout

- `Cargo.toml`: Rust crate definition for the FFI library.
- `src/lib.rs`: Rust module root that exports migrated subsystems.
- `src/rip.rs`: RIP tombstone art provider (`rogue_rip_count`, `rogue_rip_line`).
- `src/save.rs`: Save/restore and autosave entrypoints (`save_game`, `save_file`, `restore`, `auto_save`).
- `src/score.rs`: Scoreboard persistence (`rd_score`, `wr_score`).
- `src/rings.rs`: Ring mechanics (`ring_on`, `ring_off`, `gethand`, `ring_eat`, `ring_num`).
- `src/monsters.rs`: Monster behavior helpers and save checks (`randmonster`, `new_monster`, `wake_monster`, etc.).
- `src/weapons.rs`: Weapon/projectile handling and formatting (`missile`, `do_motion`, `fall`, `wield`, `num`, etc.).
- `src/armor.rs`: Armor equip/unequip and pass-turn helper functions (`wear`, `take_off`, `waste_time`).
- `src/sticks.rs`: Wand/staff behavior and charge formatting (`fix_stick`, `do_zap`, `drain`, `fire_bolt`, `charge_str`).
- `src/scrolls.rs`: Scroll effects and curses removal (`read_scroll`, `uncurse`).

## FFI contract

- Rust functions exposed to C must keep exact legacy symbol names and compatible C ABIs.
- Data layout bindings are `#[repr(C)]` and intentionally mirror the C structures/macros they replace.
- Module-local FFI declarations should stay consistent across Rust modules to avoid declaration drift.

## Build notes

- The crate is built as a static library and linked by the top-level makefiles.
- When adding a new Rust source module used by C symbols, ensure it is:
  1. Exported from `src/lib.rs`.
  2. Included in `$(RUST_LIB)` prerequisites in makefiles.
