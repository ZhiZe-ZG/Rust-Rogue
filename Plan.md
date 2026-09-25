# Rust Source Refactor Plan

## Goal

Reduce raw pointers, `unsafe`, unused code, redundant C types, and C ABI coupling in `src/` while preserving gameplay, save-file compatibility, score-file compatibility, and supported build targets. The repository contains no C implementation, but much of the Rust port still exports or imports legacy C symbols. Treat ABI cleanup as a compatibility decision, not a mechanical rename.

## Guiding Rules

- Keep C representations only at real foreign-function or persisted-format boundaries. Use Rust-native types and ownership inside the game.
- Prefer safe scoped access and opaque handles over exposing mutable references or raw pointers. Avoid holding collection locks while invoking callbacks or other code that can re-enter the collection.
- Preserve the existing deterministic game behavior and legacy save/score bytes unless a separately approved format change is made.
- Do not suppress warnings globally. Remove dead code only after confirming it has no runtime, feature, test, ABI, or serialization role.
- Keep a narrow, documented unsafe boundary. Each remaining unsafe operation should state its invariant locally and be covered by a focused check where practical.

## Staged Refactor

### 0. Baseline and compatibility inventory

1. Record current `cargo check`, `cargo test`, and `cargo clippy --all-targets` output, separating existing warnings from errors. Add a CI or documented baseline so later warning reductions are measurable.
2. Inventory every `#[no_mangle]`, `extern "C"` declaration, `#[repr(C)]` type, raw-pointer field, and unsafe block. Classify each as: required external ABI, libc/ncurses/platform FFI, save/score representation, or Rust-internal compatibility residue.
3. Confirm the intended consumers of the `staticlib` and `rlib` crate types and which exported symbols are a supported interface. Check build scripts, packaging, and downstream integrations before removing symbols.
4. Audit declarations against the original C headers/source or authoritative ABI definitions. Resolve type/layout and callback-signature questions before editing; in particular verify boolean storage widths and daemon callback argument shapes rather than inferring them from Rust call sites.
5. Capture behavior baselines: unit tests, save/restore round trips, score-file read/write compatibility, startup/demo modes, and a short interactive smoke test where a terminal is available.

**Exit criteria:** a checked-in or documented ABI inventory, known warning baseline, and repeatable behavior checks. No ABI-breaking changes in this stage.

### 1. Contain and audit the unsafe/FFI boundary

1. Create or consolidate a small private FFI layer for libc, ncurses, signals, and any intentionally supported C exports. Use the `libc` crate declarations where it covers the platform API; avoid handwritten variadic declarations when a safe or typed API exists.
2. Put C string conversion, pointer validation, and ABI type conversion in boundary helpers. Keep game logic on `String`, slices, `bool`, fixed-width Rust integers, and enums; do not propagate `c_char` or `c_int` merely because an old C function used them.
3. Replace unchecked pointer-to-function transmutation in daemon registration with typed callback identifiers or an explicit callback enum/table. First validate the actual legacy callback contract and save-state representation.
4. Review every `unsafe impl Send/Sync`, especially in player/equipment, item storage, globals, and list wrappers. Remove thread-safety claims not needed by the single-threaded game, or replace global lock-based sharing with an explicit single-threaded owner. Document any invariant that must remain.
5. Reduce unsafe blocks by narrowing them to individual operations and adding safe wrappers at the module boundary. Do not make an API `unsafe` solely because its current implementation contains an internal pointer operation if the wrapper can uphold the invariant.

**Exit criteria:** all foreign calls are declared in the FFI layer, pointer/callback conversions are centralized, and each remaining unsafe block or unsafe trait implementation has a reviewed invariant.

### 2. Replace raw `Thing` handles and intrusive lists

1. Use `MonsterList`/`MonsterId` as the model: move callers from `*mut Thing` to typed opaque handles and closure-scoped access. The current `MonsterList::handle`/`find` bridge should become temporary and progressively private.
2. Replace object `Box` arena pointers and intrusive `l_next`/`l_prev` links in `item/thing_list.rs`, `item/item_list.rs`, packs, floor items, equipment, and monster inventories with an owning arena plus generational `ThingId` handles. Generations prevent stale handles from becoming valid after slot reuse.
3. Represent pack and floor-item ordering with `VecDeque`/`Vec` of IDs or a small safe list abstraction. Preserve ordering and mutation behavior, including stack merging, dropping, pickup, monster inventory, and level teardown.
4. Remove pointer reinterpretation helpers (`thing_t`, `thing_o`, `thing_next`, `thing_prev`) in batches, starting with the item list and its direct consumers. Keep any necessary pointer adapter private to the FFI layer.
5. Add focused tests for attach/detach, head/tail/only-element cases, removal during traversal, stale IDs, ownership cleanup, and item/monster turn order.

**Exit criteria:** gameplay modules no longer store or pass raw `Thing` pointers; raw handles remain only in a narrow compatibility adapter if an external ABI still requires them.

### 3. Replace C-style globals with owned game state

1. Group mutable globals into explicit state owners (for example `GameState`, `Player`, `Level`, `Inventory`, `ScoreState`, and `RuntimeConfig`) and pass scoped references to the owning subsystem.
2. Migrate reads first, then writes, in small module clusters. Keep legacy exported globals as boundary mirrors only where the ABI inventory proves they are required; synchronize them at explicit transitions rather than allowing unrestricted cross-module mutation.
3. Convert C flag bytes and magic integers to Rust `bool` and domain enums internally. Convert values at the boundary and validate unknown values when loading saves.
4. Avoid replacing `static mut` with a global mutex as the end state. Prefer ownership and scoped borrowing; retain synchronization only where it represents a real concurrency requirement.
5. Start with tightly owned subsystems (daemon/fuse scheduling, score state, and UI/runtime state), then migrate cross-cutting gameplay state after the entity handles have stabilized.

**Exit criteria:** gameplay code accesses mutable state through explicit owners and scoped APIs; `static mut` is absent from internal gameplay modules or confined to documented ABI shims.

### 4. Separate runtime models from C and persistence representations

1. Keep Rust-native runtime structs separate from `#[repr(C)]` mirrors used by an actual ABI. Avoid using one type for gameplay, FFI, and save encoding.
2. Replace duplicate C-layout declarations such as `CFile`, `Score`, daemon records, and mirrored globals with one authoritative private boundary definition where possible. Use the existing `Score` layout only where it is truly required by the legacy score interface; encode the on-disk format explicitly rather than serializing in-memory layout.
3. Centralize `c_char` buffer conversion and fixed-size string handling. Use checked conversions that report truncation/invalid data rather than unchecked indexing or implicit lossy casts.
4. Keep save-file compatibility in explicit encode/decode code. Add golden fixtures or byte-for-byte round-trip tests before changing any field representation.
5. Remove redundant `c_*` imports and aliases from internal modules once the runtime model is native; retain `c_*` only in FFI signatures and format/layout adapters.

**Exit criteria:** C layout is used only for a proven ABI, and persistence compatibility is tested independently of Rust struct layout.

### 5. Remove unused and redundant code incrementally

1. Re-run compiler warnings and Clippy after each migration slice. Confirm every dead-code candidate against symbol exports, command dispatch, cfg/target-specific builds, tests, and save compatibility before deleting it.
2. Remove superseded pointer helpers, duplicate conversions, compatibility aliases, stale comments, and unused imports as their callers migrate. Avoid a broad cleanup commit mixed with behavior changes.
3. Prefer replacing duplicated wrappers with one typed helper only after the affected call sites have moved to the new API; do not preserve redundant abstractions just to keep old call shapes alive.
4. Consider `cargo machete` for unused dependencies and `cargo clippy --all-targets -- -D warnings` only after the existing baseline warnings have been addressed. Do not make warning denial the first step.

**Exit criteria:** no unexplained warnings in supported targets, no known-dead compatibility helpers, and no unused dependencies without an explicit reason.

### 6. Decide and enforce the final ABI policy

1. If the library is intentionally consumable by C or another binary interface, publish a small explicit ABI surface and keep its conversions/tests. Hide internal symbols and types from that surface.
2. If only the Rust binary and Rust library are supported, remove `#[no_mangle]` and `extern "C"` from internal game functions in batches after all Rust call sites use native APIs. Retain only platform FFI and any specifically supported exports.
3. Update crate types and documentation to match the chosen policy; do not keep a `staticlib` or broad symbol exports by inertia if nothing consumes them.

**Exit criteria:** the documented ABI matches the actual exported symbols and supported build products.

## Suggested Work Order

1. Establish the inventory and baseline (Stage 0).
2. Fix ABI mismatches and contain FFI/unsafe operations (Stage 1).
3. Migrate item/entity handles and list ownership (Stage 2).
4. Migrate globals by subsystem (Stage 3).
5. Separate runtime, ABI, and persistence types (Stage 4).
6. Remove dead compatibility code continuously (Stage 5).
7. Prune exports or formalize the remaining ABI (Stage 6).

Stages 2 and 3 are the largest and should be split into reviewable subsystem changes. Do not start broad ABI removal until the consumer inventory is complete. Stages 4 and 5 can proceed alongside each subsystem migration, but persistence and gameplay behavior checks must remain green throughout.

## Validation Per Change

- Run `cargo fmt --check`, `cargo check --all-targets`, and focused unit tests for the touched subsystem.
- Run `cargo test` after each subsystem migration; run `cargo clippy --all-targets` periodically and track warning count by category.
- Test save/restore and score-file compatibility against existing fixtures and, where available, files produced by the current build.
- Build and smoke-test supported platforms/features, especially platform-specific libc/ncurses and signal bindings.
- For handle/list changes, test ordering, stale handles, ownership cleanup, and mutations during traversal. For FFI changes, test null/invalid input handling and verify layout/signatures against the ABI inventory.
- Before final ABI cleanup, inspect exported symbols and link a minimal consumer for each supported library interface.

## Main Risks

- Existing symbol exports may be consumed externally even though the repository contains no C source; confirm before deleting or changing them.
- `src/state.rs` and score persistence encode legacy assumptions. Keep their external bytes stable while changing runtime types.
- Replacing pointer-linked lists can subtly change iteration order, deletion timing, or ownership cleanup, which affects gameplay.
- Mutable globals and unsafe pointer helpers are cross-cutting; migrate by ownership boundary, not by mass textual conversion.
- A safe wrapper is only sound if handle lifetime, aliasing, re-entrancy, and mutation rules are explicit. Preserve those invariants in APIs and tests.