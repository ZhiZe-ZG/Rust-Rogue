# Rust Rogue

This is a Rust rewrite of *Rogue: Exploring the Dungeons of Doom*, based on the original C code forked from <https://github.com/Davidslv/rogue>. The game library and executable are implemented in Rust. The separate [rogomatic/README.md](rogomatic/README.md) describes the retained C Rog-O-Matic sources, which are not part of the Cargo game build.

The Rust terminal UI uses `ratatui` with the `crossterm` backend, not ncurses. Dependencies are declared in [Cargo.toml](Cargo.toml). The Rust game has no ncurses crate dependency, and ncurses is not enabled in the current Nix development shell.

## Development environment

```bash
nix develop
```

The development shell in [flake.nix](flake.nix) provides Rust 1.98.0, including Rust sources and rust-analyzer. Alternatively, use a compatible installed Rust toolchain and Cargo from the repository root. The flake's package definition still references the old src/rust layout; the Cargo build below reflects the current root-level crate layout.

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

### Explicit dependencies in item and entity code

Functions in [src/item/mod.rs](src/item/mod.rs) and [src/entity/mod.rs](src/entity/mod.rs) no longer directly access mutable process-wide globals. Each dependency is passed separately through function parameters; there is no aggregate context object. This changes how state is accessed, not where the remaining globals are stored.

- Dungeon, player, monster, and object storage is supplied through `&Dungeon` or `&ThingArena` parameters.
- Random draws use a supplied `&mut Random`, rather than locking `RNG` inside these modules.
- Flags and counters such as `has_hit`, `running`, `count`, `inpack`, and `group`, and tables such as `monsters` and the item metadata arrays, are explicit parameters. UI-dependent item operations also receive their UI owner.
- Values and shared references are used where appropriate. Reentrant mutable state is passed through individual raw pointers so an exclusive reference is not held across combat, input, or daemon callbacks. Callers must keep that storage valid and avoid conflicting borrows; immutable metadata snapshots are used where callbacks could otherwise mutate a borrowed table.
- Constants such as `terse` and immutable lookup tables remain available directly. `entity::player` retains a compatibility re-export of `nh`, but does not directly read or write that global.

Callers outside these modules still obtain dependencies from the singleton owners and [src/game/globals.rs](src/game/globals.rs). External gameplay, rendering, message, and equipment helpers can still access globals internally, so passing a separate dungeon does not yet make the entire engine instance-independent or thread-safe. Parameterized `check_level()` and `chg_str()` implementations in [src/entity/player.rs](src/entity/player.rs) coexist with legacy singleton-backed versions in [src/misc.rs](src/misc.rs).

### Dungeon-owned state

`DUNGEON` in [src/dungeon/mod.rs](src/dungeon/mod.rs) owns:

- The current `Level`, accessed through scoped `with_level()` / `with_level_mut()` closures.
- The live `monster_list` and per-cell `monster_map`.
- `player`: the hero actor and its equipment, defined by `Player` in [src/game/player.rs](src/game/player.rs).
- `objects`: the generational `ThingArena` in [src/item/arena.rs](src/item/arena.rs), shared by floor items, player inventory, monster packs, and equipment handles.
- `scroll_names`: randomly generated scroll labels behind an `RwLock`, accessed through `scroll_name()`, `scroll_names()`, and their setters. Each dungeon has independent names that persist across level changes and save/restore.
- `fruit`: the customizable fruit name behind an `RwLock`, accessed through `fruit()` and `set_fruit()`. It defaults to `slime-mold`, is configured by the `fruit` option, and is saved and restored per game.
- `whoami`: the player name behind an `RwLock`, accessed through `whoami()` and `set_whoami()`. It starts empty, is configured at startup or through the `name` option, and is saved and restored per game.

It also owns atomic fields for `max_level` (deepest dungeon depth), `no_food` (food-generation counter), `ntraps`, `seenstairs`, and `amulet`. These are not standalone globals. Use methods such as `max_depth()`, `seen_stairs()`, `set_seen_stairs()`, and `has_amulet()`.

`Level` in [src/level/level.rs](src/level/level.rs) contains depth, generation identity, stairs position, rooms, passages, room graph, map, cell flags, passage links, and floor-item handles. There is no separate `dnum` global: `DUNGEON.current_depth()` reads `Level::depth`. **`seenstairs` currently belongs to `Dungeon`, not `Level`.**

The former `CURRENT_LEVEL`, `MONSTER_LIST`, `MONSTER_MAP`, `PLAYER`, and `OBJECTS` singleton storage is now owned by `DUNGEON`. Access the player and object arena through `DUNGEON.player` and `DUNGEON.objects`; there are no separate `PLAYER` or `OBJECTS` globals.

`Player` uses an `RwLock` for the lazily initialized hero and mutex-protected optional `ThingId` handles for equipment. Packs and floor-item lists also hold `ThingId` handles, not raw item pointers. `Dungeon::clear_items()` unlinks floor-item handles and removes those objects from its own arena without discarding unrelated inventory objects.

`Dungeon::empty()` creates independent level, counter, player, monster, and object storage. The engine is not yet fully instance-based: many gameplay helpers still use the `DUNGEON` singleton, including equipment helpers that resolve object handles through `DUNGEON.objects`. The locks do not make the entire legacy engine thread-safe.

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
- `has_hit`: tracks a pending abbreviated combat message so subsequent output can append to or finish it; it is not the attack-success result.
- `in_shell`: whether the game is in the shell/command mode.
- `inv_describe`: controls whether inventory items are described in detail.
- `jump`: suppresses intermediate screen updates during running.
- `kamikaze`: enables the special kamikaze behavior.
- `lower_msg`: message-priority/capitalization flag; suppresses automatic initial capitalization when set.
- `move_on`: whether movement continues after an action.
- `msg_esc`: allows the user to leave the message display.
- `passgo`: controls automatic passage-following during running.
- `playing`: global game-running flag.
- `q_comm`: quit-flow sentinel used during quit confirmation and handling.
- `running`: whether the player is in a running movement state.
- `save_msg`: whether messages are saved to the message history.
- `see_floor`: whether floor tiles are visible or hidden.
- `stat_msg`: whether the status line is shown through the message area.
- `terse`: immutable constant set to `0`; output is always verbose, not runtime-configurable.
- `to_death`: controls repeated fighting, not whether the player is dead.
- `tombstone`: whether the death screen / tombstone is enabled.
- `master_mode_enabled`: wizard-mode gate for master-mode debugging.
- `dir_ch`: most-recent input direction, represented by `Direction`.
- `runch`: current running direction, represented by `Direction`.
- `take`: last object-take command state.

#### Player / inventory / progress state

- `pack_used`: record of used pack slots.
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
- `group`: stack-group identifier counter used during weapon initialization.
- `nh`: legacy next-hero-position state retained for save compatibility; also re-exported by `entity::player`. Current movement computes its next position locally.
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
- The player name is owned by `Dungeon`, not a standalone `WHOAMI` global.
- The chosen fruit name is owned by `Dungeon`, not a standalone `FRUIT` global.
- `HOME`: home-directory string.
- `FILE_NAME`: current save filename.
- `HUH`: message history buffer.
- `PRBUF`: shared scratch string buffer.
- `INV_T_NAMES`: inventory display style names.

### Other singleton owners and local state

The current owners and other module-local statics outside the main globals module include:

- `DUNGEON`: current level, dungeon counters, live monsters, occupancy map, player, and object arena.
- `RNG`: the shared `LazyLock<Mutex<Random>>` random-number generator in [src/rnd.rs](src/rnd.rs).
- `RELEASE`: release/version string.
- `WIZARD`: private atomic wizard-mode state in [src/wizard.rs](src/wizard.rs), reached through `wizard_mode()`, `set_wizard_mode()`, and `is_wizard()`. The old public mutable `wizard` global is absent; wizard helpers remain.
- `LFD`: optional file handle used by platform locking helpers.
- `UI`: the `UiState` owner in [src/ui/terminal.rs](src/ui/terminal.rs), containing the grid, cursor, reverse-video mode, timeout, shutdown state, terminal backend, pending-render flag, message buffer, and status caches. Former standalone terminal/message/cache globals are fields of this owner.
- Exit-request atomics in [src/startup.rs](src/startup.rs) and the rendered-level cache in [src/daemon.rs](src/daemon.rs).

`CommandState` in [src/command_dispatch.rs](src/command_dispatch.rs) is local to the main loop and passed into dispatch. Its repeat/run command fields and new-count flag replace the old `COUNTCH`, `DIRECTION`, and `NEWCOUNT` statics. The `Command` and `Direction` enums replace untyped command/direction bytes in gameplay state.

Former chase-coordinate, name-buffer, random-move return, and material-use scratch globals are no longer separate process-wide owners. `STATLIST` and `VERSION` in [src/vers.rs](src/vers.rs) and the weapon initializer table `INIT_DAM` in [src/item/weapons.rs](src/item/weapons.rs) are immutable constants, not mutable globals.

### Randomness and runtime options

`Random` owns a plain `StdRng`; synchronization belongs to the shared `RNG`, not the generator's fields. `rnd()`, `roll()`, and `set_seed()` are methods taking `&mut self`. Level/maze generation, item generation, random colors, trap messages, and item/entity gameplay effects receive `&mut Random` explicitly. Callers at the application boundary lock `RNG` and pass that generator onward; code already using the borrowed generator must not lock `RNG` again.

The runtime options in [src/options.rs](src/options.rs) are `flush`, `jump`, `seefloor`, `passgo`, `tombstone`, `inven`, `name`, `fruit`, and `file`. `terse` is no longer an option.

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
- [src/game/player.rs](src/game/player.rs): dungeon-owned `Player` and `Equipment` types.
- [src/dungeon/mod.rs](src/dungeon/mod.rs): dungeon owner, including player/object storage, and level lifecycle.
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

The `trap_names` snapshot field is retained for compatibility and written with the fixed trap names. Older saved names are accepted but ignored during restoration; customized trap names in old saves no longer override `TRAP_NAMES`. A missing `trap_names` field is also accepted.

The hero, pack, and equipment are read from `DUNGEON.player`, and item records are read from `DUNGEON.objects`. Restoration allocates fresh arena handles and reconnects saved item references. The snapshot retains a `terse` field for format compatibility, but restoring it does not change the fixed verbose output mode. The saved `seed` field is not a serialized copy of the live `StdRng` state.

Score recording is no longer suppressed by wizard-mode use. The former score-suppression field is omitted from new snapshots; its presence in older saves is ignored during deserialization.

Parts of the engine still use `unsafe`, mutable statics, platform-level raw pointers, and legacy-shaped helpers. Actor and item references use handles or explicit reference variants rather than raw `Thing` pointers. These remaining legacy interfaces are implementation details of the port, not a requirement to link against the original C sources or a promise of C save-file compatibility.