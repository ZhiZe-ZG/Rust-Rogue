# Rust source notes

This directory contains the Rust static library used by the C game via FFI.

The migration is incremental: C remains the stable ABI surface for the full game,
while selected subsystems are reimplemented in Rust and exported with the same
legacy symbol names where practical.

## Layout

- `Cargo.toml`: Rust crate definition for the FFI library.
- `src/lib.rs`: Rust module root that exports migrated subsystems.
- `src/rnd.rs`: Random number generator state and legacy RNG symbols (`rnd`, `set_seed`) backed by the `rand` crate.
- `src/level/`: Level generation subsystem.
  - `src/level/ffi.rs`: Level creation entrypoints (`new_level`, `find_floor`) and internal room/passage drawing.
  - `src/level/rooms.rs`: Rust room model (`Room`, `Structure` layout) and maze building.
  - `src/level/passages.rs`: Corridor/passage digging and passage numbering (internal helpers).
  - `src/level/structure.rs`: Generic 2D tile grid (`Structure`).
  - `src/level/tile.rs`: Tile enum used by the level model.
  - `src/level/mod.rs`: Level module root and public re-exports.
- `src/rndmove.rs`: Random movement helper used by monster/player logic.
- `src/trap.rs`: Trap effects and trap-side status messaging.
- `src/rip.rs`: RIP tombstone art provider (`rogue_rip_count`, `rogue_rip_line`).
- `src/save.rs`: Save/restore and autosave entrypoints (`save_game`, `save_file`, `restore`, `auto_save`).
- `src/score.rs`: Scoreboard persistence (`rd_score`, `wr_score`).
- `src/rings.rs`: Ring mechanics (`ring_on`, `ring_off`, `gethand`, `ring_eat`, `ring_num`).
- `src/monsters.rs`: Monster behavior helpers and save checks (`randmonster`, `new_monster`, `wake_monster`, etc.).
- `src/potions.rs`: Potion effects and visibility/status helpers (`quaff`, `is_magic`, `invis_on`, `turn_see`, `seen_stairs`, `raise_level`, `do_pot`).
- `src/weapons.rs`: Weapon/projectile handling and formatting (`missile`, `do_motion`, `fall`, `wield`, `num`, etc.).
- `src/armor.rs`: Armor equip/unequip and pass-turn helper functions (`wear`, `take_off`, `waste_time`).
- `src/sticks.rs`: Wand/staff behavior and charge formatting (`fix_stick`, `do_zap`, `drain`, `fire_bolt`, `charge_str`).
- `src/scrolls.rs`: Scroll effects and curses removal (`read_scroll`, `uncurse`).
- `src/wizard.rs`: Wizard helpers and debug commands (`whatis`, `teleport`, `create_obj`, `show_map`) with runtime global gating instead of `#ifdef` guards.
- `src/player.rs`: Shared C-layout bindings and player-facing helpers used across modules.

## FFI contract

- Rust functions exposed to C must keep exact legacy symbol names and compatible C ABIs.
- Data layout bindings are `#[repr(C)]` and intentionally mirror the C structures/macros they replace.
- Module-local FFI declarations should stay consistent across Rust modules to avoid declaration drift.
- Prefer keeping formatting-heavy varargs entrypoints in C shims and forwarding preformatted data into Rust.
- Keep `unsafe` at the FFI boundary where possible; game logic inside Rust modules should stay as safe and idiomatic as practical.

## Build notes

- The crate is built as a static library and linked by the top-level makefiles.
- The crate currently depends on `rand` for the shared RNG implementation used by legacy `rnd` call sites.
- When adding a new Rust source module used by C symbols, ensure it is:
  1. Exported from `src/lib.rs`.
  2. Included in `$(RUST_LIB)` prerequisites in makefiles.
