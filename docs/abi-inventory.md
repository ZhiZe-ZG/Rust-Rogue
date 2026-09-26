# ABI and Compatibility Inventory (Stage 0)

This document records the baseline and the full inventory of C-ABI surface in
`src/`, as required by Stage 0 of `Plan.md`. It is updated as later stages make
changes, so it always describes the current state of the tree.

## 1. Baseline (before any refactor)

Recorded on the initial `HEAD` (`a0da05f`) before any Stage 1+ edits.

| Command | Result |
| --- | --- |
| `cargo check --all-targets` | compiles; **0 errors**, **455** `warning:` lines |
| `cargo test --all-targets` | **36 passed; 0 failed** |
| `cargo clippy --all-targets` | **3 errors** (`clippy::eq_op`), **809** lib warnings |

### Stage 1 progress

| Command | Baseline | After Stage 1 |
| --- | --- | --- |
| `cargo check --all-targets` warnings | 455 | **439** |
| `cargo clippy --all-targets` errors | 3 | **0** |
| `cargo test --all-targets` | 36 passed | **38 passed** (2 new daemon tests) |
| `cargo fmt --check` diffs | 77 | **0 (repo is now rustfmt-clean)** |

Landed in this slice:

- **Stage 1.3**: `daemon.rs` now uses a typed `Daemon` callback enum instead of
  transmuting `*const c_void` to `Option<unsafe extern "C" fn()>`; the dispatcher
  passes the stored `d_arg` to the callback, matching the legacy `d_func(d_arg)`
  contract (the previous zero-argument call was UB for `turn_see`). All call sites
  in `startup.rs`, `daemons.rs`, `misc.rs`, `entity/monsters.rs`, and
  `item/potions.rs` were migrated; `state.rs` serialises via the stable legacy
  ids through `Daemon::save_id`/`from_save_id` (bytes unchanged, covered by tests).
- **Latent bugs fixed**: the three `clippy::eq_op` denials — `ISMISL | ISMISL` in
  `item/weapons.rs` (now `ISMISL`, matching the original weapon table) and the
  always-false screen-size check in `startup.rs` (now compares the real physical
  terminal size via `ui::physical_size`).
- Removed the now-dead daemon-function imports introduced by the migration.

Stage 1.1 (FFI consolidation) is **done**: all portable libc/stdio/string/ctype
declarations now live in the private `src/ffi.rs`, and the three duplicate
`CFile` definitions collapsed to the single `ffi::CFile`. The only Stage 1 item
still open is the `unsafe impl Send`/`Sync` removal, which is **blocked on
Stage 2** because `Thing` still embeds `NonNull` handles that `MonsterList`'s
`Mutex` relies on.

### Stage 2 progress (handle/list migration)

Because the handle migration touches nearly every gameplay module, it is being
split into reviewable slices:

- **Stage 2.5 (done)**: added focused tests to `item/thing_list.rs` pinning the
  list/arena invariants the migration must preserve — attach/detach across
  head/middle/tail/only-element cases, neighbour-link patching, removal during
  traversal (capturing the successor before free), and balanced allocation
  accounting via `allocated_count`.
- **Stage 2.6 (done)**: introduced the generational arena in `src/item/arena.rs`
  (`ThingArena` + opaque `ThingId { index, generation }`). Freed slots advance
  their generation on reuse, so a handle to a removed object is rejected
  (`contains`/`with`/`with_mut`/`remove` all validate the generation); that is
  the property the old slot-only index lacked. Access is closure-scoped like
  `MonsterList`. Two explicitly documented bridge methods (`ptr`, `id_for_ptr`/
  `remove_by_ptr`) map between `ThingId` and the legacy `*mut Thing` while the
  remaining raw-pointer callers migrate; the object store in
  `item/thing_list.rs` now delegates to this arena (`OBJECTS`), replacing the
  ad-hoc `Box` vector + separate counter. Four new arena tests cover round-trip,
  stale-handle rejection across reuse, the pointer bridge, and scoped mutation.
- **Still open**: migrating callers off raw `*mut Thing` handles (pack, floor
  items, equipment, monster inventories) to `ThingId`, and removing the
  intrusive `l_next`/`l_prev` ordering in favour of id sequences. Test count is
  now 45.

### Stage 5 progress (dead-code reduction)

| Command | After Stage 1 | After Stage 5 slice |
| --- | --- | --- |
| `cargo check --all-targets` warnings | 434 | **411** |
| `cargo test --all-targets` | 41 passed | **41 passed** |
| `cargo clippy --all-targets` errors | 0 | **0** |
| `cargo fmt --check` diffs | 0 | **0** |

Landed: removed the compiler-verified dead imports (unused `c_uint`/`c_void`/
`c_short`/`c_char`, unused `CObjInfo`/`Stats`/`ThingMonster`/`spread`, and the
now-unused daemon-fn/`cansee`/`runto`/`set_mname`/`endmsg` imports) across
`command`, `daemons`, `entity/fight`, `entity/monsters`, `init`, `item/{armor,
pack, potions, scrolls, sticks, thing_list, weapons}`, `machdep`, `misc`,
`options`, `save`, `startup`, and `wizard`. Only imports the compiler flagged as
unused were removed; no behavior changed.

### Stage 3 progress (owned state / `static_mut_refs`)

Started removing the `static_mut_refs` warnings (the largest remaining group)
by replacing shared/mutable references to `static mut` with
`std::ptr::addr_of!` / `std::ptr::addr_of_mut!` at access sites, which is the
sound, behavior-preserving fix (and the Rust-2024-recommended form).

| Command | Before Stage 3 | After Stage 3 (widespread slice) |
| --- | --- | --- |
| `cargo check --all-targets` warnings | 411 | **334** |
| `cargo test --all-targets` | 41 passed | **45 passed** |
| `cargo clippy --all-targets` errors | 0 | **0** |

Converted so far (via `addr_of!`/`addr_of_mut!`, `addr_of!(x).read()` for scalar
reads, and `addr_of_mut!(x).cast()` for buffer pointers/`strcpy`):

- **`command.rs`**: `call()` `prbuf`/`huh`/item-info pointers; wizard `dnum`,
  `inpack`, `food_left` reads.
- **`misc.rs`**: `fruit`/`prbuf` in `eat` and `call_it`.
- **`globals.rs`**: the `huh` copy in `set_huh_string`.
- **`entity/fight.rs`**: `PRNAME_BUF` in `prname`.
- **`item/sticks.rs`**: the `BUF` scratch buffer in `charge_str` (bounded slice
  from `addr_of_mut!`).
- **`item/pack.rs`**: `pack_char` indexes the fixed 26-entry `pack_used` array.
- **`options.rs`**: `home` in `parse_opts`; `inv_t_name` length uses the named
  `INV_T_NAME_LEN`.
- **`wizard.rs`**: `scr_info`/`pot_info`/`ws_info`/`ring_info` in `whatis`.
- **`init.rs`**: `init_probs` passes `addr_of_mut!` tables to `sumprobs`.
- **`item/potions.rs`**: `pot_info_at` helper replaces `pot_info.as_mut_ptr()`;
  `prbuf`/`fruit` in the `SeeInvisible` `snprintf`.
- **`item/things.rs`**: `prbuf` in `copy_to_prbuf`/`inv_name`, and every
  `pick_one(<table>.as_ptr() as *mut CObjInfo)` now uses `addr_of!(..).cast()`.
- **`rip.rs`**: `monsters` lookup, `KILLNAME_BUFFER`, and `whoami`/`purse` reads.

Remaining `static_mut_refs` sites by module (to convert the same way):
`state.rs` (60), `startup.rs` (9), `mdport.rs` (4), `machdep.rs` (3).

### `cargo check` warning categories (top groups)

| Count | Warning | Owning stage |
| --- | --- | --- |
| 111 | creating a mutable reference to mutable static | Stage 3 |
| 36 | creating a shared reference to mutable static | Stage 3 |
| 14 | direct cast of function item into an integer | Stage 1 |
| 12 | function pointer comparisons … addresses not guaranteed unique | Stage 1 |
| 7 / 7 | `extern` block uses type `Thing`, which is not FFI-safe | Stage 3/4 |
| 4 / 4 | `extern` block uses type `Stats`, which is not FFI-safe | Stage 3/4 |
| 3 | `extern` block uses type `Option<usize>`, which is not FFI-safe | Stage 3/4 |
| ~10 | variable does not need to be mutable | Stage 5 |
| ~20 | unused import / unused variable | Stage 5 |
| ~200 | `… is never used` (dead constants/functions/statics) | Stage 5 |
| 5 | unreachable pattern | Stage 5 |

The two structural groups that gate Stages 1 and 3 are the 147
`static_mut_refs` warnings and the 26 daemon function-pointer casts/comparisons.

## 2. Crate types and supported products

`Cargo.toml` declares:

```toml
[lib]
name = "rogue_rust"
path = "src/lib.rs"
crate-type = ["staticlib", "rlib"]
```

- **`rlib`**: consumed by the in-repo binary `src/bin/rogue.rs`.
- **`staticlib`**: **no consumer exists in this repository.** There is no C
  source in `src/`; `rogomatic/` is a separate C program that does not link this
  crate.

**Decision (Stage 6 target):** support only the `rlib` + `rogue` binary.
Retain only platform FFI (libc/ncurses/signals) in a private layer and the
`rogue_main` Rust entry point; drop `staticlib` and the broad `#[no_mangle]`
exports at the end of the refactor. Persistence stays byte-compatible via
explicit encode/decode, independent of exported symbols.

## 3. `#[no_mangle]` exports — 322 occurrences

Counts per file:

| File | Count | Classification |
| --- | ---: | --- |
| `globals.rs` | 90 | Rust-internal compatibility residue (Stage 3/6) |
| `mdport.rs` | 26 | platform FFI wrappers (Stage 1) |
| `misc.rs` | 15 | internal residue (Stage 6) |
| `entity/fight.rs` | 14 | internal residue (Stage 6) |
| `entity/chase.rs` | 14 | internal residue (Stage 6) |
| `item/pack.rs` | 13 | internal residue (Stage 6) |
| `init.rs` | 13 | internal residue; 5 are data tables (Stage 3) |
| `daemons.rs` | 12 | daemon callbacks (Stage 1) |
| `startup.rs` | 10 | entry point + signal handlers (Stage 1/6) |
| `machdep.rs` | 10 | platform FFI (Stage 1) |
| `item/weapons.rs` | 9 | internal residue (Stage 6) |
| `command.rs` | 9 | internal residue (Stage 6) |
| `rip.rs` | 8 | internal residue + 2 true exports (Stage 6) |
| `entity/monsters.rs` | 8 | internal residue (Stage 6) |
| `draw.rs` | 8 | internal residue (Stage 6) |
| `daemon.rs` | 8 | daemon scheduler (Stage 1) |
| `item/things.rs` | 7 | internal residue (Stage 6) |
| `wizard.rs` | 6 | internal residue (Stage 6) |
| `item/potions.rs` | 6 | internal residue (Stage 6) |
| `vers.rs` | 5 | **save-file representation** (keep bytes) |
| `save.rs` | 4 | save entry points (Stage 4) |
| `item/sticks.rs` | 4 | internal residue (Stage 6) |
| `item/rings.rs` | 4 | internal residue (Stage 6) |
| `item/armor.rs` | 4 | internal residue (Stage 6) |
| `entity/player.rs` | 4 | internal residue + `nh` save global (Stage 2/3) |
| `options.rs` | 3 | internal residue (Stage 6) |
| `state.rs` | 2 | save/restore entry points (Stage 4) |
| `score.rs` | 2 | **score-file representation** (Stage 4) |
| `item/scrolls.rs` | 2 | internal residue (Stage 6) |
| `game/player.rs` | 1 | internal residue (Stage 3) |
| `entity/rndmove.rs` | 1 | internal residue (Stage 6) |

The only exports that are *potential* external ABI under the "Rust-only"
decision are `startup::rogue_main` and `rip::rogue_rip_count` /
`rip::rogue_rip_line`. Everything else is internal compatibility residue and can
be de-exported once all Rust call sites are native (Stage 6).

## 4. `#[repr(C)]` types

| Location | Type | Classification | Action |
| --- | --- | --- | --- |
| `save.rs:27` | `CFile` | duplicate opaque file handle | merge (Stage 4) |
| `score.rs:11` | `CFile` | duplicate | merge (Stage 4) |
| `state.rs:117` | `CFile` | duplicate | merge (Stage 4) |
| `score.rs:16` | `Score` | **score-file representation** | keep layout for score I/O (Stage 4) |
| `state.rs:122` | `CStone` | init table mirror | native or boundary (Stage 4) |
| `daemon.rs:30` | `CDelayedAction` | daemon table | replace with typed table (Stage 1) |
| `init.rs:53` | `CStone` | material table mirror | native (Stage 4) |
| `rip.rs:37` | (RIP struct) | internal | review (Stage 6) |
| `item/potions.rs:102` | (info struct) | internal | review (Stage 5) |

`src/entity/stats.rs` documents `Stats` as the successor to the legacy C
`struct stats`; it is no longer `#[repr(C)]` (the doc-comment reference is
stale).

## 5. Raw-pointer surface (Stage 2)

| Location | Form | Note |
| --- | --- | --- |
| `entity/player.rs` | `Thing`, `ThingMonster`/`ThingObject` with `NonNull` links, `thing_t/thing_o/thing_next/thing_prev`, 4 `static mut` externs | intrusive list + actor/object union |
| `item/arena.rs` | `ThingArena` slots + `ThingId{index,generation}`; `ptr`/`id_for_ptr` bridge | new owner; callers to migrate off `*mut Thing` |
| `item/thing_list.rs` | `*mut Thing` returns; adapter over `ThingArena` | thin legacy shim; migrating to `ThingId` |
| `item/item_list.rs` | `head: *mut Thing` | floor list head |
| `game/player.rs` | `Slot(RwLock<Option<NonNull<Thing>>>)` equipment | convert to IDs |
| `game/monster_list.rs` | `handle()`/`find()` bridge to `*mut Thing` | temporary bridge |
| `state.rs` | `*mut Thing`, `*mut CDelayedAction` throughout save code | encode/decode layer |
| `globals.rs` | `l_last_pick`/`last_pick: *mut CThing` | globals |

### `unsafe impl Send`/`Sync`

| Location | Type | Justification to review |
| --- | --- | --- |
| `entity/player.rs:366-367` | `Thing` | needed only because `MonsterList` is a `Mutex`; remove when handles are IDs (Stage 2) |
| `init.rs:62` | `CStone` | `static` table of `'static` string pointers; the impl is load-bearing |

The unsafe-impl count dropped from **9 → 3**:
- `item/thing_list.rs` — `unsafe impl Send for OwnedThing` was redundant
  (`Thing` already opts into `Send`, so `Box<Thing>` is `Send`).
- `item/item_list.rs` — `unsafe impl Send/Sync for ItemList` were removed by
  storing the head in an `AtomicPtr<Thing>` (unconditionally `Send + Sync`)
  instead of a bare `*mut Thing`, while keeping the same handle semantics.
- `game/player.rs` — `unsafe impl Send/Sync for Slot` were removed the same way,
  by storing each equipment handle in an `AtomicPtr<Thing>`.

## 6. Confirmed ABI-safety / correctness items

These are the items `Plan.md` requires be verified against the authoritative
source *before* editing:

1. **Daemon callback signature.** The pre-refactor `daemon.rs` stored
   `type DFunc = Option<unsafe extern "C" fn()>` and transmuted from
   `*const c_void`, while calling it with **no argument**. Its own comment
   quotes the real C declaration as `void (*d_func)(int)`, and the
   `item/potions.rs` monster-detection effect registers `turn_see` with a
   non-zero `d_arg` and relies on the callback receiving it. Verified contract:
   the callback is **`fn(int)` and must be invoked with the slot's `d_arg`**;
   most callbacks ignore the argument, but `turn_see` does not. The previous
   zero-argument call was therefore undefined behaviour. The persisted daemon
   identity is a **stable integer 1–9** (`state.rs:rs_write_daemons`/
   `rs_read_daemons`), not a pointer.

   **Resolved in Stage 1.3**: the table now stores the typed `Daemon` tag and
   `Daemon::run(arg)` passes `d_arg` through, so the calling convention matches
   the C contract. Persistence is unchanged (`Daemon::save_id`/`from_save_id`
   reproduce the same 1–9 ids and the `-1`/`0` sentinels).
2. **Boolean storage width.** Save helpers read/write booleans as **1 byte**
   (`rs_write_boolean`/`rs_read_boolean`), while several globals are declared
   `bool` and others `c_uchar`. `state.rs` externs mix `bool` (`msg_esc`) and
   `c_uchar` for the same conceptual flags. This is safe only because every
   boolean goes through the 1-byte helper; it must be preserved exactly.
3. **`version` array.** `vers.rs` declares `static mut version: [c_char; 28]`
   and `save_file` writes `strlen(version)+1` bytes via `CStr`. `restore` reads
   `strlen(version)+1` then an 80-byte header line. The `encstr`/`statlist`
   arrays are 40/38 bytes. These widths are **save-file representation** and must
   not change.

## 7. Persistence boundaries (must stay byte-identical)

- **Save file**: header `"{lines} x {cols}\n"` (1 byte + 80-byte block) followed
  by version string and the `state.rs` `rs_*` record stream. Version identity
  lives in `vers.rs` (`release`, `encstr`, `statlist`, `version`).
- **Score file**: `score.rs` reads/writes `MAXSTR` (1024) name bytes + a
  100-byte `snprintf` line per entry; `Score` layout is `#[repr(C)]`.
- Both must be covered by golden round-trip tests before any representation
  change (Stage 4).