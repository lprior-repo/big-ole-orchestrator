# ADR-038 Lineage Review Findings (ve-ugt7)

## Scope
Review of workflow lineage tracking, continue-as-new semantics, and lineage ID propagation against ADR-038 v2.

## ADR-038 Contract Summary

ADR-038 §2 defines the rollover must atomically:
1. Write `ContinuedAsNew` for the old epoch
2. Create new `WorkflowStarted` for the successor epoch
3. Carry forward minimal canonical state
4. Update lineage routing so signals/queries target the active epoch

## Files Reviewed

| File | Role |
|------|------|
| `vo-types/src/lineage.rs` | Core types: `Epoch`, `WorkflowLineage`, `LineageState`, `LineageStatus`, `LineageError` |
| `vo-actor/src/routing.rs` | Lineage-aware query routing, `EpochResolver` trait, `LineageRouter` |
| `vo-storage/src/lineage_store.rs` | Fjall-backed lineage persistence, `LineageRecord`, `record_rollover` |
| `vo-actor/src/lib.rs:829-881` | `handle_continue_as_new` in ControlActor |
| `vo-actor/src/actor_messages.rs` | `ControlActorMessage::ContinueAsNew` message type |
| `vo-actor/src/signal_messages.rs:346-393` | `WorkflowContinued`, `ContinueAsNewError` types |
| `vo-types/src/events/payload.rs:93-99` | `EventPayload::ContinuedAsNew` variant |
| `vo-core/src/replay/engine.rs:93-98` | Replay skips `ContinuedAsNew` (lineage tracking, not state transition) |
| `docs/adr/v2/ADR-038-v2-workflow-lineage-and-continue-as-new.md` | The ADR itself |

## Critical Bug: Hardcoded Epochs (P0)

**Location**: `vo-actor/src/lib.rs:868-869`

```rust
let old_epoch = 0u64;
let new_epoch = 1u64;
```

`handle_continue_as_new` hardcodes `old_epoch = 0` and `new_epoch = 1` instead of resolving the actual current epoch from storage. Consequences:

- After the first rollover (0→1), all subsequent rollovers incorrectly report epoch 0→1 instead of N→N+1
- The `ContinuedAsNew` event written to the event log will contain wrong epoch numbers, corrupting lineage history
- Lineage routing updates will use stale epoch references
- **Violates ADR-038 §2.1**: "writes `ContinuedAsNew` for the old epoch" — the old epoch is always wrong after epoch 0

**Also present** in `vo-actor/src/control_actor.rs:341-342` (identical bug in radrat worktree copy).

**Fix**: Accept `old_epoch` as a parameter or resolve it from the lineage store before computing `new_epoch = old_epoch + 1`.

## Bug: No Tombstoning Check (P1)

**Location**: `vo-actor/src/lib.rs:829-881`

`ContinueAsNewError::LineageTombstoned` exists in the error enum (`signal_messages.rs:382`) but `handle_continue_as_new` never checks if the lineage is tombstoned. A tombstoned lineage (per ADR-042 §5, `LineageStatus::Tombstoned`) should reject continue-as-new, but the current code allows it.

**Fix**: Before proceeding, look up `LineageState` for the given `lineage_id` and reject if `!can_spawn_epoch()`.

## Bug: No Lineage ID Validation (P2)

**Location**: `vo-actor/src/lib.rs:829-881`

The `lineage_id` string parameter is not validated. `WorkflowLineage::new()` rejects empty and control-character-containing IDs, but `handle_continue_as_new` passes `lineage_id` through unchecked. An empty or control-char-polluted lineage_id will propagate into `WorkflowContinued` and the `ContinuedAsNew` event.

**Fix**: Call `validate_lineage_id(&lineage_id)` or construct a `WorkflowLineage` at the entry point.

## Observation: No Event Persistence in Handler

The handler returns `WorkflowContinued` but does not:
1. Write `ContinuedAsNew` event to storage
2. Create `WorkflowStarted` for the new epoch
3. Call `record_rollover` on the lineage store

This appears to be by design — the caller (likely a supervisor or orchestration layer) handles these side effects. However, the ADR's "atomic" requirement means the caller must perform all three in a single transaction. **Risk**: if the caller does not use a transaction, partial failures will leave inconsistent state (e.g., `ContinuedAsNew` written but routing not updated).

## Observation: No Tests for handle_continue_as_new

There are zero tests exercising `handle_continue_as_new` directly. The tests in `signal_messages.rs` only test `WorkflowContinued` struct construction. Missing test coverage:

- Happy path: correct epoch propagation across multiple rollovers
- Error path: terminal instance rejection
- Error path: tombstoned lineage rejection
- Error path: invalid instance_id format
- Error path: lock/storage failures
- Invariant: lineage_id is preserved across rollover
- Invariant: epoch strictly monotonically increases

## What Works Well

1. **Core types are solid**: `Epoch`, `WorkflowLineage`, `LineageState` are well-designed with proper validation (control chars, empty check, epoch monotonicity). Comprehensive unit tests in `lineage.rs`.
2. **Routing is clean**: `LineageRouter` with `EpochResolver` trait is a good abstraction. Tests cover active/historical/tombstoned/not-found cases.
3. **Storage is correct**: `lineage_store.rs` properly handles read/upsert/rollover with `record_rollover` correctly shifting `previous_instance_id`. Fjall-backed with encode/decode separation.
4. **Replay is correct**: Engine correctly skips `ContinuedAsNew` as a lineage-tracking event (not a state transition).
5. **Event payload parsing**: `ContinuedAsNew` variant correctly parses `lineage_id`, `old_epoch`, `new_epoch` from JSON.
6. **Red Queen adversarial tests**: Extensive control-character and boundary tests for `WorkflowLineage` validation.

## Recommended Fixes (Priority Order)

1. **P0**: Fix hardcoded epochs in `handle_continue_as_new` — resolve actual epoch from storage
2. **P1**: Add tombstoning check before allowing continue-as-new
3. **P2**: Validate `lineage_id` at entry point
4. **P2**: Add handler-level tests for all error paths and invariants
5. **P3**: Document the atomicity contract for callers (which side effects must be transactional)
