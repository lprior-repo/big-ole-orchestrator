## Contract: Distributed Transaction Coordinator

### 1. Purpose

Defines the contract for the veloxide distributed transaction coordinator — the pure typed system that governs two-phase commit (2PC) protocol state transitions across multiple resource participants, enforces invariants, and classifies errors. This contract establishes the authoritative types, invariants, and error taxonomy for all distributed transaction coordination in the vo-types crate.

### 2. Source ADRs

- `docs/adr/v2/ADR-041-v2-managed-connector-runtime-contract.md` (connector prepare/commit/reconcile semantics)
- `docs/adr/v2/ADR-034-v2-saga-compensation-and-reversibility.md` (compensation and rollback semantics)
- `docs/adr/v2/ADR-029-v2-execution-leases-and-fencing.md` (fencing for concurrent coordinator access)
- `docs/adr/v2/ADR-027-v2-deterministic-event-sourced-replay.md` (replay semantics for recovery)

### 3. Type Definitions

#### 3.1 TransactionState

Lifecycle state of a distributed transaction coordinator following the 2PC protocol.

```
TransactionState {
  Init,          // Coordinator initialized, no participants enrolled
  Enrolling,     // Enrolling participants, preparing to send prepare
  Preparing,     // Prepare sent to all participants, awaiting responses
  Prepared,      // All participants voted "prepared" — ready to commit
  Committing,    // Sending commit to all participants
  Committed,     // Transaction committed successfully (terminal)
  RollingBack,   // Sending rollback to all participants
  RolledBack,    // Transaction rolled back successfully (terminal)
  Aborted,       // Transaction aborted due to timeout/failure (terminal)
  Ambiguous,     // Outcome unknown — recovery required (non-terminal)
}
```

Terminal states: `Committed`, `RolledBack`, `Aborted`.
`Ambiguous` is NOT terminal — it can be reconciled via recovery transitions.

#### 3.2 ParticipantStatus

Status of a single participant in a distributed transaction.

```
ParticipantStatus {
  Enrolled,       // Enrolled but not yet responded to prepare
  Prepared,       // Voted "prepared" — can commit or rollback
  VotedRollback,  // Voted rollback or timed out
  Committed,      // Has committed the transaction
  RolledBack,     // Has rolled back the transaction
  Unknown,        // Status unknown — reconcile required
}
```

#### 3.3 CoordinatorDecision

Decision made by the coordinator after the prepare phase.

```
CoordinatorDecision {
  Commit,    // All participants voted prepared — proceed with commit
  Rollback,  // One or more voted rollback — abort transaction
}
```

#### 3.4 CoordinatorTransition

Events that drive TransactionState transitions.

```
CoordinatorTransition {
  BeginEnroll,          // Begin enrolling participants
  BeginPrepare,         // All enrolled, begin prepare phase
  ParticipantPrepared,  // A participant responded prepared
  ParticipantRollback,  // A participant voted rollback
  AllResponded,         // All participants have responded
  DecideCommit,         // Coordinator decided to commit
  DecideRollback,       // Coordinator decided to rollback
  Timeout,              // Coordinator timed out waiting for responses
  Recover,              // Coordinator crashed and is recovering
  ReconcileCommitted,   // Recovery determined transaction committed
  ReconcileRolledBack,  // Recovery determined transaction rolled back
  ReconcileRetry,       // Recovery could not determine outcome
}
```

#### 3.5 ParticipantVote

Vote cast by a participant during the prepare phase.

```
ParticipantVote {
  Prepared,  // Participant is prepared to commit
  Rollback,  // Participant wishes to rollback
}
```

#### 3.6 TransactionRecord

Durable record of a distributed transaction.

```
TransactionRecord {
  transaction_id: String,                       // Unique, non-empty (INV-TC-001)
  state: TransactionState,
  decision: Option<CoordinatorDecision>,         // Set after prepare phase
  participants: Vec<ParticipantRecord>,
  created_at: Option<TimestampMs>,
  prepared_at: Option<TimestampMs>,
  committed_at: Option<TimestampMs>,
}
```

#### 3.7 ParticipantRecord

Record of a single participant in a distributed transaction.

```
ParticipantRecord {
  participant_id: String,                // Unique, non-empty (INV-TC-002)
  status: ParticipantStatus,
  vote: Option<ParticipantVote>,
}
```

### 4. State Transition Graph

```
Init ──BeginEnroll──→ Enrolling ──BeginPrepare──→ Preparing
                                                    │
                                  ParticipantPrepared│ (stays in Preparing)
                                  ParticipantRollback│ (stays in Preparing)
                                  AllResponded───────┼──→ Prepared
                                  Timeout────────────┼──→ Aborted
                                                    │
Preparing ──AllResponded──→ Prepared ──DecideCommit──→ Committing ──AllResponded──→ Committed
                                  │                                        │
                           DecideRollback──→ RollingBack──AllResponded──→RolledBack
                                  │                                        │
                               Timeout──→ Aborted                        Timeout──→ Ambiguous
                                                                           │
                               RollingBack──Timeout──→ Ambiguous          │
                                                                           │
Ambiguous ──ReconcileCommitted──→ Committed (terminal)
          ──ReconcileRolledBack──→ RolledBack (terminal)
          ──ReconcileRetry──→ Ambiguous (retry loop)

Any non-terminal state ──Recover──→ Ambiguous
```

### 5. Invariants (INV-TC-*)

- **INV-TC-001**: `TransactionRecord::new` returns `None` if `transaction_id` is empty
- **INV-TC-002**: `ParticipantRecord::new` returns `None` if `participant_id` is empty
- **INV-TC-003**: Terminal states (`Committed`, `RolledBack`, `Aborted`) reject all transitions — `apply_coordinator_transition` returns `TerminalStateTransition`
- **INV-TC-004**: `Ambiguous` is not terminal — it accepts recovery transitions (`ReconcileCommitted`, `ReconcileRolledBack`, `ReconcileRetry`)
- **INV-TC-005**: `Recover` is valid from any non-terminal state and always transitions to `Ambiguous`
- **INV-TC-006**: `Timeout` in `Preparing` transitions to `Aborted` (known outcome — no participants committed)
- **INV-TC-007**: `Timeout` in `Committing` or `RollingBack` transitions to `Ambiguous` (participants may have committed)
- **INV-TC-008**: `Preparing` state absorbs `ParticipantPrepared` and `ParticipantRollback` without changing state (votes accumulate)
- **INV-TC-009**: `AllResponded` is valid from `Preparing` → `Prepared`, `Committing` → `Committed`, `RollingBack` → `RolledBack`
- **INV-TC-010**: `Prepared` state only accepts `DecideCommit`, `DecideRollback`, and `Timeout`
- **INV-TC-011**: All invalid (state, event) combinations return `InvalidTransition` (no panics)
- **INV-TC-012**: `apply_coordinator_transition` is total over all 10×12 = 120 (state, event) combinations — no panics
- **INV-TC-013**: Serde round-trip preserves equality for all enum types (`TransactionState`, `ParticipantStatus`, `CoordinatorDecision`, `CoordinatorTransition`, `ParticipantVote`)
- **INV-TC-014**: `is_terminal()` returns `true` for `Committed`, `RolledBack`, `Aborted` and `false` for all other states
- **INV-TC-015**: `all_variants()` returns all declared variants in declaration order for each enum type

### 6. Error Taxonomy

```rust
enum CoordinatorTransitionError {
    // Attempted transition from a terminal state (Committed, RolledBack, Aborted)
    TerminalStateTransition,

    // Event not valid for the current state
    InvalidTransition,

    // Required votes not yet received (reserved for future use)
    InsufficientVotes,
}
```

#### 6.1 Error Categories

| Error Variant | Category | Recoverable |
|--------------|----------|-------------|
| `TerminalStateTransition` | ProtocolViolation | No (terminal is final) |
| `InvalidTransition` | ProtocolViolation | No (wrong state/event combination) |
| `InsufficientVotes` | TemporalBlock | Yes (wait for more votes) |

#### 6.2 Error Display Format

- `TerminalStateTransition`: "Cannot transition from terminal transaction state"
- `InvalidTransition`: "Invalid transaction coordinator state transition"
- `InsufficientVotes`: "Insufficient participant votes to transition"

### 7. Transition Protocol

1. **Validate**: Check current state is non-terminal (or `Ambiguous` for recovery)
2. **Match**: Check event is in the valid transition set for current state via exhaustive match
3. **Apply**: Compute new state — `apply_coordinator_transition(state, event) -> Result<TransactionState, CoordinatorTransitionError>`
4. **Record**: Emit transition for event sourcing/replay
5. **Recover**: On crash, `Recover` from any non-terminal state → `Ambiguous`, then reconcile

### 8. Recovery Protocol

Per ADR-041 §3, a coordinator timeout does not mean the transaction failed.

1. **Crash during Preparing**: Recovery yields `Aborted` (no participants committed, safe to abort)
2. **Crash during Committing**: Recovery yields `Ambiguous` (some participants may have committed)
3. **Crash during RollingBack**: Recovery yields `Ambiguous` (some participants may have rolled back)
4. **Reconciliation**: Query each participant to determine actual outcome
   - All committed → `ReconcileCommitted` → `Committed`
   - All rolled back → `ReconcileRolledBack` → `RolledBack`
   - Mixed/unknown → `ReconcileRetry` → stay `Ambiguous`, retry later

### 9. Constraints

- The transaction coordinator types are pure: no I/O, no engine integration, no async
- Exhaustive match in `apply_coordinator_transition` ensures compiler catches missing (state, event) combinations
- All transitions are total over valid (state, event) pairs — no panics
- Terminal states are absorbing: no outward transitions
- `Ambiguous` is the only non-terminal state with recovery transitions
- Participant vote accumulation (in `Preparing` state) is handled externally; the state machine only tracks coordinator-level state
- No support for three-phase commit (3PC) or nested transactions in this contract
- Thread safety: types require external synchronization for concurrent access

### 10. Relevant Files

- `crates/vo-types/src/tx_coordinator/mod.rs` (module re-exports)
- `crates/vo-types/src/tx_coordinator/types.rs` (types, state machine, calc layer)
- `crates/vo-types/src/tx_coordinator/transition.rs` (transition helpers)
- `crates/vo-types/src/tx_coordinator/tests.rs` (unit tests)
- `crates/vo-types/src/tx_coordinator/proptests.rs` (property-based tests)
- `crates/vo-types/src/tx_coordinator/verification.rs` (Kani formal verification)

### 11. Acceptance Criteria

- [x] All types (`TransactionState`, `ParticipantStatus`, `CoordinatorDecision`, `CoordinatorTransition`, `ParticipantVote`, `TransactionRecord`, `ParticipantRecord`) compile and are well-formed
- [x] All invariants (INV-TC-001 through INV-TC-015) are formally stated and testable
- [x] `apply_coordinator_transition` is total over all 10×12 = 120 combinations without panics
- [x] Error taxonomy is exhaustive: every failure mode has a corresponding error variant
- [x] Terminal states reject all transitions with `TerminalStateTransition`
- [x] Recovery path (Ambiguous → Committed/RolledBack) is well-defined
- [x] Serde round-trip preserves equality for all enum types
- [x] Kani verification harnesses cover exhaustiveness and constructor validation
- [x] Contract is self-contained and references existing ADR documentation
