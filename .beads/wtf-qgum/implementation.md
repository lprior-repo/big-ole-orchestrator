# Implementation Summary

## Metadata
- **bead_id**: vo-qgum
- **phase**: STATE 3 (implementation)
- **date**: 2026-03-23

## Files Changed

| File | Action | Description |
|------|--------|-------------|
| `crates/vo-worker/src/builtin.rs` | **NEW** | Built-in activity handlers module |
| `crates/vo-worker/src/lib.rs` | **MODIFIED** | Added `pub mod builtin;` and `pub use builtin::register_defaults;` |
| `crates/vo-worker/Cargo.toml` | **MODIFIED** | Added `serde_json = { workspace = true }` dependency |

## Implementation Details

### `builtin.rs` — 3 public functions, 1 private pure function

1. **`register_defaults(worker: &mut Worker)`** (4 lines)
   - Calls `worker.register("echo", echo_handler)` and `worker.register("sleep", sleep_handler)`.
   - Idempotent — HashMap insert overwrites on duplicate key.
   - Preserves pre-existing handlers for other activity types.

2. **`echo_handler(task: ActivityTask) -> Result<Bytes, String>`** (2 lines)
   - Returns `Ok(task.payload)` — moves payload into return value (zero-copy on inner buffer).
   - Never fails. All payloads are valid.

3. **`sleep_handler(task: ActivityTask) -> Result<Bytes, String>`** (4 lines)
   - Delegates to `parse_sleep_ms` (pure calculation) for payload parsing.
   - Uses `?` operator to propagate parse errors as `Err(String)`.
   - Calls `tokio::time::sleep(Duration::from_millis(ms)).await` (cooperative).
   - Returns `Ok(SLEPT_RESULT.clone())` where `SLEPT_RESULT = b"\"slept\""`.

4. **`parse_sleep_ms(payload: &[u8]) -> Result<u64, String>`** (private, 10 lines)
   - Pure calculation — no I/O, no side effects.
   - Data->Calc separation: boundary parsing in one function.
   - Chain: `from_utf8` → `serde_json::from_str` → `as_object` → `get("ms")` → `as_u64`.
   - Returns `Err(SLEEP_PARSE_ERR)` for any deviation from expected format.

### Constraint Adherence

| Constraint | Status | Evidence |
|-----------|--------|----------|
| Zero `unwrap()` in production code | ✅ | No unwrap/expect/panic in non-test code |
| Zero `mut` in core logic | ✅ | Only `mut` in tests (allowed per skill spec) |
| No panics | ✅ | All failure paths return `Err(String)` |
| Expression-based | ✅ | `sleep_handler` uses `?` operator chain |
| Clippy flawless (no new warnings) | ✅ | Zero clippy warnings from `builtin.rs` |
| Module-level lints | ✅ | `#![deny(clippy::unwrap_used)]`, `#![deny(clippy::expect_used)]`, `#![deny(clippy::panic)]`, `#![warn(clippy::pedantic)]`, `#![forbid(unsafe_code)]` |
| Data->Calc->Actions | ✅ | `parse_sleep_ms` is pure calculation; `sleep_handler` is action (I/O sleep) |
| Parse at boundary | ✅ | JSON parsing isolated in `parse_sleep_ms`, returns typed `u64` |
| Contract signatures match | ✅ | All 3 public signatures match `contract.md` exactly |
| Error taxonomy match | ✅ | All errors return `SLEEP_PARSE_ERR` constant string |

### Test Coverage

32 unit tests in `#[cfg(test)] mod tests`:

| Category | Count | Tests |
|----------|-------|-------|
| echo_handler happy path | 3 | ASCII, binary, empty |
| echo_handler edge cases | 2 | 1MB payload, null bytes |
| sleep_handler happy path | 2 | 10ms (paused time), 0ms |
| sleep_handler error paths | 12 | non-UTF8, invalid JSON, missing ms, string ms, float ms, negative, nested object, empty, array, number, null, string JSON, boolean JSON |
| sleep_handler edge cases | 2 | extra JSON fields, u64::MAX parse |
| Invariant tests | 2 | echo never panics, sleep never panics |
| Pure parse function | 7 | valid, extra fields, non-UTF8, invalid JSON, missing ms, string ms, non-object, empty |

All 32 tests pass. Uses `tokio::time::pause()` / `start_paused = true` for deterministic sleep tests.

### Completion Criteria Checklist

- [x] `crates/vo-worker/src/builtin.rs` exists with `register_defaults`, `echo_handler`, `sleep_handler`
- [x] `echo_handler` returns `Ok(task.payload)` — unit test passes
- [x] `sleep_handler` parses `{"ms": u64}`, sleeps, returns `Ok(Bytes::from_static(b"\"slept\""))` — unit tests pass
- [x] `sleep_handler` returns `Err` on invalid JSON / missing `"ms"` — unit tests pass
- [x] `register_defaults` wired into `lib.rs` as `pub use builtin::register_defaults`
- [x] `cargo clippy -p vo-worker` passes (zero new warnings)
- [x] `cargo test -p vo-worker --lib` passes (69/69 tests green)
- [x] Zero `unwrap()` or `expect()` in new production code
- [x] Module-level clippy lints match existing pattern
- [ ] Integration tests (requires NATS — deferred to NATS-available environment)
