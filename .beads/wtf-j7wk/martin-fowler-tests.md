# Martin Fowler Test Plan

bead_id: vo-j7wk
bead_title: "vo-frontend: Simulate Mode Procedural — step through ctx calls, show checkpoint map"
phase: test-plan
updated_at: 2026-03-21T17:30:00Z

## Happy Path Tests

- `test_initial_state_has_empty_checkpoint_map`
  Given: SimProceduralState is newly created
  When: state is inspected
  Then: checkpoint_map is empty HashMap

- `test_initial_state_has_zero_current_op`
  Given: SimProceduralState is newly created
  When: current_op is read
  Then: current_op equals 0

- `test_initial_state_has_empty_event_log`
  Given: SimProceduralState is newly created
  When: event_log is inspected
  Then: event_log is empty Vec

- `test_provide_result_appends_activity_completed_to_event_log`
  Given: SimProceduralState with empty event_log
  When: provide_result("success", "act-001") is called
  Then: event_log has exactly one WorkflowEvent::ActivityCompleted

- `test_provide_result_adds_to_checkpoint_map`
  Given: SimProceduralState with empty checkpoint_map
  When: provide_result("result-value", "act-001") is called
  Then: checkpoint_map contains key "act-001" with value "result-value"

- `test_provide_result_increments_current_op`
  Given: SimProceduralState with current_op == 0
  When: provide_result("ok", "act-001") is called
  Then: current_op becomes 1

- `test_multiple_provide_result_calls_accumulate`
  Given: SimProceduralState after 2 provide_result calls
  When: state is inspected
  Then: checkpoint_map has 2 entries and current_op == 2

- `test_can_advance_returns_true_when_ops_remaining`
  Given: SimProceduralState with current_op == 1 and total_ops == 3
  When: can_advance(3) is called
  Then: returns true

- `test_can_advance_returns_false_at_end`
  Given: SimProceduralState with current_op == 3 and total_ops == 3
  When: can_advance(3) is called
  Then: returns false

## Error Path Tests

- `test_provide_result_returns_empty_result_error_when_result_is_empty`
  Given: SimProceduralState with empty result string
  When: provide_result("", "act-001") is called
  Then: returns Err(Error::EmptyResult)

- `test_provide_result_returns_already_completed_when_at_end`
  Given: SimProceduralState with current_op == 5 and total_ops == 5
  When: provide_result("ok", "act-1") is called
  Then: returns Err(Error::AlreadyCompleted)

- `test_provide_result_returns_no_ops_available_when_ops_list_empty`
  Given: SimProceduralState and total_ops == 0
  When: can_advance(0) is checked or provide_result is called
  Then: returns Err(Error::NoOpsAvailable)

## Edge Case Tests

- `test_long_activity_id_in_checkpoint`
  Given: SimProceduralState
  When: provide_result("x", "very-long-activity-id-with-many-chars") is called
  Then: checkpoint_map key matches exactly, no truncation

- `test_empty_activity_id_edge_case`
  Given: SimProceduralState
  When: provide_result("result", "") is called
  Then: behavior depends on NonEmptyString enforcement (prefer compile-time reject)

- `test_current_op_never_negative`
  Given: SimProceduralState
  When: any operation is performed
  Then: current_op >= 0 always holds (u32 is unsigned)

- `test_max_u32_boundary`
  Given: SimProceduralState with current_op approaching u32::MAX
  When: provide_result is called many times
  Then: would overflow (not realistic for UI, document as out-of-scope)

## Contract Verification Tests

- `test_invariant_current_op_never_exceeds_ops_length`
  Given: SimProceduralState and total_ops
  When: any sequence of operations is performed
  Then: invariant I1 holds: current_op <= total_ops

- `test_invariant_checkpoint_map_len_matches_current_op`
  Given: SimProceduralState after N provide_result calls
  Then: invariant I2 holds: checkpoint_map.len() == current_op

- `test_invariant_event_log_len_matches_current_op`
  Given: SimProceduralState after N provide_result calls
  Then: invariant I3 holds: event_log.len() == current_op

- `test_checkpoint_map_is_append_only`
  Given: SimProceduralState with entries in checkpoint_map
  When: provide_result is called again
  Then: previous entries are preserved, not overwritten

- `test_event_log_is_append_only`
  Given: SimProceduralState with N events in event_log
  When: provide_result is called
  Then: previous N events remain, new event appended at position N

## End-to-End Scenario

### Scenario: Step through 3 ctx operations

Given: A workflow with 3 CtxActivity nodes (act-001, act-002, act-003)

When:
1. User sees current_op=0, pending=act-001
2. User types "result-1" and clicks Complete
3. User sees current_op=1, pending=act-002, checkpoint_map shows act-001→result-1
4. User types "result-2" and clicks Complete
5. User sees current_op=2, pending=act-003, checkpoint_map shows both entries
6. User types "result-3" and clicks Complete
7. User sees current_op=3, no pending ops, all 3 entries in checkpoint_map
8. Complete button is disabled/grayed

Then: Checkpoint map panel shows exactly:
```
act-001 → result-1
act-002 → result-2
act-003 → result-3
```

Event log shows 3 ActivityCompleted events with matching activity_ids.
