# Rust Rogue

This is a Rust rewrite of *Rogue: Exploring the Dungeons of Doom*, based on the original C code forked from <https://github.com/Davidslv/rogue>. The project is now written entirely in Rust, with no C source code remaining.

> ncurses is still required to build the game.

## Develop Environment

```bash
nix develop
```

## Build & run

```bash
cargo build --release
./target/release/rogue          # play
./target/release/rogue -d       # demo death screen
./target/release/rogue -s       # show high scores
./target/release/rogue FILE     # restore a saved game
```

## Rust global variable inventory

This repository still has a number of process-wide Rust globals and singleton owners. They are the current refactor targets if the goal is to move away from legacy C-style global state and make state ownership more explicit.

### Core game globals (`src/game/globals.rs`)

The main mutable game-state bucket is in `src/game/globals.rs`. It currently contains the following process-wide values. Each item is annotated with the role it plays in the legacy engine so it can be reduced or moved more intentionally.

#### Boolean / flag globals

- `allscore`: enables the all-scores display mode used by the standard build.
- `after`: toggles the post-move/status phase used by the command loop.
- `again`: tracks whether the previous command should be replayed.
- `seenstairs`: remembers whether the staircase has been seen on this level.
- `amulet`: marks whether the Amulet of Yendor has been acquired.
- `door_stop`: stops movement when a door is encountered.
- `fight_flush`: requests a fight/flush update after combat resolution.
- `firstmove`: indicates the first move in the current turn cycle.
- `got_ltc`: tracks whether the player has obtained the last letter/level clue state.
- `has_hit`: flag set when a hit occurs during combat.
- `in_shell`: whether the game is in the shell/command mode.
- `inv_describe`: controls whether inventory items are described in detail.
- `jump`: active when the player is performing a jumping move.
- `kamikaze`: enables the special kamikaze behavior.
- `lower_msg`: whether messages should be displayed in lower-case.
- `move_on`: whether movement continues after an action.
- `msg_esc`: allows the user to leave the message display.
- `passgo`: whether moving onto a tile should advance the turn.
- `playing`: global game-running flag.
- `q_comm`: indicates a quick command mode.
- `running`: whether the player is in a running movement state.
- `save_msg`: whether messages are saved to the message history.
- `see_floor`: whether floor tiles are visible or hidden.
- `stat_msg`: whether the status line is shown through the message area.
- `terse`: toggles terse output mode.
- `to_death`: ending-state flag used when the game is dying.
- `tombstone`: whether the death screen / tombstone is enabled.
- `master_mode_enabled`: wizard-mode gate for master-mode debugging.
- `dir_ch`: most-recent direction character input.
- `runch`: current running direction.
- `take`: last object-take command state.

#### Player / inventory / progress state

- `pack_used`: record of used pack slots.
- `orig_dsusp`: original suspicion value used by the game logic.
- `l_last_comm`: previous last command value.
- `l_last_dir`: previous last direction value.
- `last_comm`: most recent command.
- `last_dir`: most recent movement direction.
- `numscores`: total number of saved score entries.
- `n_objs`: number of objects in the current state.
- `ntraps`: number of traps in the current level state.
- `hungry_state`: player hunger state / status level.
- `inpack`: whether the player is carrying or interacting with inventory.
- `inv_type`: current inventory display type.
- `max_hit`: maximum hit value used by combat calculations.
- `max_level`: highest dungeon level reached.
- `mpos`: current message cursor column / pagination position.
- `no_food`: food deprivation / no-food state.
- `a_class`: armor class table used by the game.
- `count`: generic engine counter used across turn logic.
- `scoreboard`: open score file handle.
- `food_left`: remaining food units.
- `lastscore`: most recently recorded score value.
- `no_command`: flag to suppress command processing.
- `no_move`: flag to block movement.
- `purse`: current gold count.
- `quiet`: quiet mode / suppress output state.
- `vf_hit`: venus-flytrap hit counter state.
- `dnum`: dungeon depth number.
- `seed`: RNG seed.
- `e_levels`: experience thresholds per level.
- `delta`: last movement delta / offset vector.
- `oldpos`: previous player position.
- `l_last_pick`: last picked item for the previous cycle.
- `last_pick`: last picked item in the current cycle.
- `max_stats`: player max-stat template.
- `oldrp`: old room pointer / save-restore helper state.

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

### Other global singleton owners and caches

These are process-wide singletons used outside the main globals table, and they are usually the next best candidates for explicit ownership or context-scoped state:

- `PLAYER`: the live player actor and equipment owner.
- `CURRENT_LEVEL`: current dungeon level singleton.
- `MONSTER_LIST`: live monster list registry.
- `MONSTER_MAP`: per-cell monster occupancy map.
- `OBJECTS`: item arena / object storage.
- `RNG`: random-number generator state.
- `RELEASE`: release/version string.
- `statlist`: obfuscated status-list bytes.
- `version`: game version banner bytes.
- `LFD`: last file descriptor / save-file handle.
- `COUNTCH`: command counting state.
- `DIRECTION`: current input direction state.
- `NEWCOUNT`: updated count used during input processing.
- `RET`: generic return position used in random movement.
- `CH_RET`: chase-return destination.
- `THIS`: current target/position holder in movement logic.
- `TRYP`: temporary move-attempt coordinate.
- `CANSEE_TP`: visible-target coordinate used in chase logic.
- `KILLNAME_BUFFER`: buffer used to build monster/player kill names.
- `MNAME_BUF`: text buffer for monster names.
- `MNAME_INIT`: whether monster-name buffer has been initialized.
- `PRNAME_BUF`: temporary player name buffer.
- `USED`: material-use tracking array.
- `MESSAGE_STATE`: UI message renderer state.
- `GRID`: terminal rendering grid.
- `CURSOR`: current terminal cursor location.
- `STANDOUT`: standout-mode rendering flag.
- `INPUT_TIMEOUT`: terminal input timeout setting.
- `SHUTDOWN`: terminal shutdown state.
- `TERMINAL`: actual terminal/backend instance.
- `HPWIDTH`: width of the HP/status display.
- `S_HUNGRY`: cached hunger status display value.
- `S_LVL`: cached level display value.
- `S_PUR`: cached purse display value.
- `S_HP`: cached HP display value.
- `S_ARM`: cached armor display value.
- `S_STR`: cached strength display value.
- `S_EXP`: cached experience display value.

This list is not meant to be permanent; it is a checklist for future reductions and ownership cleanup.

## Layout

- `Cargo.toml`: Crate definition.  Produces both a library (`rogue_rust`,
  used for unit testing) and a native binary (`rogue`).
- `src/bin/rogue.rs`: Native Rust entry point.  Builds the legacy C-style
  `argv`/`envp` arrays from `std::env` and hands them to `startup::rogue_main`.
- `src/lib.rs`: Rust module root.
- `src/startup.rs`: Process startup sequence (`rogue_main`, `playit`, `quit`,
  `shell`, `my_exit`, signal handlers).
- `src/vers.rs`: Version/identity strings (`release`, `version`,
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
  - `src/level/passages.rs`: Corridor/passage digging.
  - `src/level/structure/`: Tile-grid container and room model
    (`structure/structure.rs` = `Structure`, `structure/room.rs` = `Room`).
  - `src/level/tile/`: Tile and trap vocabulary (`tile/tile.rs` = `Tile`,
    `tile/trap.rs` = `Trap`).
  - `src/level/mod.rs`: Level module root and public re-exports.
- `src/game.rs`: Shared `CURRENT_LEVEL`, monster/object lists, and game state
  backing the legacy save format.
- `src/rndmove.rs`: Random movement helper used by monster/player logic.
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