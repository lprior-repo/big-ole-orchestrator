# Martin Fowler Test Plan: Per-Activity Timeout Support

## Happy Path Tests
- `test_activity_completes_within_timeout_when_no_timeout_set`
  Given: ActivityTask with `timeout: None`
  When: Activity handler completes in 100ms
  Then: Returns Ok(result), ActivityCompleted is appended with duration_ms=100

- `test_activity_completes_before_timeout`
  Given: ActivityTask with `timeout: Some(Duration::from_secs(5))`
  When: Activity handler completes in 100ms
  Then: Returns Ok(result), no timeout error

- `test_activity_with_explicit_infinite_timeout`
  Given: ActivityTask with `timeout: None`
  When: Activity runs for 10 seconds
  Then: Returns Ok(result), no timeout error

## Error Path Tests
- `test_activity_exceeds_timeout_returns_timeout_error`
  Given: ActivityTask with `timeout: Some(Duration::from_millis(50))`
  When: Activity handler runs for 200ms (simulated slow operation)
  Then: fail_activity is called with ActivityError::TimeoutElapsed

- `test_timeout_error_appends_activity_failed_event`
  Given: ActivityTask with `timeout: Some(Duration::from_millis(10))`
  When: Activity handler does not complete within timeout
  Then: ActivityFailed event is appended with error = "Activity timeout elapsed"

- `test_no_handler_returns_handler_not_found_error`
  Given: ActivityTask with `activity_type: "unknown_type"`
  When: process_task is called
  Then: Warning logged, task is acked (not nak'd), no timeout involved

## Edge Case Tests
- `test_timeout_of_exactly_1_millisecond`
  Given: ActivityTask with `timeout: Some(Duration::from_millis(1))`
  When: Activity handler completes in 1ms exactly
  Then: Returns Ok(result) (boundary case)

- `test_timeout_with_zero_duration_is_rejected_at_construction`
  Given: Attempt to construct ActivityTask with `timeout: Some(Duration::ZERO)`
  When: Validation is performed
  Then: Returns Err (or(Duration::from_millis(1)))

- `test_activity_task_clone_does_not_share_timeout_mutations`
  Given: Original ActivityTask with timeout
  When: Task is cloned for handler
  Then: Clone has independent timeout value

## Contract Verification Tests
- `test_precondition_timeout_none_means_no_limit`
  Given: ActivityTask with `timeout: None`
  When: Processed by worker
  Then: tokio::time::timeout is called with None variant, no actual timeout is set

- `test_precondition_timeout_some_requires_positive_duration`
  Given: ActivityTask with `timeout: Some(Duration::from_millis(1))`
  When: Validated before processing
  Then: Validation passes

- `test_postcondition_timeout_elapsed_returns_correct_error_variant`
  Given: ActivityTask with `timeout: Some(Duration::from_millis(1))`
  When: Handler runs longer than timeout
  Then: Returns `Err(ActivityError::TimeoutElapsed)` (not panic)

- `test_invariant_task_acked_after_processing`
  Given: A valid ActivityTask
  When: process_task completes (success or failure)
  Then: ack() is called exactly once

## Contract Violation Tests
- `test_p1_violation_zero_timeout_rejected`
  Given: ActivityTask with `timeout: Some(Duration::ZERO)`
  When: Created via constructor or deserializer
  Then: Returns Err or(Duration::from_millis(1))

- `test_p2_violation_submillisecond_timeout_rejected`
  Given: ActivityTask with `timeout: Some(Duration::from_nanos(1))`
  When: Created via constructor or deserializer
  Then: Returns Err

- `test_q1_violation_slow_activity_triggers_timeout`
  Given: ActivityTask with `timeout: Some(Duration::from_millis(100))`
  When: Handler simulates 500ms work
  Then: fail_activity called with ActivityError::TimeoutElapsed

## Given-When-Then Scenarios

### Scenario 1: Fast activity with generous timeout
Given: ActivityTask for "send_email" with `timeout: Some(Duration::from_secs(30))`
When: Handler sends email in 50ms
Then:
- Handler returns Ok
- ActivityCompleted is appended
- duration_ms = 50
- Task is acked

### Scenario 2: Slow activity exceeds timeout
Given: ActivityTask for "process_payment" with `timeout: Some(Duration::from_secs(1))`
When: Handler takes 5 seconds due to upstream latency
Then:
- Handler is cancelled via tokio::time::timeout
- fail_activity is called with ActivityError::TimeoutElapsed
- Task is acked (not nak'd - timeout is permanent failure)

### Scenario 3: Activity with no timeout configured
Given: ActivityTask with `timeout: None`
When: Handler runs for any duration
Then:
- No timeout enforcement
- Handler completes or panics naturally
- Normal success/failure path followed

### Scenario 4: Activity timeout on boundary
Given: ActivityTask with `timeout: Some(Duration::from_millis(100))`
When: Handler completes in exactly 100ms
Then:
- Returns Ok (not considered timeout since completion == deadline)
