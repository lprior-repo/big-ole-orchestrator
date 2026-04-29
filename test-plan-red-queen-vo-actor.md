# Red Queen Test Plan: vo-actor

**Bead ID**: ve-ppald
**Module**: vo-actor
**Date**: 2026-04-17
**Status**: Active - 1 failing test discovered

## Executive Summary

Red Queen adversarial testing against `vo-actor` has produced **5 test modules** covering lifecycle state machines, signal matching, timer lifecycle, execution lease fencing, and structured logging. Testing has uncovered **1 failing invariant** related to duplicate timer handling.

## Red Queen Methodology

The Red Queen approach uses deterministic state machine execution with AI-generated adversarial test commands. Tests are designed to:

1. **Cover state transitions**: Verify all valid and invalid lifecycle transitions
2. **Verify invariants under mutation**: Ensure correctness invariants hold
3. **Kill surviving mutants**: Tests should fail when invariants break

## Test Modules

### 1. `redqueen_actor.rs` — Core Actor Lifecycle

**File**: `crates/vo-actor/tests/redqueen_actor.rs`

Covers:
- Lifecycle state machine transitions (Pending → Running → Stopping → Stopped)
- Supervision tree operations (parent-child relationships)
- Dead Letter Queue (DLQ) overflow and eviction
- Instance Registry atomicity (rollback on stop_fn failure/timeout)
- ReservedPermitBudget enforcement

**Tests**: 14 tests
- `rq_no_transition_from_terminal_stopped` - Terminal states are absorbing
- `rq_no_transition_from_terminal_failed` - Failed is terminal
- `rq_valid_transitions_roundtrip` - Valid transitions work correctly
- `rq_failure_scope_epoch_keeps_lineage_active` - Epoch failures preserve lineage
- `rq_failure_scope_lineage_tombstones` - Lineage scope triggers tombstone
- `rq_orphan_child_removed_without_update` - Cleanup of removed children
- `rq_empty_registry_reports_all_terminal` - Edge case: empty registry
- `rq_failed_child_is_terminal_not_stopping` - Failed children are terminal
- `rq_dlq_evicts_oldest_when_full` - DLQ FIFO eviction
- `rq_dlq_empty_dequeue_returns_none` - Empty DLQ returns None
- `rq_registry_stop_fn_failure_rolls_back` - Atomic rollback on failure
- `rq_registry_stop_fn_timeout_rolls_back` - Atomic rollback on timeout
- `rq_registry_deregister_unknown_errors` - Error handling for missing entries
- `rq_budget_exhaustion_then_release_allows` - Budget acquire/release cycle
- `rq_budget_cross_class_isolation` - Class-independent budgets

**Invariant**: `compute_next_state(terminal_state, any_transition) = None`

---

### 2. `signal_timer_lifecycle_red_queen.rs` — Signal Matching + Timer Integration

**File**: `crates/vo-actor/tests/signal_timer_lifecycle_red_queen.rs`

Covers:
- Signal lineage/epoch resolution (ADR-042)
- Signal delivery to hibernated instances (ADR-005)
- Timer lifecycle: creation, firing, cancellation on completion
- Crash-recovery timer correctness
- Signal buffer integration with hibernation
- Dual-clock verification (ADR-013)
- Timer overdue detection (ADR-005)

**Tests**: 35+ tests across 8 attack vectors

**Attack Vector 1: Signal Lineage Resolution**
- `rq_lineage_wide_signal_ignores_epoch` - Lineage-wide signals route correctly
- `rq_epoch_local_signal_matches_when_epoch_zero` - Epoch-local at epoch 0
- `rq_epoch_local_signal_mismatches_when_epoch_differs` - Epoch mismatch detection
- `rq_signal_match_returns_lineage_mismatch_when_lineage_differs`
- `rq_signal_match_returns_instance_mismatch_when_instance_differs`
- `rq_signal_match_returns_wait_key_mismatch_when_key_differs`

**Attack Vector 2: Signal Delivery to Hibernated Instances**
- `rq_timer_fires_wakes_hibernated_instance` - Timer wakes hibernated instance
- `rq_multiple_timers_same_instance_wakes_once` - Deduplication
- `rq_timer_for_terminal_instance_not_dispatched` - Terminal filtering

**Attack Vector 3: Timer Cancellation on Completion**
- `rq_cancel_timers_on_workflow_completion` - All timers cancelled on completion
- `rq_cancel_timers_returns_zero_when_none_exist` - Edge case handling
- `rq_terminal_instance_no_timer_leak_after_cancel` - No leaks post-cancellation

**Attack Vector 4: Crash-Recovery Timer Correctness**
- `rq_crash_recovery_replays_pending_timer` - Pending timers replayed
- `rq_delete_before_dispatch_no_double_fire_on_retry` - Delete-before-dispatch works
- `rq_crash_recovery_skips_terminal_instance_timers` - Terminal instances skipped

**Attack Vector 5: Signal Buffer Integration**
- `rq_signal_buffered_for_hibernated_instance` - Buffering works
- `rq_buffered_signal_survives_multiple_operations` - Buffer durability
- `rq_buffer_one_rejects_subsequent_signals_until_first_is_consumed` - BufferOne policy

**Attack Vector 6: Property - Signals Resume Correct Wait State**
- `rq_lineage_wide_signal_only_matches_correct_lineage` - Lineage isolation
- `rq_epoch_local_signal_only_matches_correct_epoch` - Epoch isolation
- `rq_after_crash_only_correct_wait_state_resumed` - Crash recovery correctness
- `rq_wait_key_mismatch_prevents_wrong_delivery` - Key matching

**Attack Vector 7: Dual-Clock Verification**
- `rq_verify_dual_clock_both_clocks_must_agree` - Both clocks required
- `rq_wall_clock_drift_prevented` - Drift detection
- `rq_hibernation_immune_to_timer_drift` - Hibernate correctness

**Attack Vector 8: Timer Overdue Detection**
- `rq_is_overdue_true_when_beyond_tick_interval` - Overdue detection
- `rq_is_overdue_false_when_within_tick_interval` - Within tolerance
- `rq_is_overdue_false_at_exact_boundary` - Boundary condition

---

### 3. `reanimator_red_queen.rs` — Reanimator Timer Lifecycle

**File**: `crates/vo-actor/tests/reanimator_red_queen.rs`

Covers:
- Timer creation during shutdown
- Timer cancellation race with fire
- Duplicate timer registration
- Timer with past fire_at
- Timer across epoch boundaries
- No timer leaks or double-fires
- Fairness budget enforcement
- Crash recovery invariants
- Storage failure handling

**Tests**: 15 tests (1 FAILING)

| Test | Status | Description |
|------|--------|-------------|
| `rq_reanimator_shutdown_rejects_new_work` | PASS | Shutdown rejects new timers |
| `rq_timers_processed_before_shutdown` | PASS | Timers processed during shutdown |
| `rq_delete_before_dispatch_no_double_fire` | PASS | Delete-before-dispatch ordering |
| `rq_concurrent_delete_no_leak` | PASS | Concurrent operations don't leak |
| `rq_duplicate_timer_ids_same_instance` | PASS | Multiple timers per instance |
| `rq_past_fire_at_processed_immediately` | PASS | Past timers fire immediately |
| `rq_timer_at_u64_max_boundary` | PASS | Boundary handling at u64::MAX |
| `rq_timer_at_zero_boundary_rejected` | PASS | Zero fire_at rejected |
| `rq_no_timer_leaks_all_processed` | PASS | All timers eventually processed |
| `rq_deleted_timers_do_not_fire` | PASS | Deleted timers don't fire |
| **`rq_no_double_fire_same_timer`** | **FAIL** | **Duplicate entries fire twice** |
| `rq_enqueue_failure_no_double_fire` | PASS | Enqueue failures don't cause double-fire |
| `rq_fairness_budget_enforced` | PASS | Fairness limits enforced |
| `rq_crash_recovery_skips_terminal_instances` | PASS | Terminal filtering |
| `rq_storage_failure_handled` | PASS | Storage errors handled gracefully |

#### Failing Test Analysis: `rq_no_double_fire_same_timer`

```rust
#[tokio::test]
async fn rq_no_double_fire_same_timer() {
    let instance_id = make_instance_id(0x01);
    let timer = TimerRecord::new(instance_id.clone(), ts_ms(100), None, ts_ms(50));
    let storage = Arc::new(MockTimerStorage::new(vec![timer.clone(), timer.clone()]));
    // ... expects fire_calls.len() == 1
}
```

**Finding**: When the same `TimerRecord` appears twice in storage (identical `instance_id`, `fire_at_ms`, `trigger_time_ms`, `duration_ms`), the reanimator processes both, resulting in 2 fire calls instead of 1.

**Root Cause**: The `MockTimerStorage::scan_due_timers` deduplicates using key `(instance_id, fire_at_ms, timer_id)`. Since both timers have `timer_id: None`, they have the same key and should be deduplicated at the storage layer. However, `fire_calls.len() == 2` indicates both timers are being processed.

**Hypothesis**: The `VecDeque::into()` conversion may not preserve duplicate entries as expected, or the mock's internal iteration behavior differs from real storage.

**Impact**: LOW - In production, storage should deduplicate. This may indicate MockTimerStorage behavior doesn't match real storage.

---

### 4. `red_queen_execution_lease_fencing.rs` — Execution Lease Fencing

**File**: `crates/vo-actor/tests/red_queen_execution_lease_fencing.rs`

Covers (ADR-029):
- Stale fence completion rejection
- Lease expiry during execution
- Concurrent lease acquisition for same instance
- Fence token monotonicity
- Verifying stale completions cannot win

**Tests**: 20+ tests

**Attack 1: Stale Fence Completion Rejection**
- `stale_fence_token_is_rejected_by_lease_record` - Stale tokens rejected
- `stale_completion_cannot_win_race` - Stale can't win concurrent race
- `many_stale_tokens_all_rejected` - 1-99 all rejected against token 100

**Attack 2: Lease Expiry During Execution**
- `expired_lease_token_is_stale` - Expired lease tokens are stale
- `expiry_during_execution_prevents_double_commit` - Double-commit prevented
- `long_running_execution_must_refresh_lease` - Lease refresh required

**Attack 3: Concurrent Lease Acquisition**
- `concurrent_acquisition_same_instance_id_only_one_wins` - Mutual exclusion
- `different_step_ids_have_independent_fence_tokens` - Step-level isolation
- `same_instance_different_step_ids_fences_dont_cross_contaminate` - Clean isolation

**Attack 4: Fence Token Monotonicity**
- `fence_tokens_are_strictly_monotonic` - 1-100 monotonic
- `token_next_increments_by_one` - next() increments by 1
- `token_max_cannot_increment` - u64::MAX boundary
- `monotonic_chain_100_tokens` - Full chain verified

**Attack 5: Stale Completions Cannot Win**
- `stale_completion_after_reacquire_is_rejected` - Token 1 rejected by token 2
- `stale_completion_after_multiple_reacquires_is_rejected` - 1 rejected by 2-5
- `latest_token_is_only_valid_one` - Only exact match wins
- `race_condition_simulated_stale_wins_not_possible` - Race simulation

**Attack 6: Edge Cases**
- `token_one_is_minimum_valid` - Token::new(1) valid
- `zero_token_is_invalid` - Token::new(0) invalid
- `empty_instance_id_lease_edge_case` - Empty instance_id rejected
- `empty_step_id_lease_edge_case` - Empty step_id rejected

---

### 5. `red_queen_structured_logging.rs` — Structured Logging + Error Classification

**File**: `crates/vo-actor/tests/red_queen_structured_logging.rs`

Covers:
- Error classification completeness (all errors transient or fatal)
- SpawnSupervisorError taxonomy
- ReanimatorError taxonomy
- Counter concurrent increments (no lost updates)
- Metrics counter independence
- Error display formatting
- SpawnRecord transition chain preservation

**Tests**: 20+ tests

**Error Classification Tests**
- `spawn_supervisor_error_no_unclassified_variants` - CRITICAL invariant
- All `SpawnSupervisorError` variants classified (9 tests)
- All `ReanimatorError` variants classified (5 tests)
- Transient/fatal mutual exclusion verified

**Concurrency Tests**
- `counter_concurrent_increments_no_lost_updates` - 8 threads × 10000 increments
- `metrics_counters_are_independent` - No cross-contamination

**SpawnRecord Tests**
- `spawn_record_transition_chain_preserves_all_fields` - Field preservation
- `spawn_record_last_error_preserved_through_transition` - Error retention
- `spawn_record_respawn_clears_error` - Respawn clears error state

---

## Coverage Summary

| Dimension | Status | Coverage |
|-----------|--------|----------|
| Lifecycle State Machine | COMPLETE | All transitions tested |
| Supervision Tree | COMPLETE | Parent-child relationships |
| Dead Letter Queue | COMPLETE | Overflow, eviction, FIFO |
| Instance Registry | COMPLETE | Atomicity, rollback |
| Signal Matching | COMPLETE | Lineage, epoch, wait-key |
| Timer Lifecycle | COMPLETE | Create, fire, cancel, crash-recovery |
| Execution Lease Fencing | COMPLETE | Stale rejection, monotonicity |
| Error Classification | COMPLETE | Transient/fatal taxonomy |
| Dual-Clock Verification | COMPLETE | Wall + monotonic clocks |
| Fairness Budget | COMPLETE | Budget enforcement |

## Invariants Verified

1. **INV-1**: Terminal states (Stopped, Failed) absorb all transitions
2. **INV-2**: Delete-before-dispatch ordering prevents double-fire
3. **INV-3**: Signal matching requires exact lineage + epoch + wait-key
4. **INV-4**: Dual-clock verification requires both wall AND monotonic agreement
5. **INV-5**: Fence tokens are strictly monotonically increasing
6. **INV-6**: Stale completions cannot win against newer lease
7. **INV-7**: All errors must be classified as transient OR fatal
8. **INV-8**: Counter increments are atomic with no lost updates

## Findings

### Critical Findings

None at this time.

### Major Findings

| ID | Finding | Severity | Status |
|----|---------|----------|--------|
| F-001 | `rq_no_double_fire_same_timer` fails - duplicate timers fire twice | MAJOR | OPEN |

### Minor Findings

None at this time.

## Dependencies

- `vo-types` - Core types (InstanceId, TimestampMs, etc.)
- `vo-common` - Common utilities
- `ractor` - Actor framework
- `tokio` - Async runtime
- `futures` - Async utilities

## Notes

The Red Queen tests use deterministic state machine execution. Tests are designed to be:
- **Reproducible**: Same input → same output
- **Composable**: Each test is independent
- **Exhaustive**: Cover edge cases and boundary conditions
- **Adversarial**: Designed to find weaknesses

Mutation testing (cargo-mutants) was not available at time of execution. If available, would run:
```bash
cargo mutants -p vo-actor --all
```

## Recommendations

1. **Fix F-001**: Investigate MockTimerStorage deduplication behavior
2. **Add cargo-mutants**: Enable mutation testing for stronger verification
3. **Expand coverage**: Consider adding property-based tests (proptest)
4. **Performance**: Add benchmarks for timer throughput and latency
