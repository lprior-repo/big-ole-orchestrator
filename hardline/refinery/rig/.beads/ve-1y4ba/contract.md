# Contract: Publication Barrier State Transition (ve-1y4ba)

## Overview

This contract specifies the Design-by-Contract (DbC) terms for the publication barrier state transition in the workflow lifecycle. The publication barrier ensures that workflow steps cannot emit an `output_ref` to downstream consumers until the associated blob is confirmed durable.

## Reference

- Parent Issue: ve-s08ri (vo-core: Implement publication barrier state transition)
- ADR Reference: ADR-040 (Blob Publication)
- Phase: Go State 1 - Design-by-contract specification

---

## Types

### BarrierState Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarrierState {
    /// No blob publication in progress
    Idle,
    /// Step yielded a blob, waiting for durable confirmation
    PendingPublication,
    /// Blob confirmed durable, output_ref can be emitted
    Confirmed,
    /// Blob publication failed, step must handle error
    Failed,
}
```

### TransitionEvent Variants (Publication Barrier)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BarrierTransitionEvent {
    /// Step yielded a blob reference
    BlobYielded,
    /// Blob confirmed durably stored
    BlobConfirmed,
    /// Blob publication failed
    BlobFailed,
    /// Reset to idle (for retry or cancellation)
    Reset,
}
```

---

## State Machine

```
       ┌──────────────────────────────────────────────────────────────┐
       │                                                              │
       ▼                                                              │
   ┌──────┐     BlobYielded      ┌──────────────────┐                 │
   │ Idle │ ───────────────────► │ PendingPublication │                │
   └──────┘                      └──────────────────┘                 │
       ▲                                 │                              │
       │                                 │ BlobConfirmed                │
       │                                 ▼                              │
       │                           ┌──────────┐                        │
       │                           │ Confirmed│                        │
       │                           └──────────┘                        │
       │                                 │                              │
       │                                 │ (transition to next step)   │
       │                                 ▼                              │
       │      ┌──────────────────────────────────┐                    │
       │      │          (emits output_ref)        │                    │
       │      └──────────────────────────────────┘                    │
       │                                                              │
       │                           ┌──────────┐                        │
       └────────────────────────── │  Failed  │ ◄── BlobFailed        │
                                   └──────────┘                        │
                                         │                              │
                                         │ Reset                       │
                                         ▼                              │
                                   ┌──────┐ ───────────────────────────►│
                                   │ Idle │                            │
                                   └──────┘                            │
```

---

## Invariants

### I1: No Double-Enter
A step cannot enter `PendingPublication` state if it is already in that state.
- **Formal**: `∀ step: step.state ≠ PendingPublication` before `BlobYielded`

### I2: No Skip Transitions
A step cannot transition directly to `Confirmed` without passing through `PendingPublication`.
- **Formal**: `Confirmed` is only reachable from `PendingPublication` via `BlobConfirmed`

### I3: Output Ref Emission Gate
An `output_ref` MUST NOT be emitted while the step is in `PendingPublication` state.
- **Formal**: `emitted(output_ref) ⇒ state ≠ PendingPublication`

### I4: No Terminal Without Confirmation
A step with a required blob cannot reach a terminal success state without confirming the blob.
- **Formal**: `terminal_success ⇒ blob_status = Confirmed ∨ blob_optional`

### I5: Failure Propagates
If `BlobFailed` occurs, the step transitions to `Failed` state and the error is recorded.
- **Formal**: `BlobFailed ⇒ state = Failed ∧ last_error = Some(BlobPublicationError)`

---

## Preconditions

### P1: BlobYielded
**Requires**:
- Step must be in an active executing state (`StepExecuting`)
- Step must have produced a `BlobRef`
- No pending publication for this step already exists

**Enforces**:
- Transition to `PendingPublication` state
- Blob reference is stored for later confirmation

### P2: BlobConfirmed
**Requires**:
- Step must be in `PendingPublication` state
- The blob must be confirmed durable by storage layer
- Blob reference must match the one from `BlobYielded`

**Enforces**:
- Transition to `Confirmed` state
- Blob status updated to `Published`

### P3: BlobFailed
**Requires**:
- Step must be in `PendingPublication` state
- Failure reason must be provided

**Enforces**:
- Transition to `Failed` state
- Error recorded with reason

---

## Postconditions

### Q1: After BlobYielded
- `state = PendingPublication`
- `blob_ref = provided BlobRef`
- `publication_start_time = now()`

### Q2: After BlobConfirmed
- `state = Confirmed`
- `blob_ref.status = Published`
- `output_ref` may now be emitted

### Q3: After BlobFailed
- `state = Failed`
- `last_error = Some(error_reason)`
- Retry decision deferred to caller

---

## Error Taxonomy

### BarrierError Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BarrierError {
    /// Blob confirmation timed out
    #[error("Blob publication timeout for step {step_id}")]
    PublicationTimeout { step_id: StepId },

    /// Blob status check failed
    #[error("Blob {blob_id} status check failed: {reason}")]
    StatusCheckFailed { blob_id: BlobId, reason: String },

    /// Invalid transition attempted
    #[error("Invalid barrier transition from {from} with event {event}")]
    InvalidTransition { from: BarrierState, event: BarrierTransitionEvent },

    /// Blob not found during confirmation
    #[error("Blob {blob_id} not found during publication confirmation")]
    BlobNotFound { blob_id: BlobId },

    /// Storage error during publication
    #[error("Storage error during blob publication: {cause}")]
    StorageError { cause: String },
}
```

### Error Classification

| Error | Category | Retryable |
|-------|----------|-----------|
| `PublicationTimeout` | Transient | Yes |
| `StatusCheckFailed` | Transient | Yes |
| `InvalidTransition` | Programming Error | No |
| `BlobNotFound` | Permanent | No |
| `StorageError` | Transient | Depends |

---

## Test Scenarios

### Happy Paths

1. **T1**: Step yields blob → transitions to PendingPublication → blob confirms → transitions to Confirmed → output_ref emitted
2. **T2**: Step yields optional blob → blob fails → step completes with inline data (no output_ref)

### Error Paths

3. **T3**: Attempt to emit output_ref while in PendingPublication → rejected with `InvalidTransition`
4. **T4**: Blob confirmation timeout → transitions to Failed
5. **T5**: Invalid transition from Idle with BlobConfirmed → `InvalidTransition` error
6. **T6**: Double-enter PendingPublication → `InvalidTransition` error

---

## Implementation Notes

- The barrier state machine should be implemented as a submodule of the workflow execution state machine
- State transitions must be atomic and validated
- All transitions should be logged for observability
- The barrier must be `Send + Sync` for concurrent access
