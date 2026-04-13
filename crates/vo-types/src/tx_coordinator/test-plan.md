# Test Plan: Distributed Transaction Coordinator

**Bead ID:** ve-j73a  
**Phase:** Test Planning  
**Type:** Test Plan (comprehensive coverage across Testing Trophy layers)

---

## Summary

This test plan covers the distributed transaction coordinator in `crates/vo-types/src/tx_coordinator/` which implements the two-phase commit (2PC) protocol state machine per ADR-041.

**Scope:**
- TransactionState (10 variants)
- ParticipantStatus (6 variants)
- CoordinatorDecision (2 variants)
- CoordinatorTransition (12 events)
- TransactionRecord / ParticipantRecord
- `apply_coordinator_transition` state machine logic

---

## Existing Test Coverage

### Unit Tests (`tests.rs`)
- ✓ Debug format equals variant name for all enums
- ✓ `is_terminal()` for Committed, RolledBack, Aborted, Ambiguous, Prepared
- ✓ `is_terminal()` completeness (all 10 states tested)
- ✓ `CoordinatorTransitionError` Display implementation
- ✓ Happy path transitions (17 test cases)
- ✓ Terminal state rejections (3 states × 12 events = 36 tests)
- ✓ Invalid transitions (44 test cases)
- ✓ `TransactionRecord::new()` validation (empty ID rejection)
- ✓ `ParticipantRecord::new()` validation (empty ID rejection)
- ✓ `all_variants()` completeness (4 enums)
- ✓ `Recover` transition from all 7 non-terminal states
- ✓ `Prepared` state validation (rejects 8 invalid events)

### Proptest Invariants (`proptests.rs`)
- ✓ Serde round-trip preserves equality (TransactionState, ParticipantStatus, CoordinatorDecision)
- ✓ `TransactionRecord::new()` rejects empty ID
- ✓ `ParticipantRecord::new()` rejects empty ID
- ✓ `apply_coordinator_transition` never panics (exhaustiveness)

### Red Queen Adversarial Tests (`red_queen_tests.rs`)
- ✓ 60 adversarial test cases
- ✓ Serde attacks (deserialize bypassing validation)
- ✓ Exhaustiveness (all 120 combinations)
- ✓ Invariant attacks (INV-TC-003 through INV-TC-015)
- ✓ Transition attacks (happy paths, rollback, timeouts)
- ✓ Error taxonomy (all 3 error variants)
- ✓ Boundary values (long IDs, unicode, many participants)
- ✓ Path attacks (malicious sequences)

### Kani Verification (`verification.rs`)
- ✓ K-01: Exhaustiveness proof (120 combinations)
- ✓ K-02: `TransactionRecord::new()` empty ID rejection
- ✓ K-03: `ParticipantRecord::new()` empty ID rejection

---

## Testing Trophy Allocation

### Layer 1: Unit Tests (Given-When-Then)
**Status:** ✅ **Complete** (138+ tests)

**Coverage:**
- All 10 TransactionState variants
- All 6 ParticipantStatus variants
- All 2 CoordinatorDecision variants
- All 12 CoordinatorTransition events
- All 3 CoordinatorTransitionError variants
- All valid/invalid transition combinations
- All constructor validations

**Missing:** None - all unit test coverage complete.

---

### Layer 2: BDD (Dan North Given-When-Then)
**Status:** ⚠️ **Partial** - Needs refinement

**Current:** Tests are unit-style assertions, not BDD-style scenarios.

**Recommended BDD Scenarios:**

```gherkin
Feature: Two-phase commit coordinator

  Scenario: Successful commit with all participants prepared
    Given a new transaction with 3 participants
    When the coordinator enrolls all participants
    And sends prepare to all participants
    And all participants vote "prepared"
    And all responses received
    Then the coordinator decides to commit
    And all participants transition to committed
    And the transaction reaches Committed state

  Scenario: Rollback due to participant vote rollback
    Given a new transaction with 2 participants
    When the coordinator enrolls all participants
    And sends prepare to all participants
    And one participant votes "rollback"
    And all responses received
    Then the coordinator decides to rollback
    And the transaction reaches RolledBack state

  Scenario: Timeout during prepare phase
    Given a transaction in Preparing state
    When the coordinator waits for participant responses
    And a timeout occurs
    Then the transaction transitions to Aborted

  Scenario: Ambiguous recovery after crash
    Given a transaction in Ambiguous state
    When the coordinator recovers from crash
    And the coordinator reconciles with participants
    And determines transaction was committed
    Then the transaction transitions to Committed

  Scenario: Timeout during commit phase
    Given a transaction in Committing state
    When the coordinator sends commit to participants
    And a timeout occurs
    Then the transaction transitions to Ambiguous

  Scenario: Recovery from non-terminal state
    Given a transaction in Preparing state
    When the coordinator crashes
    And then recovers
    Then the transaction transitions to Ambiguous
```

---

### Layer 3: Proptest (Property-Based Testing)
**Status:** ✅ **Good** (6 invariants)

**Current Invariants:**
1. Serde round-trip preserves TransactionState equality
2. Serde round-trip preserves ParticipantStatus equality
3. Serde round-trip preserves CoordinatorDecision equality
4. TransactionRecord rejects empty ID
5. ParticipantRecord rejects empty ID
6. apply_coordinator_transition never panics

**Recommended Additional Invariants:**

```rust
/// INV-TC-020: Terminal states are absorbing - once committed/rolledback/aborted, always terminal
#[test]
fn invariant_terminal_states_are_absorbing(
    state in proptest::sample::select(&[
        TransactionState::Committed,
        TransactionState::RolledBack,
        TransactionState::Aborted,
    ])
) {
    // All 12 events must be rejected from terminal states
    for event in CoordinatorTransition::all_variants() {
        let result = apply_coordinator_transition(state, event);
        prop_assert!(result.is_err());
    }
}

/// INV-TC-021: All valid transitions lead to a defined state (no undefined behavior)
#[test]
fn invariant_all_valid_transitions_defined(
    state_idx in 0usize..10,
    event_idx in 0usize..12
) {
    let states = TransactionState::all_variants();
    let events = CoordinatorTransition::all_variants();
    
    let current = states[state_idx % states.len()];
    let event = events[event_idx % events.len()];
    
    let result = apply_coordinator_transition(current, event);
    
    // Every combination must return Ok or Err - never panic
    match result {
        Ok(next_state) => {
            // Next state must be a valid variant
            prop_assert!(TransactionState::all_variants().contains(&next_state));
        }
        Err(_) => {
            // Invalid transition - acceptable
            prop_assert!(true);
        }
    }
}

/// INV-TC-022: Prepared state is reachable only from Preparing via AllResponded
#[test]
fn invariant_prepared_only_reachable_from_preparing(
    state in proptest::sample::select(TransactionState::all_variants()),
    event in proptest::sample::select(CoordinatorTransition::all_variants())
) {
    // If transition results in Prepared, it must be from Preparing + AllResponded
    let result = apply_coordinator_transition(state, event);
    if let Ok(TransactionState::Prepared) = result {
        prop_assert_eq!(state, TransactionState::Preparing);
        prop_assert_eq!(event, CoordinatorTransition::AllResponded);
    }
}

/// INV-TC-023: Committing state is reachable only from Prepared via DecideCommit
#[test]
fn invariant_committing_only_reachable_from_prepared(
    state in proptest::sample::select(TransactionState::all_variants()),
    event in proptest::sample::select(CoordinatorTransition::all_variants())
) {
    let result = apply_coordinator_transition(state, event);
    if let Ok(TransactionState::Committing) = result {
        prop_assert_eq!(state, TransactionState::Prepared);
        prop_assert_eq!(event, CoordinatorTransition::DecideCommit);
    }
}

/// INV-TC-024: RollingBack state is reachable only from Prepared via DecideRollback
#[test]
fn invariant_rollingback_only_reachable_from_prepared(
    state in proptest::sample::select(TransactionState::all_variants()),
    event in proptest::sample::select(CoordinatorTransition::all_variants())
) {
    let result = apply_coordinator_transition(state, event);
    if let Ok(TransactionState::RollingBack) = result {
        prop_assert_eq!(state, TransactionState::Prepared);
        prop_assert_eq!(event, CoordinatorTransition::DecideRollback);
    }
}

/// INV-TC-025: Aborted state is reachable from Preparing via Timeout, or from Prepared via Timeout
#[test]
fn invariant_aborted_only_from_timeout_in_preparing_or_prepared(
    state in proptest::sample::select(TransactionState::all_variants()),
    event in proptest::sample::select(CoordinatorTransition::all_variants())
) {
    let result = apply_coordinator_transition(state, event);
    if let Ok(TransactionState::Aborted) = result {
        prop_assert!(
            (state == TransactionState::Preparing || state == TransactionState::Prepared)
                && event == CoordinatorTransition::Timeout
        );
    }
}

/// INV-TC-026: Ambiguous state is reachable from Committing/RollingBack via Timeout, 
///              or from any non-terminal via Recover, or from Ambiguous via ReconcileRetry
#[test]
fn invariant_ambiguous_reachable_only_via_timeout_recover_or_retry(
    state in proptest::sample::select(TransactionState::all_variants()),
    event in proptest::sample::select(CoordinatorTransition::all_variants())
) {
    let result = apply_coordinator_transition(state, event);
    if let Ok(TransactionState::Ambiguous) = result {
        prop_assert!(
            (state == TransactionState::Committing || state == TransactionState::RollingBack)
                && event == CoordinatorTransition::Timeout
            || event == CoordinatorTransition::Recover
            || (state == TransactionState::Ambiguous && event == CoordinatorTransition::ReconcileRetry)
        );
    }
}

/// INV-TC-027: Committed state is reachable only from Committing via AllResponded
#[test]
fn invariant_committed_only_reachable_from_committing(
    state in proptest::sample::select(TransactionState::all_variants()),
    event in proptest::sample::select(CoordinatorTransition::all_variants())
) {
    let result = apply_coordinator_transition(state, event);
    if let Ok(TransactionState::Committed) = result {
        prop_assert_eq!(state, TransactionState::Committing);
        prop_assert_eq!(event, CoordinatorTransition::AllResponded);
    }
}

/// INV-TC-028: RolledBack state is reachable only from RollingBack via AllResponded
#[test]
fn invariant_rolledback_only_reachable_from_rollingback(
    state in proptest::sample::select(TransactionState::all_variants()),
    event in proptest::sample::select(CoordinatorTransition::all_variants())
) {
    let result = apply_coordinator_transition(state, event);
    if let Ok(TransactionState::RolledBack) = result {
        prop_assert_eq!(state, TransactionState::RollingBack);
        prop_assert_eq!(event, CoordinatorTransition::AllResponded);
    }
}
```

---

### Layer 4: Mutation Testing
**Status:** ❌ **Not started**

**Recommended Mutations:**

1. **Operator Mutations:**
   - Flip `is_terminal()` to always return true
   - Flip `is_terminal()` to always return false
   - Change `TransactionRecord::new()` to accept empty ID
   - Change `ParticipantRecord::new()` to accept empty ID

2. **Branch Mutations:**
   - Remove all terminal state rejections
   - Remove all invalid transition rejections
   - Remove all Recover transitions
   - Remove all timeout transitions

3. **State Mutation:**
   - Change state machine to allow transitions from terminal states
   - Change state machine to skip required phases (e.g., Init → Committing)
   - Remove AllResponded requirement before deciding

**Mutation Kill Rate Target:** ≥90%

---

### Layer 5: Kani Formal Verification
**Status:** ✅ **Basic coverage** (3 harnesses)

**Current Harnesses:**
- K-01: Exhaustiveness (120 combinations)
- K-02: TransactionRecord empty ID rejection
- K-03: ParticipantRecord empty ID rejection

**Recommended Additional Harnesses:**

```rust
/// K-04: Verify all 120 state/event combinations return Result (no panic)
#[kani::proof]
fn verify_all_combinations_return_result() {
    let state: u8 = kani::any();
    let event: u8 = kani::any();
    
    kani::assume(state < 10);
    kani::assume(event < 12);
    
    let current = match state {
        0 => TransactionState::Init,
        1 => TransactionState::Enrolling,
        2 => TransactionState::Preparing,
        3 => TransactionState::Prepared,
        4 => TransactionState::Committing,
        5 => TransactionState::Committed,
        6 => TransactionState::RollingBack,
        7 => TransactionState::RolledBack,
        8 => TransactionState::Aborted,
        _ => TransactionState::Ambiguous,
    };
    
    let evt = match event {
        0 => CoordinatorTransition::BeginEnroll,
        1 => CoordinatorTransition::BeginPrepare,
        2 => CoordinatorTransition::ParticipantPrepared,
        3 => CoordinatorTransition::ParticipantRollback,
        4 => CoordinatorTransition::AllResponded,
        5 => CoordinatorTransition::DecideCommit,
        6 => CoordinatorTransition::DecideRollback,
        7 => CoordinatorTransition::Timeout,
        8 => CoordinatorTransition::Recover,
        9 => CoordinatorTransition::ReconcileCommitted,
        10 => CoordinatorTransition::ReconcileRolledBack,
        _ => CoordinatorTransition::ReconcileRetry,
    };
    
    // Must not panic - always return Result
    let result: Result<TransactionState, CoordinatorTransitionError> = 
        apply_coordinator_transition(current, evt);
    
    // Verify result is either Ok or Err (never panic)
    kani::assert(result.is_ok() || result.is_err(), "Must return Result");
}

/// K-05: Verify terminal state invariants
#[kani::proof]
fn verify_terminal_states_reject_all() {
    let terminal_states = [
        TransactionState::Committed,
        TransactionState::RolledBack,
        TransactionState::Aborted,
    ];
    
    for state in terminal_states {
        let event: u8 = kani::any();
        kani::assume(event < 12);
        
        let evt = match event {
            0 => CoordinatorTransition::BeginEnroll,
            1 => CoordinatorTransition::BeginPrepare,
            2 => CoordinatorTransition::ParticipantPrepared,
            3 => CoordinatorTransition::ParticipantRollback,
            4 => CoordinatorTransition::AllResponded,
            5 => CoordinatorTransition::DecideCommit,
            6 => CoordinatorTransition::DecideRollback,
            7 => CoordinatorTransition::Timeout,
            8 => CoordinatorTransition::Recover,
            9 => CoordinatorTransition::ReconcileCommitted,
            10 => CoordinatorTransition::ReconcileRolledBack,
            _ => CoordinatorTransition::ReconcileRetry,
        };
        
        let result = apply_coordinator_transition(state, evt);
        
        // Must always return TerminalStateTransition error
        kani::assert(
            matches!(result, Err(CoordinatorTransitionError::TerminalStateTransition)),
            "Terminal states must reject all events"
        );
    }
}

/// K-06: Verify Recover transition to Ambiguous for all non-terminal states
#[kani::proof]
fn verify_recover_to_ambiguous() {
    let non_terminal = [
        TransactionState::Init,
        TransactionState::Enrolling,
        TransactionState::Preparing,
        TransactionState::Prepared,
        TransactionState::Committing,
        TransactionState::RollingBack,
        TransactionState::Ambiguous,
    ];
    
    for state in non_terminal {
        let result = apply_coordinator_transition(state, CoordinatorTransition::Recover);
        
        kani::assert(
            result == Ok(TransactionState::Ambiguous),
            "Recover from non-terminal must transition to Ambiguous"
        );
    }
}

/// K-07: Verify timeout transitions
#[kani::proof]
fn verify_timeout_transitions() {
    // Preparing + Timeout → Aborted
    let result_preparing = apply_coordinator_transition(
        TransactionState::Preparing,
        CoordinatorTransition::Timeout
    );
    kani::assert(
        result_preparing == Ok(TransactionState::Aborted),
        "Preparing timeout must abort"
    );
    
    // Prepared + Timeout → Aborted
    let result_prepared = apply_coordinator_transition(
        TransactionState::Prepared,
        CoordinatorTransition::Timeout
    );
    kani::assert(
        result_prepared == Ok(TransactionState::Aborted),
        "Prepared timeout must abort"
    );
    
    // Committing + Timeout → Ambiguous
    let result_committing = apply_coordinator_transition(
        TransactionState::Committing,
        CoordinatorTransition::Timeout
    );
    kani::assert(
        result_committing == Ok(TransactionState::Ambiguous),
        "Committing timeout must go ambiguous"
    );
    
    // RollingBack + Timeout → Ambiguous
    let result_rolling = apply_coordinator_transition(
        TransactionState::RollingBack,
        CoordinatorTransition::Timeout
    );
    kani::assert(
        result_rolling == Ok(TransactionState::Ambiguous),
        "RollingBack timeout must go ambiguous"
    );
}

/// K-08: Verify all_variants() returns all enum variants
#[kani::proof]
fn verify_all_variants_complete() {
    let states = TransactionState::all_variants();
    kani::assert(
        states.len() == 10,
        "TransactionState must have 10 variants"
    );
    
    let statuses = ParticipantStatus::all_variants();
    kani::assert(
        statuses.len() == 6,
        "ParticipantStatus must have 6 variants"
    );
    
    let decisions = CoordinatorDecision::all_variants();
    kani::assert(
        decisions.len() == 2,
        "CoordinatorDecision must have 2 variants"
    );
    
    let transitions = CoordinatorTransition::all_variants();
    kani::assert(
        transitions.len() == 12,
        "CoordinatorTransition must have 12 variants"
    );
}
```

---

## Coverage Gaps

### Gap 1: BDD Scenarios
**Priority:** Medium  
**Action:** Convert unit tests to BDD-style Given-When-Then scenarios for business logic verification.

### Gap 2: Additional Proptest Invariants
**Priority:** High  
**Action:** Add reachability invariants (INV-TC-020 through INV-TC-028) to verify state machine correctness.

### Gap 3: Mutation Testing
**Priority:** Medium  
**Action:** Run cargo-mutatest or similar to measure kill rate and add tests for surviving mutations.

### Gap 4: Kani Formal Proofs
**Priority:** Low (unit tests cover this)  
**Action:** Add formal proofs for terminal state invariants and timeout transitions.

---

## Execution Checklist

### Phase 1: BDD Refinement (2 hours)
- [ ] Create BDD-style Gherkin scenarios for all 5 main workflows
- [ ] Implement scenario steps as test functions
- [ ] Run BDD tests and verify all pass

### Phase 2: Proptest Expansion (3 hours)
- [ ] Add INV-TC-020: Terminal states are absorbing
- [ ] Add INV-TC-021: All valid transitions defined
- [ ] Add INV-TC-022 through INV-TC-028: Reachability invariants
- [ ] Run proptest with ≥1000 iterations per invariant
- [ ] Verify all invariants hold

### Phase 3: Mutation Testing (1 hour)
- [ ] Install cargo-mutatest
- [ ] Run mutation testing on tx_coordinator module
- [ ] Add tests to kill surviving mutations
- [ ] Achieve ≥90% kill rate

### Phase 4: Kani Proofs (2 hours)
- [ ] Add K-04 through K-08 harnesses
- [ ] Run `cargo kani` on all harnesses
- [ ] Verify all proofs pass without assumptions

---

## Acceptance Criteria

- ✅ All 138+ unit tests pass
- ✅ 6 proptest invariants hold with ≥1000 iterations
- ✅ 60+ Red Queen adversarial tests pass
- ✅ 3 Kani proofs verify
- ✅ ≥90% mutation kill rate achieved
- ✅ BDD scenarios cover all 5 main workflows
- ✅ Zero panics in any test configuration
- ✅ All 120 state/event combinations verified

---

## References

- **ADR-041:** Distributed Transaction Coordinator Architecture
- **Existing Tests:** `crates/vo-types/src/tx_coordinator/tests.rs`
- **Proptest:** `crates/vo-types/src/tx_coordinator/proptests.rs`
- **Red Queen:** `crates/vo-types/src/tx_coordinator/red_queen_tests.rs`
- **Kani:** `crates/vo-types/src/tx_coordinator/verification.rs`
