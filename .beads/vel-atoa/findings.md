# vel-atoa: Fix wait_epoch_for_instance() stub returning Epoch::ZERO

## Problem
`wait_epoch_for_instance()` in `crates/vo-types/src/signal/signal_match.rs:133` was a stub function that always returned `Epoch::ZERO`. This caused incorrect epoch-local signal matching per ADR-042:

1. Any epoch-local signal with epoch != 0 would ALWAYS mismatch (even if the instance is actually in that epoch)
2. Any epoch-local signal with epoch == 0 would ALWAYS match (even if the instance has since moved to a different epoch via `continue-as-new`)

## Root Cause
The `signal_match()` function called `wait_epoch_for_instance(wait.instance_id())` internally, which was a placeholder stub. Per ADR-042: "an explicitly epoch-scoped signal must fail if the targeted epoch is no longer eligible."

## Fix Applied
Changed `signal_match()` signature to accept `instance_epoch: Epoch` as a 4th parameter, making the epoch resolution explicit in the caller rather than hidden in a stub. This follows the same pattern already used for `wait_instance_lineage_id` (caller-resolved, not stored in `WaitRecord`).

### API Change
```rust
// Before:
pub fn signal_match(
    signal: &SignalAddress,
    wait: &WaitRecord,
    wait_instance_lineage_id: &InstanceId,
) -> SignalMatchResult

// After:
pub fn signal_match(
    signal: &SignalAddress,
    wait: &WaitRecord,
    wait_instance_lineage_id: &InstanceId,
    instance_epoch: Epoch,
) -> SignalMatchResult
```

### Removed
- `wait_epoch_for_instance()` stub function (always returned `Epoch::ZERO`)

### Updated Callers
- All test calls in `signal_match.rs` (vo-types lib tests)
- All test calls in `signal_timer_lifecycle_red_queen.rs` (vo-actor tests)

### New Tests Added
1. `signal_match_epoch_local_matches_when_instance_epoch_matches_signal_epoch` — proves epoch=42 matches when instance_epoch=42
2. `signal_match_epoch_local_mismatches_when_instance_epoch_differs_from_signal_epoch` — proves epoch=42 mismatches when instance_epoch=99
3. `signal_match_epoch_local_lineage_wide_ignores_instance_epoch` — proves lineage-wide signals ignore instance_epoch

## Files Changed
- `crates/vo-types/src/signal/signal_match.rs` — API change, stub removal, new tests
- `crates/vo-actor/tests/signal_timer_lifecycle_red_queen.rs` — updated call sites

## Notes
- Production callers in vo-actor/src do not yet call `signal_match()` (only tests do), so this is a safe API surface change
- When production code is added, callers must resolve `instance_epoch` from workflow state and pass it in
