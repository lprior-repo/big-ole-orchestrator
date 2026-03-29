# Martin Fowler Test Plan

## Metadata
- **bead_id**: vo-qgum
- **bead_title**: worker: Implement echo and sleep activity handlers
- **phase**: STATE 2 (test plan)
- **updated_at**: 2026-03-23

---

## Unit Tests (in `builtin.rs` `#[cfg(test)] mod tests`)

### Happy Path Tests

- `test_echo_returns_payload_unchanged_for_ascii_bytes`
- `test_echo_returns_payload_unchanged_for_binary_bytes`
- `test_echo_returns_payload_unchanged_for_empty_payload`
- `test_sleep_returns_ok_slept_after_duration_10ms`
- `test_sleep_returns_ok_slept_after_duration_100ms`
- `test_sleep_returns_ok_slept_after_duration_0ms`

### Error Path Tests

- `test_sleep_rejects_non_utf8_payload`
- `test_sleep_rejects_invalid_json_payload`
- `test_sleep_rejects_json_without_ms_field`
- `test_sleep_rejects_ms_field_with_string_value`
- `test_sleep_rejects_ms_field_with_float_value`
- `test_sleep_rejects_ms_field_with_negative_number`
- `test_sleep_rejects_ms_field_with_nested_object`
- `test_sleep_rejects_empty_payload`
- `test_sleep_rejects_json_array_payload`
- `test_sleep_rejects_json_number_payload`
- `test_sleep_rejects_json_null_payload`
- `test_sleep_rejects_json_string_payload`
- `test_sleep_rejects_json_boolean_payload`

### Edge Case Tests

- `test_echo_preserves_large_payload_1mb`
- `test_echo_preserves_null_bytes_in_payload`
- `test_sleep_with_ms_zero_completes_immediately`
- `test_sleep_accepts_payload_with_extra_json_fields` (payload `{"ms":10,"trace_id":"abc","extra":true}` returns `Ok("slept")`)
- `test_sleep_with_ms_u64_max_is_accepted` (uses `tokio::time::pause()`; payload accepted and returns `Ok("slept")` without advancing time)

### Contract Verification Tests

- `test_precondition_register_defaults_requires_mutable_worker` (compile-time; function signature enforces this)
- `test_invariant_echo_handler_never_panics`
- `test_invariant_sleep_handler_never_panics_on_any_bytes_input`

> **D1/D2 Resolution**: `register_defaults` postconditions (adds echo+sleep, preserves existing, idempotent) moved to E2E Integration Tests below. `Worker::new` requires a live NATS `Context` and `handlers` is private — these cannot be unit-tested. Verification is through actual task dispatch in integration tests.

---

## Integration Tests (in `worker_integration_tests.rs`)

### E2E Happy Path Tests

- `test_echo_task_round_trip_through_worker`
- `test_sleep_task_round_trip_through_worker`
- `test_register_defaults_enables_echo_handler`
- `test_register_defaults_preserves_existing_handlers`
- `test_register_defaults_is_idempotent`

### E2E Error Path Tests

- `test_sleep_task_with_invalid_payload_triggers_fail_activity`

### E2E Edge Case Tests

- `test_worker_with_defaults_acks_unknown_activity_type`

---

## Given-When-Then Scenarios

### Scenario 1: Echo returns ASCII payload unchanged

**Given** an `ActivityTask` with `payload = Bytes::from_static(b"hello world")` and `activity_type = "echo"`
**When** `echo_handler(task)` is awaited
**Then** the result is `Ok(Bytes::from_static(b"hello world"))`
**And** the returned `Bytes` are byte-identical to the input (length 11, same content).

### Scenario 2: Echo returns binary payload unchanged

**Given** an `ActivityTask` with `payload = Bytes::from_static(b"\x00\x01\x02\xff\xfe")` and `activity_type = "echo"`
**When** `echo_handler(task)` is awaited
**Then** the result is `Ok(Bytes::from_static(b"\x00\x01\x02\xff\xfe"))`
**And** all 5 bytes match exactly including null bytes and high-bit values.

### Scenario 3: Echo returns empty payload

**Given** an `ActivityTask` with `payload = Bytes::new()` (empty) and `activity_type = "echo"`
**When** `echo_handler(task)` is awaited
**Then** the result is `Ok(Bytes::new())` (empty Bytes).

### Scenario 4: Sleep succeeds with valid JSON payload

**Given** an `ActivityTask` with `payload = Bytes::from_static(br#"{"ms":10}"#)` and `activity_type = "sleep"`
**And** a `tokio::time::pause()` handle is active
**When** `sleep_handler(task)` is awaited
**Then** the result is `Ok(Bytes::from_static(b"\"slept\""))`
**And** advancing the paused clock by 10ms via `time_handle.advance(Duration::from_millis(10)).await` causes the handler to complete
**And** `time_handle.current()` reflects the expected virtual time (deterministic; no wall-clock dependency).

### Scenario 5: Sleep with ms=0 completes immediately

**Given** an `ActivityTask` with `payload = Bytes::from_static(br#"{"ms":0}"#)` and `activity_type = "sleep"`
**And** a `tokio::time::pause()` handle is active
**When** `sleep_handler(task)` is awaited
**Then** the result is `Ok(Bytes::from_static(b"\"slept\""))`
**And** the handler completes without advancing the paused clock (no `advance()` call needed).

### Scenario 6: Sleep rejects non-UTF-8 payload

**Given** an `ActivityTask` with `payload = Bytes::from_static(b"\xff\xfe")` (invalid UTF-8) and `activity_type = "sleep"`
**When** `sleep_handler(task)` is awaited
**Then** the result is `Err(msg)` where `msg.contains("invalid payload")`.

### Scenario 7: Sleep rejects invalid JSON

**Given** an `ActivityTask` with `payload = Bytes::from_static(b"not json")` and `activity_type = "sleep"`
**When** `sleep_handler(task)` is awaited
**Then** the result is `Err(msg)` where `msg.contains("invalid payload")`.

### Scenario 8: Sleep rejects missing ms field

**Given** an `ActivityTask` with `payload = Bytes::from_static(br#"{"other":42}"#)` and `activity_type = "sleep"`
**When** `sleep_handler(task)` is awaited
**Then** the result is `Err(msg)` where `msg.contains("invalid payload")`.

### Scenario 9: Sleep rejects ms field with string value

**Given** an `ActivityTask` with `payload = Bytes::from_static(br#"{"ms":"fast"}"#)` and `activity_type = "sleep"`
**When** `sleep_handler(task)` is awaited
**Then** the result is `Err(msg)` where `msg.contains("invalid payload")`.

### Scenario 10: Sleep rejects empty payload

**Given** an `ActivityTask` with `payload = Bytes::new()` (empty) and `activity_type = "sleep"`
**When** `sleep_handler(task)` is awaited
**Then** the result is `Err(msg)` where `msg.contains("invalid payload")`.

### Scenario 10a: Sleep rejects JSON number payload

**Given** an `ActivityTask` with `payload = Bytes::from_static(b"42")` and `activity_type = "sleep"`
**When** `sleep_handler(task)` is awaited
**Then** the result is `Err(msg)` where `msg.contains("invalid payload")`.
**And** the error is distinct from invalid-JSON errors (payload is valid JSON but not an object).

### Scenario 10b: Sleep rejects JSON null payload

**Given** an `ActivityTask` with `payload = Bytes::from_static(b"null")` and `activity_type = "sleep"`
**When** `sleep_handler(task)` is awaited
**Then** the result is `Err(msg)` where `msg.contains("invalid payload")`.

### Scenario 10c: Sleep rejects JSON string payload

**Given** an `ActivityTask` with `payload = Bytes::from_static(br#""hello""#)` and `activity_type = "sleep"`
**When** `sleep_handler(task)` is awaited
**Then** the result is `Err(msg)` where `msg.contains("invalid payload")`.

### Scenario 10d: Sleep rejects JSON boolean payload

**Given** an `ActivityTask` with `payload = Bytes::from_static(b"true")` and `activity_type = "sleep"`
**When** `sleep_handler(task)` is awaited
**Then** the result is `Err(msg)` where `msg.contains("invalid payload")`.

### Scenario 10e: Sleep accepts payload with extra JSON fields (lenient parsing)

**Given** an `ActivityTask` with `payload = Bytes::from_static(br#"{"ms":10,"trace_id":"abc","extra":true}"#)` and `activity_type = "sleep"`
**And** a `tokio::time::pause()` handle is active
**When** `sleep_handler(task)` is awaited
**Then** the result is `Ok(Bytes::from_static(b"\"slept\""))`
**And** advancing the paused clock by 10ms causes the handler to complete.
**And** extra fields (`trace_id`, `extra`) are silently ignored (lenient parsing).

### Scenario 10f: Sleep with ms=u64::MAX is accepted without hanging

**Given** an `ActivityTask` with `payload = Bytes::from_static(br#"{"ms":18446744073709551615}"#)` and `activity_type = "sleep"`
**And** a `tokio::time::pause()` handle is active
**When** `sleep_handler(task)` is awaited
**Then** the result is `Ok(Bytes::from_static(b"\"slept\""))`
**And** the test completes without advancing the paused clock (verifying acceptance without waiting ~584M years).

### Scenario 11: register_defaults enables echo handler via dispatch (E2E, requires NATS)

**Given** a provisioned NATS JetStream server
**And** a `Worker` created via `Worker::new(ctx, "test-reg", None)` with no handlers registered
**When** `register_defaults(&mut worker)` is called
**And** an `ActivityTask` with `activity_type = "echo"`, `payload = b"defaults-test"`, `attempt = 1` is enqueued via `enqueue_activity`
**And** `worker.run(shutdown_rx)` is spawned with a 350ms shutdown delay
**Then** `worker.run` completes without error
**And** the task is consumed (no redelivery on a subsequent pull with 200ms timeout).

### Scenario 12: register_defaults preserves existing handlers (E2E, requires NATS)

**Given** a provisioned NATS JetStream server
**And** a `Worker` with a custom `"ping"` handler already registered (returns `b"pong"`)
**When** `register_defaults(&mut worker)` is called
**And** an `ActivityTask` with `activity_type = "ping"`, `payload = b"test"`, `attempt = 1` is enqueued via `enqueue_activity`
**And** `worker.run(shutdown_rx)` is spawned with a 350ms shutdown delay
**Then** `worker.run` completes without error
**And** the task is consumed (custom handler still works after `register_defaults`).

### Scenario 13: register_defaults is idempotent (E2E, requires NATS)

**Given** a provisioned NATS JetStream server
**And** a `Worker` after `register_defaults(&mut worker)` has been called once
**When** `register_defaults(&mut worker)` is called a second time
**And** an `ActivityTask` with `activity_type = "echo"`, `payload = b"idempotent"`, `attempt = 1` is enqueued via `enqueue_activity`
**And** `worker.run(shutdown_rx)` is spawned with a 350ms shutdown delay
**Then** `worker.run` completes without error
**And** the task is consumed (no panic, no duplicate handler errors).

### Scenario 14: Echo task round-trip through worker (E2E, requires NATS)

**Given** a provisioned NATS JetStream server
**And** an `ActivityTask` with `activity_type = "echo"`, `payload = b"test-payload-1234"`, `attempt = 1`
**When** the task is enqueued via `enqueue_activity`
**And** a `Worker` is created with `register_defaults(&mut worker)`
**And** `worker.run(shutdown_rx)` is spawned with a 350ms shutdown delay
**Then** `worker.run` completes without error
**And** the task is consumed (no redelivery on a subsequent pull with 200ms timeout).

### Scenario 15: Sleep task round-trip through worker (E2E, requires NATS)

**Given** a provisioned NATS JetStream server
**And** an `ActivityTask` with `activity_type = "sleep"`, `payload = b"{\"ms\":10}"`, `attempt = 1`
**When** the task is enqueued via `enqueue_activity`
**And** a `Worker` is created with `register_defaults(&mut worker)`
**And** `worker.run(shutdown_rx)` is spawned with a 500ms shutdown delay
**Then** `worker.run` completes without error
**And** the task is consumed (no redelivery).

### Scenario 16: Worker acks unknown activity type without error (E2E, requires NATS)

**Given** a provisioned NATS JetStream server
**And** an `ActivityTask` with `activity_type = "nonexistent_handler"`, `attempt = 1`
**When** the task is enqueued via `enqueue_activity`
**And** a `Worker` is created with `register_defaults(&mut worker)`
**And** `worker.run(shutdown_rx)` is spawned with a 350ms shutdown delay
**Then** `worker.run` completes without error
**And** the task is acked (no redelivery on subsequent pull with 200ms timeout).

### Scenario 17: Sleep with invalid payload triggers fail_activity path (E2E, requires NATS)

**Given** a provisioned NATS JetStream server
**And** an `ActivityTask` with `activity_type = "sleep"`, `payload = b"garbage"`, `attempt = 1`, `retry_policy.max_attempts = 1`
**When** the task is enqueued via `enqueue_activity`
**And** a `Worker` is created with `register_defaults(&mut worker)`
**And** `worker.run(shutdown_rx)` is spawned with a 350ms shutdown delay
**Then** `worker.run` completes without error
**And** the task is consumed (handler returned `Err`, retries exhausted, `fail_activity` called, message acked).

---

## Verification Commands

```bash
# Unit tests for builtin module (handler functions only — no NATS, no Worker)
cargo test -p vo-worker -- builtin --test-threads=1

# Integration tests for defaults and round-trips (requires NATS at 127.0.0.1:4222)
cargo test --test worker_integration -- builtin_defaults -- --test-threads=1

# Clippy lint check
cargo clippy -p vo-worker -- -D warnings

# Full workspace compilation check
cargo check --workspace
```

## Implementation Notes

- **`tokio::time::pause()`**: All sleep duration assertions in unit tests use `tokio::time::pause()` for deterministic virtual time control. No wall-clock assertions are used. Pattern: create `PauseHandle`, call handler, `advance()`, assert completion. This eliminates CI flakiness from container scheduling and CPU throttling.
- **`register_defaults` tests**: Verified through E2E dispatch (Scenarios 11-13), not through `worker.handlers` inspection. `handlers` is private and `Worker::new` requires a live NATS `Context`.
- **Lenient parsing**: `sleep_handler` accepts payloads with extra JSON fields beyond `"ms"`. Only `"ms"` is extracted; all other fields are silently ignored.
- **Valid-JSON-non-object branch**: `serde_json::from_slice` succeeds for `42`, `null`, `"hello"`, `true`, but `.as_object()` returns `None`. These are distinct from invalid-JSON errors.
