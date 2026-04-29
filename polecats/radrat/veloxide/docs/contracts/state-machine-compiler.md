## Contract: State Machine Compiler

### 1. Purpose

Defines the contract for the veloxide state machine compiler — the pure typed system that governs lifecycle state transitions, enforces invariants, and classifies errors. This contract establishes the authoritative types, invariants, and error taxonomy for all state machine operations in the vo-types crate.

### 2. Source ADRs

- `docs/adr/v2/ADR-039-v2-hierarchical-lifecycle-state-machine.md` (superstate hierarchy)
- `docs/adr/v2/ADR-027-v2-deterministic-event-sourced-replay.md` (replay semantics)
- `docs/adr/v2/ADR-029-v2-execution-leases-and-fencing.md` (fencing semantics)

### 3. State Machine Types

#### 3.1 LifecycleState

The flat state enum representing the lifecycle of a bead in the workflow engine.

```
LifecycleState {
  Pending,              // Initial: bead queued, not yet assigned
  RunningDecision,       // Decision phase: evaluating which step to execute
  StepScheduled,         // Step scheduled but not yet executing
  StepExecuting,         // Step actively executing
  WaitingForTimer,       // Waiting for external timer/callback
  Completed,             // Terminal: bead completed successfully
  Failed,                // Terminal: bead failed
  Cancelled,             // Terminal: bead was cancelled
}
```

#### 3.2 LifecycleSuperstate

Hierarchical superstate grouping per ADR-039.

```
LifecycleSuperstate {
  Active,      // Pending, RunningDecision, StepScheduled, StepExecuting
  Suspended,   // WaitingForTimer
  Recovering,  // (future: explicit recovery states)
  Compensating, // (future: compensation states)
  Terminal,    // Completed, Failed, Cancelled
}
```

#### 3.3 TransitionEvent

The vocabulary of events that trigger state transitions.

```
TransitionEvent {
  AssignToNode,      // From Pending
  Cancel,            // From any non-terminal
  StepScheduled,     // From RunningDecision
  Fail,              // From RunningDecision, StepScheduled, StepExecuting, WaitingForTimer
  ExecuteStep,       // From StepScheduled
  WaitForTimer,      // From StepExecuting
  CompleteStep,      // From StepExecuting
  TimerFired,        // From WaitingForTimer
  TimerExpired,      // From WaitingForTimer
  InstanceResumed,   // From Failed only
}
```

#### 3.4 OperationalStatus

Operational classification derived from lifecycle state.

```
OperationalStatus {
  Healthy,                    // Normal operation
  Blocked(BlockedReason),     // Blocked with specific reason
  Recovering,                 // Recovering from failure
}
```

#### 3.5 BlockedReason

Reason why a bead is blocked.

```
BlockedReason {
  DependenciesPending,  // Waiting for dependencies
  ResourceContention,   // Resource contention
  ManualHold,           // Manual hold
}
```

#### 3.6 LeaseRecord

Fence-token lease record for concurrency control.

```
LeaseRecord {
  instance_id: InstanceId,
  step_id: StepId,
  token: FenceToken,
}
```

### 4. Invariants (INV-*)

- **INV-001**: Terminal states reject all transitions except `InstanceResumed` from `Failed`
- **INV-002**: No self-loops or cycles in the state transition graph (except `InstanceResumed` recovery path)
- **INV-003**: `InstanceResumed` is only valid from `Failed` state
- **INV-004**: `Cancel` is valid from all non-terminal states: `Pending`, `RunningDecision`, `StepScheduled`, `StepExecuting`, `WaitingForTimer`
- **INV-005**: `Fail` is valid from: `RunningDecision`, `StepScheduled`, `StepExecuting`, `WaitingForTimer`
- **INV-006**: `Completed` is only reachable via `CompleteStep` from `StepExecuting`
- **INV-007**: `Failed` is reachable via `Fail` from eligible states OR via `TimerExpired` from `WaitingForTimer`
- **INV-008**: Superstate mapping is consistent: `Active` contains `{Pending, RunningDecision, StepScheduled, StepExecuting}`, `Suspended` contains `{WaitingForTimer}`, `Terminal` contains `{Completed, Failed, Cancelled}`
- **INV-009**: `apply()` returns `TransitionError::TerminalStateTransition` for any transition attempt from a terminal state (except `InstanceResumed`)
- **INV-010**: `apply()` returns `TransitionError::InvalidTransition` for any event not in the valid transition set for the current state

### 5. Error Taxonomy

```rust
enum TransitionError {
    TerminalStateTransition,
    InvalidTransition,
}

enum StateMachineError {
    Transition(TransitionError),
    InvariantViolation(InvariantViolation),
    SerializationError(String),
    UnknownState(String),
}
```

### 6. Transition Protocol

1. **Validate**: Check current state is non-terminal (or `Failed` for `InstanceResumed`)
2. **Match**: Check event is in the valid transition set for current state
3. **Apply**: Compute new state via exhaustive match
4. **Verify**: Confirm superstate consistency post-transition
5. **Record**: Emit transition for event sourcing/replay

### 7. Constraints

- The state machine compiler is pure: no I/O, no engine integration
- Exhaustive match in `apply()` ensures compiler catches missing variants
- All transitions are total over valid (state, event) pairs
- Terminal states are absorbing: no outward transitions except recovery path
- Superstate consistency is enforced by `LifecycleState::superstate()`

### 8. Relevant Files

- `crates/vo-types/src/state/lifecycle.rs` (state machine types)
- `crates/vo-types/src/state/transition.rs` (pure transition engine)
- `crates/vo-types/src/lifecycle_superstate.rs` (superstate hierarchy)
- `crates/vo-types/src/state/semantic_types.rs` (AttemptNumber, InstanceState, NodeName, TimerId)
- `crates/vo-types/src/integer_types.rs` (FenceToken, SequenceNumber)
- `crates/vo-types/src/errors.rs` (error taxonomy pattern)

### 9. Acceptance Criteria

- LifecycleState enum covers all workflow bead states with no gaps
- TransitionEvent enum is exhaustive and covers all valid state change triggers
- `apply()` function is total over valid (state, event) pairs with no panics
- All invariants (INV-001 through INV-010) are formally stated and testable
- Error taxonomy distinguishes transition errors from invariant violations
- Superstate mapping is consistent with flat state enum
- The contract is self-contained and references existing ADR documentation
