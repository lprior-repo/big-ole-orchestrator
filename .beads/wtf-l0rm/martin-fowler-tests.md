# Martin Fowler Test Plan: Timer Loop KV Watch Optimization

## Happy Path Tests

- `test_watch_loop_fires_timer_immediately_when_due_on_put`
  Given: A timer record with fire_at in the past is created in wtf-timers
  When: KV watch delivers a Put operation for that timer
  Then: Timer is fired and TimerFired event is appended to JetStream

- `test_watch_loop_does_not_fire_future_timer`
  Given: A timer record with fire_at 1 hour in the future is created
  When: KV watch delivers a Put operation
  Then: Timer is NOT fired; record remains in KV

- `test_watch_loop_continues_after_shutdown_signal`
  Given: Timer loop is running
  When: Shutdown signal is sent on shutdown_rx
  Then: Loop exits cleanly after processing current entry

## Error Path Tests

- `test_fire_timer_jetstream_failure_returns_error`
  Given: JetStream is unavailable
  When: fire_timer is called
  Then: Returns `Err(WtfError::NatsPublish)`

- `test_watch_stream_closed_returns_error`
  Given: Watch stream closes unexpectedly
  When: watch.next() returns None
  Then: Loop exits and returns error

- `test_deserialization_failure_skips_corrupt_entry`
  Given: A timer entry with invalid msgpack is in KV
  When: Watch delivers that entry
  Then: Entry is skipped with warning logged; loop continues

## Edge Case Tests

- `test_multiple_timers_fired_in_single_batch`
  Given: Multiple due timers exist when watch connects
  When: Watch delivers initial batch or rapid sequence of Put operations
  Then: All due timers are fired

- `test_timer_fired_twice_idempotent_via_applied_seq`
  Given: A TimerFired event was already appended for a timer
  When: fire_timer is called again (e.g., race condition)
  Then: Second TimerFired is appended; instance actor handles via applied_seq check

- `test_delete_failure_after_fire_logs_warning_and_continues`
  Given: KV delete fails after TimerFired was appended
  When: fire_timer completes
  Then: Warning logged; seq returned; loop continues

## Contract Verification Tests

- `test_precondition_p1_watch_fails_when_nats_unavailable`
  Given: NATS KV is inaccessible
  When: watch_all() is called
  Then: Returns Err and loop does not start

- `test_postcondition_q1_no_duplicate_fires_for_same_timer`
  Given: Timer with fire_at in past, deleted between watch delivery and fire_timer call
  When: fire_timer is called
  Then: Either succeeds or returns error; does not panic

- `test_invariant_i1_timer_never_fired_before_fire_at`
  Given: A timer with fire_at in the future
  When: KV watch delivers Put operation
  Then: is_due() returns false; timer not fired

## Given-When-Then Scenarios

### Scenario 1: Timer fires immediately when created already-due
Given: Timer record exists with fire_at = now - 5s
And: Watch loop has started
When: Watch delivers Put operation for that timer key
Then:
- fire_timer is called
- TimerFired event is appended to JetStream
- Timer record is deleted from KV
- Sequence number is returned

### Scenario 2: Timer waits until due time
Given: Timer record with fire_at = now + 1 hour
When: Watch delivers Put operation
Then:
- is_due(now) returns false
- Timer is NOT fired
- Record remains in KV for future watch processing

### Scenario 3: Initial sync processes all existing due timers
Given: Multiple timer records exist in KV with past fire_at times
When: Watch loop starts and receives initial snapshot
Then:
- All due timers are identified and fired
- No polling (keys()) is performed during steady-state operation

### Scenario 4: Watch handles rapid create/delete cycles
Given: A timer is created and immediately deleted before being processed
When: Watch delivers Put then Delete for same key
Then:
- Put is processed first (check if due)
- Delete is processed (timer removed, no double-fire)
- Loop continues without error
