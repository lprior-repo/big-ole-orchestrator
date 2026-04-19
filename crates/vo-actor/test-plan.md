# vo-actor Lifecycle Tests - Test Plan

## Overview

vo-actor has 538 tests, but spawn/supervisor edge cases need more coverage. This plan
covers four critical areas: actor panic recovery, supervisor restart limits, mailbox
overflow, and actor lifecycle transitions.

## Test Inventory

### 1. Actor Panic Recovery Tests (Reanimator Loop)

**Coverage Gap**: The reanimator handles crash recovery but timer edge cases need testing.

| Test | Description | Expected Outcome |
|------|-------------|------------------|
| `crash_recovery_skips_terminal_instances` | Pending timer for instance in Stopped/Failed state | Timer replay skipped, pending timer cleaned |
| `crash_recovery_cleans_stale_pending_timers` | Pending timer older than 60s threshold | Stale timer cleaned up |
| `crash_recovery_replays_active_pending_timers` | Pending timer for active instance | Resume work enqueued |
| `crash_recovery_handles_work_queue_failure` | WorkQueue.enqueue_resume fails | Error logged, timer not marked complete |
| `process_cycle_skips_invalid_timer_records` | Timer with invalid instance_id | Skipped, not processed |
| `process_cycle_respects_fairness_budget` | Many timers for same instance | Limited per-cycle processing |
| `process_cycle_delete_before_dispatch` | Verify timer deleted BEFORE enqueue | No double-fire possible |
| `process_cycle_handles_storage_delete_failure` | Timer delete fails after scan | Error logged, not marked complete |

### 2. Supervisor Restart Limit Tests

**Coverage Gap**: The supervisor has max_spawn_attempts but backoff/respawn not fully tested.

| Test | Description | Expected Outcome |
|------|-------------|------------------|
| `max_attempts_exceeded_transitions_to_failed` | Record with spawn_attempts > max | Phase becomes Failed, no respawn |
| `max_attempts_at_boundary` | spawn_attempts == max - 1 then fails | One more respawn allowed |
| `respawn_increments_attempt_counter` | Failed record respawned | spawn_attempts incremented |
| `backoff_delay_calculation_attempt_1` | First respawn | initial_backoff applied |
| `backoff_delay_calculation_exponential` | Multiple respawns | Delay grows exponentially |
| `backoff_delay_saturates_at_u64_max` | Many rapid respawns | No overflow, saturates |
| `should_respawn_true_below_limit` | Failed phase, attempts < max | true |
| `should_respawn_false_at_limit` | Failed phase, attempts == max | false |
| `should_respawn_false_non_failed_phase` | Running phase, any attempts | false |
| `is_zombie_state_true_high_attempts` | Failed phase, attempts > 3 | true |
| `is_zombie_state_false_low_attempts` | Failed phase, attempts <= 3 | false |

### 3. Mailbox Overflow Tests

**Coverage Gap**: SpawnSupervisorError::MailboxFull defined but not exercised in tests.

| Test | Description | Expected Outcome |
|------|-------------|------------------|
| `mailbox_full_error_is_transient` | MailboxFull error | is_transient() == true |
| `process_cycle_handles_mailbox_full` | WorkQueue returns MailboxFull | Transient error, logged, continues |
| `spawn_record_last_error_preserved` | Spawn fails with error | last_error field set |
| `process_cycle_multiple_failures` | Multiple spawns fail in one cycle | All failures tracked in metrics |

### 4. Actor Lifecycle Transition Tests

**Coverage Gap**: lifecycle.rs has state machine but integration with ParentChildRegistry needs tests.

| Test | Description | Expected Outcome |
|------|-------------|------------------|
| `compute_next_state_pending_start` | Pending + Start | Running |
| `compute_next_state_pending_fail` | Pending + Fail | Failed |
| `compute_next_state_running_stop` | Running + Stop | Stopping |
| `compute_next_state_running_fail` | Running + Fail | Failed |
| `compute_next_state_stopping_all_children_stopped` | Stopping + AllChildrenStopped | Stopped |
| `compute_next_state_invalid_from_stopped` | Stopped + Start | None (invalid) |
| `compute_next_state_invalid_from_failed` | Failed + any transition | None (invalid) |
| `compute_next_state_stopping_child_stopped` | Stopping + ChildStopped | Still Stopping |
| `is_valid_transition_valid_cases` | All valid transitions | true |
| `is_valid_transition_invalid_cases` | Invalid transitions | false |
| `actor_lifecycle_state_is_terminal` | Stopped/Failed are terminal | is_terminal() true |
| `actor_lifecycle_state_is_stopping` | Stopping/Stopped are stopping | is_stopping() true |
| `actor_lifecycle_state_can_spawn_child` | Only Pending/Running | can_spawn_child() true |
| `parent_child_registry_add_child` | Add child to registry | Child added, state Pending |
| `parent_child_registry_remove_child` | Remove child | Child removed |
| `parent_child_registry_update_state` | Update child state | State updated |
| `parent_child_registry_get_children_by_state` | Filter children by state | Correct children returned |
| `parent_child_registry_all_children_terminal` | All children Stopped/Failed | true |
| `parent_child_registry_active_count` | Count non-terminal children | Correct count |
| `shutdown_propagator_default_timeouts` | Default propagator | 30s graceful, 10s force |
| `shutdown_result_success` | All children stopped | Success variant |
| `shutdown_result_children_running` | Some children running | ChildrenRunning with count |
| `shutdown_result_timeout` | Timeout exceeded | Timeout variant |

## Implementation Notes

1. **TDD Red Approach**: Tests should fail first against current implementation, exposing gaps
2. **Mock Strategy**: Use existing MockSpawnStorage, MockProcessManager, MockWorkQueue
3. **Edge Cases**: Focus on boundary conditions (max attempts, overflow, invalid transitions)
4. **Pure Functions**: Test lifecycle.rs compute_next_state and is_valid_transition directly

## Gap Analysis (from existing tests)

- **Gap #1**: `zombies_detected` metric never incremented (is_zombie not called)
- **Gap #2**: WorkQueue methods never called in process_cycle
- **Gap #3**: Backoff delay calculated but discarded (`let _ = backoff_delay`)
- **Gap #4**: Health check for HealthCheck phase uses pid: 0 instead of actual
- **Gap #5**: Respawn scheduling not implemented
- **Gap #6**: Storage failures not tracked in CycleResult.errors accurately

## Files to Modify

1. `crates/vo-actor/tests/spawn_supervisor_integration.rs` - Add supervisor restart limit tests
2. `crates/vo-actor/src/lifecycle.rs` - Add lifecycle transition tests (existing unit tests extend)
3. `crates/vo-actor/src/reanimator/loop_core.rs` - Add reanimator crash recovery tests
4. `crates/vo-actor/tests/` - New test files as needed