## Contract: Merge Conflict Auto-Resolver

### 1. Purpose

Defines the contract for automatically resolving merge conflicts in the veloxide event-sourced actor system. This contract establishes the types, invariants, and error taxonomy for the merge conflict resolution subsystem.

### 2. Source ADRs

- `docs/adr/v2/ADR-027-v2-exactly-once-core.md` (conflict resolution baseline)
- `docs/adr/v2/ADR-029-v2-execution-leases-and-fencing.md` (fencing semantics)
- `docs/adr/v2/ADR-039-v2-hierarchical-lifecycle-state-machine.md` (state machine invariants)

### 3. Conflict Types

#### 3.1 LeaseConflict
Occurs when two concurrent operations attempt to acquire or renew the same lease.

```
LeaseConflict {
  instance_id: InstanceId,
  step_id: StepId,
  holder_a: LeaseRecord,
  holder_b: LeaseRecord,
  contested_at: TimestampMs,
}
```

#### 3.2 StateTransitionConflict  
Occurs when two events attempt to transition the same actor instance to incompatible states.

```
StateTransitionConflict {
  instance_id: InstanceId,
  current_state: LifecycleState,
  event_a: TransitionEvent,
  event_b: TransitionEvent,
  sequence_a: SequenceNumber,
  sequence_b: SequenceNumber,
}
```

#### 3.3 SequenceConflict
Occurs when events arrive with identical or inverted sequence numbers from different producers.

```
SequenceConflict {
  instance_id: InstanceId,
  expected_next: SequenceNumber,
  received: SequenceNumber,
  producer: NodeName,
}
```

#### 3.4 FenceConflict
Occurs when a fence token mismatch is detected during lease validation.

```
FenceConflict {
  instance_id: InstanceId,
  presented_token: FenceToken,
  current_token: FenceToken,
  operation: &'static str,
}
```

### 4. Resolution Strategies

#### 4.1 ResolutionStrategy Enum

```rust
enum ResolutionStrategy {
    FenceTokenPriority,    // Higher fence token wins
    EarliestSequenceWins, // Lower sequence number wins
    LatestTimestampWins,   // Most recent timestamp wins
    CurrentHolderRetains,  // Existing lease holder wins
    RejectBoth,            // Escalate to manual resolution
}
```

#### 4.2 ResolutionResult Enum

```rust
enum ResolutionResult {
    Resolved { 
        winner: ConflictWinner, 
        strategy: ResolutionStrategy,
    },
    Unresolvable { 
        conflict: ConflictType, 
        reason: UnresolvableReason,
    },
    Deferred { 
        conflict: ConflictType, 
        retry_at: TimestampMs,
    },
}
```

### 5. Invariants (INV-*)

- **INV-001**: After resolution, exactly one operation succeeds or the conflict is marked Unresolvable
- **INV-002**: No state is lost during resolution; the losing operation's effects are preserved in a compensation log
- **INV-003**: Fence token monotonicity is preserved: once a fence token T is accepted, no operation with token < T can be accepted for the same instance
- **INV-004**: Terminal states (Completed, Failed, Cancelled) reject all conflicting transitions
- **INV-005**: Sequence numbers are totally ordered post-resolution; no fork occurs
- **INV-006**: Lease conflicts never result in both holders retaining the lease

### 6. Error Taxonomy

```rust
struct MergeConflictError {
    category: ErrorCategory,
    detail: ErrorDetail,
    context: ConflictContext,
}

enum ErrorCategory {
    DetectionFailure,      // Conflict detection itself failed
    ResolutionFailure,     // Resolution strategy could not be applied
    InvariantViolation,    // Resolution would violate an invariant
    ResourceExhaustion,    // Cannot allocate resources to resolve
    Timeout,               // Resolution timed out
}

enum ErrorDetail {
    AmbiguousConflict(ConflictType),
    CircularDependency(Vec<InstanceId>),
    StaleWinner(InstanceId, SequenceNumber),
    MultipleActiveHolders(InstanceId),
    SequenceRegress(InstanceId),
    FenceRegression(FenceToken, FenceToken),
    TerminalViolation(LifecycleState),
}
```

### 7. Resolution Protocol

1. **Detect**: Identify conflict type (Lease, StateTransition, Sequence, Fence)
2. **Classify**: Determine if conflict is resolvable or requires escalation
3. **Apply Strategy**: Execute resolution strategy based on conflict type and system policy
4. **Verify Invariants**: Confirm INV-001 through INV-006 hold post-resolution
5. **Record**: Log resolution outcome to the conflict journal

### 8. Constraints

- Resolution must complete within one round-trip to storage; no multi-phase resolution
- The auto-resolver never performs manual intervention; it escalates Unresolvable conflicts
- Resolution must preserve the monotonicity of fence tokens
- A conflict marked Deferred must not block other non-conflicting operations on the same instance

### 9. Relevant Files

- `crates/vo-types/src/state/transition.rs` (state transition logic)
- `crates/vo-types/src/state/lifecycle.rs` (lifecycle state machine)
- `crates/vo-types/src/integer_types.rs` (FenceToken, SequenceNumber)
- `crates/vo-types/src/errors.rs` (error taxonomy pattern)

### 10. Acceptance Criteria

- Conflict types compile and cover all observed conflict scenarios in the system
- Resolution strategies are exhaustive for all defined conflict types
- All invariants (INV-001 through INV-006) are formally stated and testable
- Error taxonomy covers both detection failures and resolution failures
- The contract is self-contained and does not reference nonexistent crates or files