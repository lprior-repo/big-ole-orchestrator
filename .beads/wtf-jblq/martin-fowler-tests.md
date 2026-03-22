# Martin Fowler Test Plan: wtf-jblq — Frontend SSE Watch Client Parity

```json
{"bead_id":"wtf-jblq","phase":"test-planning","updated_at":"2026-03-22T00:00:00Z"}
```

## Happy Path Tests

### Scenario: `test_watch_namespace_yields_instance_data_when_sse_is_valid`
Given: [DEFERRED TO INTEGRATION] Real SSE server returning valid instance events for namespace `payments`
When: `watch_namespace("http://localhost:8080", "payments")` is called and the stream is consumed
Then: The stream yields `Ok(InstanceView)` with correct `instance_id`, `workflow_type`, `status`, `last_event_seq`

### Scenario: `test_watch_namespace_parses_plain_json_sse_payload`
Given: [DEFERRED TO INTEGRATION] Real SSE server returning plain JSON `data:` frame (no key prefix)
When: The stream is consumed
Then: The stream yields `Ok(InstanceView)` with fields correctly parsed from the JSON

### Scenario: `test_use_instance_watch_signal_reflects_most_recent_state_per_instance`
Given: [DEFERRED TO INTEGRATION] Real SSE server returning multiple instance updates for different instance IDs
When: `use_instance_watch("payments")` is called and the signal is read after multiple events
Then: The `ReadSignal<Vec<InstanceView>>` value is sorted by `instance_id` ascending

### Scenario: `test_upsert_instance_handles_new_instance`
Given: An empty `Vec<InstanceView>` and a new `InstanceView` with `instance_id = "abc"`
When: `upsert_instance([], InstanceView { instance_id: "abc", ... })` is called
Then: The returned Vec contains exactly the new instance (sorted insertion)

### Scenario: `test_upsert_instance_handles_existing_instance`
Given: A `Vec<InstanceView>` containing `InstanceView { instance_id: "abc", last_event_seq: 5 }` and an update with `instance_id = "abc", last_event_seq: 10`
When: `upsert_instance` is called with the update
Then: The returned Vec contains exactly one entry for `"abc"` with `last_event_seq: 10` (replaced, not duplicated)

### Scenario: `test_backoff_doubles_after_each_failure`
Given: `BackoffPolicy::default()`
When: `delay_for_attempt` is called for attempts 0, 1, 2
Then: Attempt 0 returns 250ms, attempt 1 returns 500ms, attempt 2 returns 1000ms (exponential, capped at 3s)

### Scenario: `test_backoff_respects_max_delay_cap`
Given: `BackoffPolicy::new(Duration::from_millis(100), Duration::from_millis(400))`
When: `delay_for_attempt(10)` is called
Then: Returns `Duration::from_millis(400)` (max cap, not 102.4s)

## Error Path Tests

### Scenario: `test_empty_base_url_causes_watch_to_fail`
Given: An empty string as `base_url`
When: `watch_namespace("", "payments")` is called
Then: Returns a `Result` that yields `Err(WatchError::Request("builder error: empty domain"))`

### Scenario: `test_connection_refused_produces_request_error`
Given: [DEFERRED TO INTEGRATION] No server listening on the target port
When: The SSE stream is consumed
Then: Yields `Err(WatchError::Request(...))` with a connection-refused message

### Scenario: `test_server_error_triggers_backoff_recovery`
Given: [DEFERRED TO INTEGRATION] Server returns HTTP 503 on first two connections, then valid SSE on third
When: `watch_namespace_with_policy` with `BackoffPolicy::new(10ms, 20ms)` is consumed for three items
Then: First two items are `Err(WatchError::Request("503"))`, third item is `Ok(InstanceView)`

### Scenario: `test_missing_data_line_produces_invalid_payload_error`
Given: An SSE body with no `data:` prefix line
When: `parse_first_sse_data_payload("event: put\n\n")` is called
Then: Returns `Err(WatchError::InvalidPayload("missing data: line in SSE stream"))`

### Scenario: `test_unparseable_payload_produces_invalid_payload_error`
Given: A payload that is neither plain JSON nor key-prefixed format
When: `parse_first_instance_payload("not json at all")` is called
Then: Returns `Err(WatchError::InvalidPayload(...))` from serde_json

### Scenario: `test_malformed_json_in_payload_produces_invalid_payload_error`
Given: A payload with invalid JSON after the prefix separator
When: `parse_first_instance_payload("namespace/id:not json")` is called
Then: Returns `Err(WatchError::InvalidPayload(...))` because the JSON parse of "not json" fails

## Edge Case Tests

### Scenario: `test_multiline_sse_data_frames_are_concatenated`
Given: [DEFERRED TO INTEGRATION] Real SSE body with multiline `data:` frames
When: `parse_first_sse_data_payload("event: put\ndata: {\"workflow_type\":\"checkout\"\ndata: ,\"phase\":\"live\",\"events_applied\":9}\n\n")` is called
Then: Returns the concatenated payload with all fields

### Scenario: `test_only_first_event_is_parsed`
Given: An SSE body with two events separated by `\n\n`
When: `parse_first_sse_data_payload("event: put\ndata: first\n\nevent: put\ndata: second\n\n")` is called
Then: Returns `"first"` (first event's data), not `"second"`

### Scenario: `test_instance_id_extracted_from_key_prefix_path`
Given: A key-prefixed payload `payments/instances/01ABC:{"workflow_type":"checkout","phase":"live"}`
When: `parse_first_instance_payload(payload)` is called
Then: `instance_id` is `"01ABC"` (last path segment, not the full path)

### Scenario: `test_null_current_state_becomes_none`
Given: A plain JSON payload with `current_state: null`
When: `parse_first_instance_payload` is called
Then: Returns `InstanceView` with `current_state: None`

### Scenario: `test_missing_optional_fields_get_defaults`
Given: A key-prefixed payload with only `workflow_type` set, missing `phase`, `current_state`, etc.
When: `parse_first_instance_payload("ns/id:{\"workflow_type\":\"wf\"}")` is called
Then: `status` defaults to `"unknown"`, `current_state` defaults to `None`, `last_event_seq` defaults to `0`, `updated_at` defaults to `""`

### Scenario: `test_initial_backoff_delay_is_not_multiplied`
Given: `BackoffPolicy::new(Duration::from_millis(100), Duration::from_secs(10))`
When: `delay_for_attempt(0)` is called
Then: Returns exactly `Duration::from_millis(100)` (no multiplication, just initial)

### Scenario: `test_backoff_grows_exponentially_between_attempts`
Given: `BackoffPolicy::new(Duration::from_millis(100), Duration::from_secs(10))`
When: `delay_for_attempt(1)`, `delay_for_attempt(2)`, `delay_for_attempt(3)` are called
Then: Returns 200ms, 400ms, 800ms respectively

### Scenario: `test_backoff_bounded_shift_prevents_overflow`
Given: `BackoffPolicy::new(Duration::from_millis(1), Duration::from_secs(1000))`
When: `delay_for_attempt(100)` is called
Then: Delay calculation uses bounded shift to prevent overflow

### Scenario: `test_upsert_instance_maintains_sort_order_after_multiple_upserts`
Given: A `Vec<InstanceView>` with `["aaa", "ccc"]` and upsert of `"bbb"` instance
When: `upsert_instance(["aaa", "ccc"], InstanceView { instance_id: "bbb" })` is called
Then: Returns `["aaa", "bbb", "ccc"]`

### Scenario: `test_empty_instance_id_is_accepted`
Given: A key-prefixed payload with empty instance_id `":{\"workflow_type\":\"wf\"}"`
When: `parse_first_instance_payload` is called
Then: Returns `InstanceView` with `instance_id: ""`

### Scenario: `test_non_sequential_last_event_seq_is_accepted`
Given: SSE events arriving with `last_event_seq` values that are not sequential (e.g., 5, 7, 6)
When: `upsert_instance` processes these events
Then: Each event's `last_event_seq` is stored correctly, no events are dropped

### Scenario: `test_four_failure_sequence_triggers_correct_backoff_sequence`
Given: [DEFERRED TO INTEGRATION] Server fails 4 times before succeeding
When: `watch_namespace_with_policy` is consumed for 5 items
Then: Each failure yields an error, and backoff delays increase: initial, 2x, 4x, 8x, then success

## Contract Verification Tests

### Scenario: `test_watch_resets_backoff_on_successful_connection`
Given: [DEFERRED TO INTEGRATION] Watch has been retrying with accumulated backoff
When: Server becomes available and returns valid SSE
Then: The next backoff delay resets to initial value

### Scenario: `test_watch_increments_backoff_on_each_failure`
Given: [DEFERRED TO INTEGRATION] Watch encounters consecutive failures
When: Each failure is processed
Then: The backoff delay approximately doubles for each attempt

### Scenario: `test_watch_backoff_saturates_at_reasonable_maximum`
Given: [DEFERRED TO INTEGRATION] Watch encounters many consecutive failures
When: Failures continue beyond a reasonable retry count
Then: The backoff delay does not grow unbounded

### Scenario: `test_use_instance_watch_signal_never_contains_duplicate_instance_ids`
Given: Multiple SSE events updating the same `instance_id`
When: `use_instance_watch` processes these events
Then: The signal's Vec always contains exactly one entry per unique `instance_id`

### Scenario: `test_parse_requires_double_newline_event_delimiter`
Given: SSE body `"data: first\nevent: put\ndata: second\n\n"` (single newline between events)
When: `parse_first_sse_data_payload` is called
Then: Returns `"first\nevent: put\ndata: second"` as a single concatenated data payload (no double newline found, entire body treated as one event)

## Contract Violation Tests

### Scenario: `test_empty_base_url_violation_returns_request_error`
Given: `base_url = ""`
When: `watch_namespace("", "payments")` is called
Then: Returns `Err(WatchError::Request(...))` — NOT a panic, NOT an unwrap failure

### Scenario: `test_empty_namespace_violation_returns_request_error`
Given: `namespace = ""`
When: `watch_namespace("http://localhost:8080", "")` is called
Then: Returns `Err(WatchError::Request(...))` — NOT a panic, NOT an unwrap failure

### Scenario: `test_backoff_policy_invalid_initial_max_violation`
Given: `BackoffPolicy::new(Duration::from_secs(5), Duration::from_secs(1))`
When: `delay_for_attempt(0)` is called
Then: Returns `1s` (clamped by max), but the policy is logically invalid (debug_assert would fire in debug builds)

### Scenario: `test_missing_data_line_violation_returns_invalid_payload_error`
Given: SSE body with no `data:` line
When: `parse_first_sse_data_payload("event: put\n\n")` is called
Then: Returns `Err(WatchError::InvalidPayload("missing data: line in SSE stream"))`

### Scenario: `test_invalid_json_violation_returns_invalid_payload_error`
Given: Payload `"not parseable json"`
When: `parse_first_instance_payload("not parseable json")` is called
Then: Returns `Err(WatchError::InvalidPayload(...))` from serde

### Scenario: `test_watch_namespace_stream_never_terminates`
Given: A `watch_namespace` stream is created and consumed indefinitely
When: The SSE server stays up and returns valid events
Then: The stream never returns `None` (it is an infinite stream via `unfold`)

## End-to-End Scenario

### Scenario: `test_full_reconnect_cycle_with_backoff_recovery`
Given: [DEFERRED TO INTEGRATION] Real server fails with 503 twice then succeeds, using `BackoffPolicy::new(10ms, 20ms)`
When: The caller consumes the first 3 items from `watch_namespace_with_policy`
Then:
- Item 1: `Err(WatchError::Request(...))` (503)
- Item 2: `Err(WatchError::Request(...))` (503)
- Elapsed time between item 1 and 2: ≥ 10ms (initial backoff)
- Elapsed time between item 2 and 3: ≥ 20ms (next backoff, capped at max)
- Item 3: `Ok(InstanceView)` with correct parsed fields
- `instance_id` is correctly extracted from the event path

## Deferred / Advanced Testing Notes

### Property-Based Testing — DEFERRED
- No `quickcheck` or `proptest` style tests for SSE parsing invariants are planned at this time
- May be added in future iterations if parsing logic becomes more complex

### Fuzzing — DEFERRED
- No `cargo-fuzz` or similar for SSE payload parsing is planned at this time

### Mutation Testing — DEFERRED
- No mutation coverage analysis is planned at this time

## Integration Test Names (Real Infrastructure Required)

These tests require a real HTTP/SSE server and are marked DEFERRED TO INTEGRATION:

1. `test_watch_namespace_yields_instance_data_when_sse_is_valid` — real SSE server
2. `test_watch_namespace_parses_plain_json_sse_payload` — real SSE server
3. `test_use_instance_watch_signal_reflects_most_recent_state_per_instance` — real SSE server
4. `test_connection_refused_produces_request_error` — real network error
5. `test_server_error_triggers_backoff_recovery` — real server with controlled failure
6. `test_multiline_sse_data_frames_are_concatenated` — real SSE multiline handling
7. `test_four_failure_sequence_triggers_correct_backoff_sequence` — real server with 4 failures
8. `test_watch_resets_backoff_on_successful_connection` — real server recovery
9. `test_watch_increments_backoff_on_each_failure` — real backoff timing
10. `test_watch_backoff_saturates_at_reasonable_maximum` — real backoff saturation
11. `test_full_reconnect_cycle_with_backoff_recovery` — real end-to-end recovery

(End of file - total 238 lines)
