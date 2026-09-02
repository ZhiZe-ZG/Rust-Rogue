# Rogue: Exploring the Dungeons of Doom

This is a Rust rewrite of Rogue, based on the original C code forked from <https://github.com/Davidslv/rogue>. The project is now written entirely in Rust, with no C source code remaining.

> ncurses is still required to build the game.

## Build & run

```bash
cargo build --release
./target/release/rogue          # play
./target/release/rogue -d       # demo death screen
./target/release/rogue -s       # show high scores
./target/release/rogue FILE     # restore a saved game
```

## Layout

- `Cargo.toml`: Crate definition.  Produces both a library (`rogue_rust`,
  used for unit testing) and a native binary (`rogue`).
- `src/bin/rogue.rs`: Native Rust entry point.  Builds the legacy C-style
  `argv`/`envp` arrays from `std::env` and hands them to `startup::rogue_main`.
- `src/lib.rs`: Rust module root.
- `src/startup.rs`: Process startup sequence (`rogue_main`, `playit`, `quit`,
  `shell`, `my_exit`, signal handlers).
- `src/vers.rs`: Version/identity strings (`release`, `version`, `encstr`,
  `statlist`) ported byte-for-byte from the old `vers.c`.
- `src/globals.rs`: Game-global variables, all exported with the legacy
  `#[no_mangle]` symbol names.
- `src/machdep.rs` / `src/mdport.rs`: Machine-dependent helpers and
  portability wrappers (originally `mach_dep.c` / `mdport.c`).
- `src/curses.rs`: C-shape ncurses wrappers (`*mut c_void` windows, raw
  string pointers) translating to the safe `ncurses` crate.
- `src/rnd.rs`: Random number generator state and legacy RNG symbols
  (`rnd`, `set_seed`) backed by the `rand` crate.
- `src/level/`: Level generation subsystem.
  - `src/level/ffi.rs`: Level creation entrypoints (`new_level`, `find_floor`)
    and internal room/passage drawing.
  - `src/level/rooms.rs`: Rust room model (`Room`, `Structure` layout).
  - `src/level/passages.rs`: Corridor/passage digging.
  - `src/level/structure.rs`: Generic 2D tile grid (`Structure`).
  - `src/level/tile.rs`: Tile enum used by the level model.
  - `src/level/mod.rs`: Level module root and public re-exports.
- `src/game.rs`: Shared `CURRENT_LEVEL`, monster/object lists, and game state
  backing the legacy save format.
- `src/rndmove.rs`: Random movement helper used by monster/player logic.
- `src/level/trap.rs`: Trap effects and trap-side status messaging.
- `src/rip.rs`: RIP tombstone art provider and end-of-run scoreboard.
- `src/save.rs`: Save/restore and autosave entrypoints (`save_game`,
  `save_file`, `restore`, `auto_save`).
- `src/score.rs`: Scoreboard persistence (`rd_score`, `wr_score`).
- `src/rings.rs`: Ring mechanics (`ring_on`, `ring_off`, `gethand`,
  `ring_eat`, `ring_num`).
- `src/monsters.rs`: Monster behavior helpers and save checks (`randmonster`,
  `new_monster`, `wake_monster`, etc.).
- `src/potions.rs`: Potion effects and visibility/status helpers (`quaff`,
  `is_magic`, `invis_on`, `turn_see`, `seen_stairs`, `raise_level`, `do_pot`).
- `src/weapons.rs`: Weapon/projectile handling and formatting (`missile`,
  `do_motion`, `fall`, `wield`, `num`, etc.).
- `src/armor.rs`: Armor equip/unequip and pass-turn helper functions (`wear`,
  `take_off`, `waste_time`).
- `src/sticks.rs`: Wand/staff behavior and charge formatting (`fix_stick`,
  `do_zap`, `drain`, `fire_bolt`, `charge_str`).
- `src/scrolls.rs`: Scroll effects and curses removal (`read_scroll`,
  `uncurse`).
- `src/wizard.rs`: Wizard helpers and debug commands (`whatis`, `teleport`,
  `create_obj`, `show_map`) with runtime global gating instead of `#ifdef`
  guards.
- `src/player.rs`: Shared C-layout bindings and player-facing helpers used
  across modules.
- `src/command.rs`, `src/io.rs`, `src/fight.rs`, `src/chase.rs`,
  `src/daemon.rs`, `src/daemons.rs`, `src/draw.rs`, `src/misc.rs`,
  `src/options.rs`, `src/pack.rs`, `src/list.rs`, `src/things.rs`,
  `src/state.rs`: Remaining gameplay subsystems, all in Rust.

## C ABI notes

Although there is no C source left, many module boundaries still speak the
legacy C ABI (fixed symbol names, `#[repr(C)]` layout mirrors, raw pointers),
which keeps the port faithful to the original game and simplifies
save-file compatibility.