# C Source Directory Guide

This directory contains the core C implementation for Rogue.

## Overview

The files are grouped here by primary responsibility. Some files interact across subsystems through shared globals declared in headers at the repository root (for example `rogue.h` and `extern.h`).

## Entry and Core Flow

- `main.c`: Program entry point, top-level initialization, and main game flow startup.
- `init.c`: Early game/session initialization routines.
- `command.c`: Player command parsing and command dispatch.
- `state.c`: Core game-state transitions and state-related utilities.

## Dungeon and Movement

- `new_level.c`: Creates and initializes new dungeon levels.
- `rooms.c`: Room generation and room layout helpers.
- `passages.c`: Corridor/passage creation and related logic.
- `move.c`: Player and monster movement processing.
- `chase.c`: Monster pursuit/pathfinding behavior.

## Combat and Creatures

- `fight.c`: Combat resolution, attacks, and damage flow.
- `monsters.c`: Monster definitions, stats, and monster data helpers.
- `weapons.c`: Weapon behavior, modifiers, and related item logic.
- `armor.c`: Armor behavior and armor-specific mechanics.

## Items and Inventory

- `things.c`: Common item/object routines used across item systems.
- `pack.c`: Inventory/pack manipulation and pack-related helpers.
- `list.c`: Generic list and item list operations.
- `potions.c`: Potion effects and potion-specific handling.
- `scrolls.c`: Scroll effects and scroll-specific handling.
- `rings.c`: Ring effects and ring-specific handling.
- `sticks.c`: Wand/staff effects and stick-specific handling.

## Runtime Systems and World Updates

- `daemon.c`: Background recurring tasks (daemons/fuses) framework.
- `daemons.c`: Concrete daemon/fuse routines (hunger, healing, etc.).
- `misc.c`: Miscellaneous gameplay and map-visibility helpers.
- `options.c`: Runtime option parsing and option-state handling.

## I/O, Persistence, and UX

- `io.c`: Terminal input/output integration and screen interaction helpers.
- `save.c`: Save and restore mechanics.
- `rip.c`: Death screen and end-of-run presentation logic.
- `vers.c`: Version/build identification strings and metadata.

## Platform and OS Abstraction

- `mach_dep.c`: Machine-dependent utilities and platform-specific behavior.
- `mdport.c`: Portability wrappers and OS abstraction functions.

## Security/Crypto Support

- `xcrypt.c`: Password/crypt-related support and byte-order helpers.

## Global Definitions Bridge

- `extern.c`: Definitions for globals declared externally in headers.

## Notes for Incremental Rust Migration

For a low-risk partial rewrite, start with deterministic, low-coupling code (for example parts of `xcrypt.c`) before curses-heavy and globally-coupled gameplay files.
