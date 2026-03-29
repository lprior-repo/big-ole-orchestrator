# Architecture Drift Review — Bead vo-49tp

**STATUS: REFACTORED**

## Problem

`crates/vo-actor/src/instance/handlers.rs` was **731 lines** (limit: 300).
- Production code: lines 1–312 (312 lines, 12 over limit)
- Test code: lines 313–731 (418 lines)

## Refactoring Performed

### 1. Extracted `handlers/snapshot.rs` (66 lines)
- Moved `handle_snapshot_trigger` async function from handlers.rs
- Self-contained snapshot write logic (ADR-019)
- Declared `pub(crate) mod snapshot` to allow test access
- Function visibility: `pub(crate)` (was private, unchanged for production callers)

### 2. Extracted `handlers_tests.rs` (419 lines)
- Moved entire `#[cfg(test)] mod tests` block from handlers.rs
- Registered as `#[cfg(test)] mod handlers_tests` in `instance/mod.rs`
- All 5 snapshot tests updated to call `handlers::snapshot::handle_snapshot_trigger`
- All 6 signal tests updated to call `handlers::handle_signal` (changed visibility to `pub(crate)`)

### 3. Updated `handlers.rs` (263 lines, was 731)
- Kept: `handle_msg`, `handle_procedural_msg`, `handle_inject_event_msg`, `handle_signal`, `handle_heartbeat`, `handle_cancel`, `handle_get_status`, `inject_event`, `SNAPSHOT_INTERVAL`
- Added `pub(crate) mod snapshot;` and `pub(crate)` on `handle_signal` (for test access)

### 4. Updated `instance/mod.rs`
- Added `#[cfg(test)] mod handlers_tests;`

## File Sizes After Refactor

| File | Lines | Under 300? |
|------|-------|-------------|
| `handlers.rs` | 263 | ✅ |
| `handlers/snapshot.rs` | 66 | ✅ |
| `handlers_tests.rs` | 419 | N/A (test file) |

## Verification

- `cargo check -p vo-actor` — ✅ compiles clean
- `cargo test -p vo-actor --lib` — ✅ all 123 tests pass
- `cargo clippy -p vo-actor` — ✅ no new warnings from changed files (pre-existing doc style nits only)

## DDD Compliance

- No primitive obsession introduced
- Handler functions remain pure message-to-state transitions
- Module boundaries follow single-responsibility: snapshot write logic is isolated
- Types continue to act as documentation; no changes to domain types
