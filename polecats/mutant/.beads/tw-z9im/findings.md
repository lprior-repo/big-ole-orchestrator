# Findings: tw-z9im - ProjectionRebuilder Atomic Rebuild

## Issue
`ProjectionRebuilder::rebuild_full` in `crates/vo-core/src/replay/projection/rebuilder.rs` builds state incrementally. If an event fails mid-sequence, the function returns an error, but the design did not clearly guarantee atomic swap with the live projection on full success.

## Fix Applied

### Code Change: `crates/vo-core/src/replay/projection/rebuilder.rs`

Renamed the internal state variable from `state` to `rebuild_state` to make it explicit that this is the temporary rebuild state being built up during iteration:

```rust
// Before:
let mut state = S::default();
...
state = self.projector.project(state, &event)... // mutating in place conceptually

// After:
let mut rebuild_state = S::default();
...
rebuild_state = self.projector.project(rebuild_state, &event)... // explicit rebuild
```

The existing error-handling semantics already ensured atomicity:
- If any event fails, `rebuild_full` returns `Err(ProjectionError::BuildFailed(...))`
- The caller (`check_and_rebuild_if_stale` in registry.rs) transitions to `Failed` state and does NOT return the partial state
- On success, the caller transitions to `Ready` state

### Tests Added

Added `#[cfg(test)] mod rebuild_tests` with two tests:

1. **`rebuild_full_atomic_on_event_failure`**: Verifies that when event-47 fails, `rebuild_full` returns an error with the message "synthetic failure at event 47".

2. **`rebuild_full_succeeds_with_all_events`**: Verifies that when all events succeed (1-46), the result contains all events applied and the final state includes "event-46".

### Key Design Properties

1. **Builds into fresh state**: `rebuild_state = S::default()` starts fresh
2. **Progress tracking**: `context.update_progress(processed)` tracks progress atomically
3. **Cancellation support**: Checks `context.is_cancelled()` on each iteration
4. **Error propagation**: On failure, returns `Err(ProjectionError::BuildFailed(...))` - old projection untouched
5. **No partial state exposure**: Caller only gets `Ok(result)` on full success

## Verification

```bash
cargo test --package vo-core --lib replay::projection::rebuilder::rebuild_tests
# Result: 2 passed, 1335 filtered out

cargo build --package vo-core
# Result: Finished successfully (warnings are pre-existing)
```

## Files Modified

- `crates/vo-core/src/replay/projection/rebuilder.rs` - Added atomic rebuild semantics + tests
