# Black-Hat Adversarial Review: Runtime Configuration Hot-Reload

**Bead**: ve-ji6ve
**Target**: `crates/vo-core/src/config_hot_reload.rs` (1086 lines)
**Reviewer**: nuka (black-hat inquisition)
**Date**: 2026-04-15

## VERDICT: REJECTED

---

## PHASE 1: Contract & Bead Parity

The bead references "runtime configuration" with focus on invalid config handling. The implementation provides:
- `HotReloadConfig<T>` with atomic swap, validation, rollback
- `FileWatcher`, `FilteredFileWatcher`, `DebouncedFileWatcher`
- `EventChannel` for async event propagation
- `Error` enum with 10 variants

**Contract compliance**: PASS for the hot-reload config core. The file watching infrastructure is beyond the stated contract scope but not harmful.

---

## PHASE 2: Farley Engineering Rigor

### Function Length

| Function | Lines | Status |
|---|---|---|
| `DebouncedFileWatcher::new()` | 301-362 (61 lines) | **LETHAL**: Far over 25-line limit |
| `matches_pattern()` | 247-261 (14 lines) | PASS |
| `HotReloadConfig::reload_from_file()` | 116-135 (19 lines) | PASS |
| All other functions | < 25 lines | PASS |

### File Length

**LETHAL**: 1086 lines. Architectural drift limit is 300 lines. This file contains 5 distinct types (`HotReloadConfig`, `FileWatcher`, `FilteredFileWatcher`, `DebouncedFileWatcher`, `EventChannel`), their tests, and helper types. Must be split.

### I/O Hiding in Logic

- `reload_from_file()` (line 116-135): Reads file from disk inside a method on a generic config container. This should be in the I/O shell, not in a method that also manages in-memory state (pending/current swap).

---

## PHASE 3: NASA-Level Functional Rust (The Big 6)

### Make Illegal States Unrepresentable

- **LETHAL**: `pending: RwLock<Option<T>>` — the `None` state means "no pending update" but this is an implicit contract. The `commit()` method must check for `None` and return `SwapFailed`. A two-phase enum would be clearer:
  ```rust
  enum Phase { Stable(T), PendingStaging { current: T, staged: T } }
  ```

- **MINOR**: `WatcherConfig.debounce_duration: Option<Duration>` — `None` means "no debouncing". This is fine but could be an enum for clarity.

### The Panic Vector

**6x `expect()` on `std::sync::RwLock`** (lines 81, 89, 96, 98, 107, 130):

All claim "SAFETY: RwLock not poisoned" but this guarantee is fragile:
- The `HotReloadConfig` holds both `current` and `pending` locks
- `commit()` (line 95-104) acquires `pending` write lock then `current` write lock — this is a deadlock risk if another thread calls `current()` (read lock) while a third calls `commit()`
- Actually: `commit()` takes `pending` lock first, then `current` lock. `current()` only takes `current` read lock. `try_update()` only takes `pending` write lock. No deadlock because lock ordering is consistent. But if ANY code path panics while holding either lock, ALL `expect()` calls cascade-fail.

### Newtypes

- **MINOR**: `Duration`, `PathBuf`, `Vec<String>` used raw in `WatcherConfig`. Not domain-modeled.

---

## PHASE 4: Ruthless Simplicity & DDD

### YAGNI Violations

- **MAJOR**: `DebouncedFileWatcher` (lines 292-410, 118 lines) — This integrates a `Debouncer` with file watching. But the `new()` method creates its own event channel AND its own debouncer AND its own watcher. The `with_debouncer` method takes an external debouncer. Neither method actually uses the debouncer for anything visible — the `debouncer` field is marked `#[allow(dead_code)]` on line 297. This is dead code.

- **MAJOR**: `EventChannel` (lines 270-290) — Wraps an `mpsc::Sender` but the `new()` constructor (line 275-278) immediately drops the receiver (`let (tx, _rx) = ...`). No production code can ever receive from this channel. The `send()` method (line 280-285) will always get `EventQueueClosed` because the receiver was dropped at construction.

- **MAJOR**: `FilteredFileWatcher::matches_pattern` silently ignores invalid glob patterns (line 254: `if let Ok(glob) = ...`). An invalid pattern is a configuration error that should be caught at construction time, not silently skipped at match time.

### Silent Error Suppression

- Lines 325, 330: `let _ = event_tx.blocking_send(...)` — file system events are silently dropped if the channel is full or closed. In a hot-reload system, dropping events means config changes may be missed entirely.

---

## PHASE 5: The Bitter Truth

### Test Quality

48 tests is good density. However:

- **MINOR**: All 48 tests use `TempDir::new().unwrap()` and `fs::write(...).unwrap()`. While acceptable in test code, this creates boilerplate. Consider a test helper.

- **MINOR**: `DebouncedFileWatcher` has zero tests despite being 118 lines of production code. The `EventChannel` has 2 tests, but they test a broken abstraction (receiver dropped at construction).

### Dead Code

The `DebouncedFileWatcher` and `EventChannel` are structurally broken:
- `DebouncedFileWatcher::new()` creates a debouncer from the event receiver, but also creates a separate `EventChannel` that no one reads from
- The internal `event_tx` (line 308) sends to the debouncer, but the returned `EventChannel` (line 351) is a different channel entirely
- This means the returned `EventChannel` will never receive any events — the file watcher sends to the debouncer's internal channel, not the returned one

---

## Summary of Findings

### LETHAL (3)

1. **File over 300 lines**: 1086 lines — must be split into separate modules
2. **`DebouncedFileWatcher::new()` over 25 lines**: 61 lines, mixing watcher creation, debouncer setup, and channel wiring
3. **`DebouncedFileWatcher` is structurally broken**: returned `EventChannel` never receives events because the internal sender goes to the debouncer, not the returned channel

### MAJOR (3)

1. `EventChannel::new()` drops its own receiver — `send()` will always fail
2. `DebouncedFileWatcher.debouncer` field is dead code (`#[allow(dead_code)]`)
3. Invalid glob patterns silently ignored in `matches_pattern()`

### MINOR (4)

1. 6x `expect()` on `RwLock` with fragile "SAFETY" justification
2. `pending: Option<T>` could be a two-phase enum
3. Raw primitives in `WatcherConfig`
4. `let _ = blocking_send()` drops file events silently

---

## MANDATE

1. **Split into modules**: `hot_reload.rs`, `file_watcher.rs`, `filtered_watcher.rs`, `debounced_watcher.rs`, `event_channel.rs`, `types.rs`, `tests/`
2. **Fix `DebouncedFileWatcher`**: wire the returned `EventChannel` to actually receive debounced events, or remove the broken abstraction
3. **Remove `EventChannel`**: or fix it so the receiver is accessible
4. **Replace `expect()` with `unwrap_or_else(|e| e.into_inner())`** or switch to `parking_lot::RwLock`
5. **Validate glob patterns at `FilteredFileWatcher` construction time**, not silently at match time

After fixes, re-submit for full 5-phase re-review.
