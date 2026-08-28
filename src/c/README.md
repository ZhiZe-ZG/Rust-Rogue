# C Source Directory Guide

This directory contains the core C implementation for Rogue.

## Overview

The files are grouped here by primary responsibility. Some files interact across subsystems through shared globals declared in headers at the repository root (for example `rogue.h` and `extern.h`).

## Entry and Core Flow

- `main.c`: Program entry point, top-level initialization, and main game flow startup.
- `init.c`: Early game/session initialization routines.
- `command.rs`: Player command parsing and command dispatch, hosted in the Rust FFI layer at `src/rust/src/command.rs`.
- `state.c`: Core game-state transitions and state-related utilities.

## Dungeon and Movement

- `rooms.c`: Room generation and room layout helpers.
- `passages.c`: Corridor/passage creation and related logic.
- `move.c`: Player and monster movement processing.
- `chase.c`: Monster pursuit/pathfinding behavior.

## Combat and Creatures

- `fight.c`: Combat resolution, attacks, and damage flow.

Monster behavior helpers are implemented in the Rust FFI module at `src/rust/src/monsters.rs`.
Weapon behavior helpers are implemented in the Rust FFI module at `src/rust/src/weapons.rs`.
Armor behavior helpers are implemented in the Rust FFI module at `src/rust/src/armor.rs`.

## Items and Inventory

- `things.rs`: Common item/object routines, hosted in the Rust FFI layer.
- `pack.c`: Inventory/pack manipulation and pack-related helpers.
- `list.c`: Generic list and item list operations.
- `potions.c`: Replaced by the Rust FFI module at `src/rust/src/potions.rs`.
- `scrolls.c`: Replaced by the Rust FFI module at `src/rust/src/scrolls.rs`.
- `sticks.c`: Replaced by the Rust FFI module at `src/rust/src/sticks.rs`.

Ring mechanics are implemented in the Rust FFI module at `src/rust/src/rings.rs`.

## Runtime Systems and World Updates

- `daemon.c`: Background recurring tasks (daemons/fuses) framework.
- `daemons.c`: Concrete daemon/fuse routines (hunger, healing, etc.).
- `misc.rs`: Miscellaneous gameplay and map-visibility helpers, hosted in the Rust FFI layer.
- `options.c`: Runtime option parsing and option-state handling.

## I/O, Persistence, and UX

- `io.c`: Terminal input/output integration and screen interaction helpers.
- `rip.c`: Death screen and end-of-run presentation logic.
- `vers.c`: Version/build identification strings and metadata.

Save/restore and score persistence entry points are implemented in the Rust FFI modules under `src/rust/src/`.

## Platform and OS Abstraction

- `mach_dep.c`: Machine-dependent utilities and platform-specific behavior.
- `mdport.c`: Portability wrappers and OS abstraction functions.

## Global Definitions Bridge

- `extern.c`: Definitions for globals declared externally in headers.

## Notes for Incremental Rust Migration

For a low-risk partial rewrite, start with deterministic, low-coupling utility code before curses-heavy and globally-coupled gameplay files.
