# Rust Rogue

This is a Rust rewrite of *Rogue: Exploring the Dungeons of Doom*, based on the original C code forked from <https://github.com/Davidslv/rogue>. The game library and executable are implemented in Rust. The separate [rogomatic/README.md](rogomatic/README.md) describes the retained C Rog-O-Matic sources, which are not part of the Cargo game build.

The Rust terminal UI uses `ratatui` with the `crossterm` backend, not ncurses. Dependencies are declared in [Cargo.toml](Cargo.toml). The Nix development shell still includes ncurses, but the Rust game has no ncurses crate dependency.

## Develop Environment

```bash
nix develop
```

## Build & run

```bash
cargo build --release
./target/release/rogue          # play
./target/release/rogue --seed 12345 # start with an explicit RNG seed
./target/release/rogue FILE     # restore a saved game
./target/release/rogue --restore FILE # restore an explicit file
./target/release/rogue -r       # restore the default save file
./target/release/rogue --help   # show command-line options
cargo test --all-targets -- --test-threads=1
```

The CLI is defined in [src/command_line_parameter.rs](src/command_line_parameter.rs) using `clap`. The old `-d` and `-s` startup modes are not defined by the current CLI. Gameplay expects an interactive terminal; the rendering grid is 80 × 24.

## Rust global variable inventory

This repository still has a number of process-wide Rust globals and singleton owners. They are the current refactor targets if the goal is to move away from legacy C-style global state and make state ownership more explicit.

### Dungeon-owned state

`DUNGEON` in [src/dungeon/mod.rs](src/dungeon/mod.rs) owns the current `Level`, accessed through scoped `with_level()` / `with_level_mut()` closures, plus the live `monster_list` and `monster_map`.

It also owns atomic fields for `max_level` (deepest dungeon depth), `no_food` (food-generation counter), `ntraps`, `seenstairs`, and `amulet`. These are not standalone globals. Use methods such as `max_depth()`, `seen_stairs()`, `set_seen_stairs()`, and `has_amulet()`.

`Level` in [src/level/level.rs](src/level/level.rs) contains depth, generation identity, stairs position, rooms, passages, room graph, map, cell flags, passage links, and floor-item handles. There is no separate `dnum` global: `DUNGEON.current_depth()` reads `Level::depth`. **`seenstairs` currently belongs to `Dungeon`, not `Level`.**

The former `CURRENT_LEVEL`, `MONSTER_LIST`, and `MONSTER_MAP` singleton storage is now owned by `DUNGEON`.

### Core game globals

The remaining mutable game-state bucket is in [src/game/globals.rs](src/game/globals.rs). Many player, movement, inventory, and message scalars remain separate globals; they have not all moved into owner structs.

#### Boolean / flag globals

- `ALLSCORE`: immutable constant enabling the standard all-scores display mode.
- `after`: toggles the post-move/status phase used by the command loop.
- `again`: tracks whether the previous command should be replayed.
- `door_stop`: stops movement when a door is encountered.
- `fight_flush`: requests a fight/flush update after combat resolution.
- `firstmove`: indicates the first move in the current turn cycle.
- `got_ltc`: records whether terminal control characters were captured.
- `has_hit`: flag set when a hit occurs during combat.
- `in_shell`: whether the game is in the shell/command mode.
- `inv_describe`: controls whether inventory items are described in detail.
- `jump`: suppresses intermediate screen updates during running.
- `kamikaze`: enables the special kamikaze behavior.
- `lower_msg`: whether messages should be displayed in lower-case.
- `move_on`: whether movement continues after an action.
- `msg_esc`: allows the user to leave the message display.
- `passgo`: controls automatic passage-following during running.
- `playing`: global game-running flag.
- `q_comm`: indicates a quick command mode.
- `running`: whether the player is in a running movement state.
- `save_msg`: whether messages are saved to the message history.
- `see_floor`: whether floor tiles are visible or hidden.
- `stat_msg`: whether the status line is shown through the message area.
- `terse`: toggles terse output mode.
- `to_death`: controls repeated fighting, not whether the player is dead.
- `tombstone`: whether the death screen / tombstone is enabled.
- `master_mode_enabled`: wizard-mode gate for master-mode debugging.
- `dir_ch`: most-recent input direction, represented by `Direction`.
- `runch`: current running direction, represented by `Direction`.
- `take`: last object-take command state.

#### Player / inventory / progress state

- `pack_used`: record of used pack slots.
- `noscore`: suppresses score recording when nonzero.
- `orig_dsusp`: original terminal delayed-suspend character.
- `l_last_comm`: previous last command value.
- `l_last_dir`: previous last direction value.
- `last_comm`: most recent command.
- `last_dir`: most recent movement direction.
- `NUMSCORES`: immutable maximum number of retained scores; `NUMNAME` is its display label.
- `n_objs`: number of eligible choices during item selection.
- `hungry_state`: player hunger state / status level.
- `inpack`: count of pack contents.
- `inv_type`: current inventory display type.
- `max_hit`: maximum hit value used by combat calculations.
- `mpos`: current message cursor column / pagination position.
- `a_class`: armor class table used by the game.
- `count`: remaining command repetitions.
- `scoreboard`: open score file handle.
- `food_left`: remaining food units.
- `lastscore`: most recently recorded score value.
- `no_command`: remaining turns during which commands are suppressed.
- `no_move`: remaining turns during which movement is blocked.
- `purse`: current gold count.
- `quiet`: turn counter used for regeneration, not output volume.
- `vf_hit`: venus-flytrap hit counter state.
- `seed`: RNG seed.
- `e_levels`: experience thresholds per level.
- `delta`: last movement delta / offset vector.
- `oldpos`: previous player position.
- `l_last_pick`: last picked item for the previous cycle.
- `last_pick`: last picked item in the current cycle.
- `max_stats`: player max-stat template.
- `oldrp`: previous room index, represented by `Option<usize>`.

#### Runtime tables and arrays

- `between`: counter used to pace wandering monster generation.
- `group`: monster-group counter used during weapon initialization.
- `nh`: next hero position used by movement/save logic.
- `D_LIST`: delayed-action daemon/fuse table.
- `ROOM_GOLD`: stable per-room gold positions for chase targets.
- `monsters`: monster prototype table used for monster metadata and stats.
- `things`: top-level object category metadata.
- `arm_info`: armor item metadata and state.
- `pot_info`: potion metadata and state.
- `ring_info`: ring metadata and state.
- `scr_info`: scroll metadata and state.
- `weap_info`: weapon metadata and state.
- `ws_info`: wand/staff metadata and state.

#### String / table-backed globals

- `p_colors`: potion color names.
- `r_stones`: ring stone names.
- `ws_made`: wand/staff material names.
- `ws_type`: wand/staff kind names.
- `WHOAMI`: player name buffer.
- `FRUIT`: chosen fruit name.
- `HOME`: home-directory string.
- `FILE_NAME`: current save filename.
- `HUH`: message history buffer.
- `PRBUF`: shared scratch string buffer.
- `SCROLL_NAMES`: generated scroll names.
- `INV_T_NAMES`: inventory display style names.
- `TRAP_NAMES`: trap naming strings.

### Other singleton owners and local state

The current owners outside the main globals module are:

- `PLAYER`: the live player actor and equipment owner.
- `DUNGEON`: current level, dungeon counters, live monsters, and occupancy map.
- `OBJECTS`: item arena / object storage.
- `RNG`: random-number generator state.
- `RELEASE`: release/version string.
- `WIZARD`: private atomic wizard-mode state in [src/wizard.rs](src/wizard.rs), reached through `wizard_mode()`, `set_wizard_mode()`, and `is_wizard()`. The old public mutable `wizard` global is absent; wizard helpers remain.
- `LFD`: optional file handle used by platform locking helpers.
- `UI`: the `UiState` owner in [src/ui/terminal.rs](src/ui/terminal.rs), containing the grid, cursor, reverse-video mode, timeout, shutdown state, terminal backend, pending-render flag, message buffer, and status caches. Former standalone terminal/message/cache globals are fields of this owner.
- Exit-request atomics in [src/startup.rs](src/startup.rs) and the rendered-level cache in [src/daemon.rs](src/daemon.rs).

`CommandState` in [src/command_dispatch.rs](src/command_dispatch.rs) is local to the main loop and passed into dispatch. Its repeat/run command fields and new-count flag replace the old `COUNTCH`, `DIRECTION`, and `NEWCOUNT` statics. The `Command` and `Direction` enums replace untyped command/direction bytes in gameplay state.

Former chase-coordinate, name-buffer, random-move return, and material-use scratch globals are no longer separate process-wide owners. `STATLIST` and `VERSION` in [src/vers.rs](src/vers.rs) are immutable constants, not mutable globals.

`PLAYER` uses an `RwLock` for the hero and mutex-protected optional `ThingId` handles for equipment. `OBJECTS` owns arena storage; packs and floor-item lists use handles. These synchronization mechanisms do not make the entire legacy engine thread-safe.

## Layout

- [Cargo.toml](Cargo.toml): crate definition for the `rogue_rust` library and native `rogue` binary.
- [src/bin/rogue.rs](src/bin/rogue.rs): parse arguments with `clap` and call `startup::rogue_main`.
- [src/lib.rs](src/lib.rs): Rust module root.
- [src/command_line_parameter.rs](src/command_line_parameter.rs): CLI definitions.
- [src/startup.rs](src/startup.rs): initialization, main loop, signals, exit requests, quit, and shell escape.
- [src/command.rs](src/command.rs), [src/direction.rs](src/direction.rs): command and direction vocabulary.
- [src/command_dispatch.rs](src/command_dispatch.rs): runtime command execution and command-local bookkeeping.
- [src/game/mod.rs](src/game/mod.rs): shared game API and owner re-exports.
- [src/game/globals.rs](src/game/globals.rs): remaining shared scalars, tables, and strings.
- [src/game/player.rs](src/game/player.rs): player and equipment owner.
- [src/dungeon/mod.rs](src/dungeon/mod.rs): dungeon owner and level lifecycle.
- [src/dungeon/generation.rs](src/dungeon/generation.rs), [src/dungeon/presence.rs](src/dungeon/presence.rs): level generation and population.
- [src/dungeon/monster_list.rs](src/dungeon/monster_list.rs), [src/dungeon/monster_map.rs](src/dungeon/monster_map.rs): monster storage and occupancy.
- [src/level/level.rs](src/level/level.rs), [src/level/passages.rs](src/level/passages.rs), [src/level/roomgraph.rs](src/level/roomgraph.rs): level model, corridor generation, and room connectivity.
- [src/structure/mod.rs](src/structure/mod.rs), [src/tile/mod.rs](src/tile/mod.rs): spatial containers, rooms, passages, tiles, and traps.
- [src/entity/mod.rs](src/entity/mod.rs): actor types, stats, movement, chase, combat, and monsters.
- [src/item/mod.rs](src/item/mod.rs): item arena, inventory, armor, weapons, potions, rings, scrolls, and sticks.
- [src/ui/mod.rs](src/ui/mod.rs): terminal input, message/status output, and rendering.
- [src/save.rs](src/save.rs), [src/state.rs](src/state.rs): save-file envelope, restoration, and serialized snapshots.
- [src/rip.rs](src/rip.rs), [src/score.rs](src/score.rs): end-of-run handling, tombstone art, and score persistence.
- [src/daemon.rs](src/daemon.rs), [src/daemons.rs](src/daemons.rs): scheduled actions and timed gameplay effects.
- [src/init.rs](src/init.rs), [src/options.rs](src/options.rs), [src/config.rs](src/config.rs): initialization tables, options, and configuration.
- [src/machdep.rs](src/machdep.rs): platform and terminal-control helpers.
- [src/rnd.rs](src/rnd.rs), [src/vers.rs](src/vers.rs): randomness and version information.
- [src/wizard.rs](src/wizard.rs): wizard-mode accessors and debug helpers.

## Save format and legacy interfaces

[src/save.rs](src/save.rs) writes the header `ROGUE-RON 1\n`, followed by a Serde/RON `GameSnapshot` defined in [src/state.rs](src/state.rs). This is a Rust snapshot format, not the original C binary save format. Item references and chase targets are encoded as indices or explicit reference variants rather than serialized memory addresses.

The snapshot adapts runtime ownership to saved fields: `seenstairs`, `amulet`, `ntraps`, `max_level`, and `no_food` are saved from `DUNGEON`, while depth is stored in the level snapshot. Daemons and the `between`/`group` counters are also persisted. Terminal resources, rendering caches, open file handles, and command-loop-local bookkeeping are not. Some output-related globals, including message flags and `mpos`, are persisted; not all UI-related state is transient.

Parts of the engine still use `unsafe`, raw item pointers, and legacy-shaped helpers. These are implementation details of the port, not a requirement to link against the original C sources or a promise of C save-file compatibility.