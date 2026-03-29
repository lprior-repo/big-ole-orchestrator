---
bead_id: vo-qgum
title: "worker: Implement echo and sleep activity handlers"
effort_estimate: "30min"
status: draft
type: task
priority: 2
labels: [worker, activities, builtins, dispatch, integration]
---

# Section 0: Clarifications

- **Scope**: Add two built-in activity handlers to `Worker` via a `register_defaults()` method: `"echo"` (returns input payload unchanged) and `"sleep"` (tokio::sleep then returns empty success). This proves the dispatch chain works end-to-end without custom handler registration.
- **Location**: New file `crates/vo-worker/src/builtin.rs` with a `pub fn register_defaults(worker: &mut Worker)` function. Re-export from `crates/vo-worker/src/lib.rs`.
- **Pattern**: Follows the existing `Worker::register()` signature — handlers are `Fn(ActivityTask) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send>>`.
- **Integration test**: Extend `crates/vo-worker/tests/worker_integration_tests.rs` with tests that use `register_defaults()` and enqueue `"echo"` / `"sleep"` tasks to verify full NATS round-trip.

---

# Section 1: EARS (Requirements)

**WHEN** `register_defaults(&mut worker)` is called
**THEN** the system SHALL register an `"echo"` handler that returns `task.payload` as-is.

**WHEN** a worker with `"echo"` handler processes an `ActivityTask` with `activity_type = "echo"` and `payload = b"hello"`
**THEN** the handler SHALL return `Ok(Bytes::from_static(b"hello"))` and the worker SHALL append `ActivityCompleted` to JetStream with that result.

**WHEN** a worker with `"sleep"` handler processes an `ActivityTask` with `activity_type = "sleep"`
**THEN** the handler SHALL parse the payload as `{"ms": <u64>}`, sleep for that many milliseconds via `tokio::time::sleep`, then return `Ok(Bytes::from_static(b"\"slept\""))`.

**IF** the `"sleep"` payload is not valid JSON or does not contain a `"ms"` field
**THEN** the handler SHALL return `Err("sleep handler: invalid payload: expected {\"ms\": <u64>}")`.

**IF** a task arrives with `activity_type` not in `{"echo", "sleep", ...custom registered...}`
**THEN** the system SHALL ack it without execution and log a warning (existing behavior at `worker.rs:214-223`).

---

# Section 2: KIRK Contracts

## Contract: `register_defaults`

```rust
/// Register built-in activity handlers on a Worker.
///
/// - `"echo"` — returns `task.payload` unchanged.
/// - `"sleep"` — parses `{"ms": u64}` from payload, sleeps, returns `"slept"`.
pub fn register_defaults(worker: &mut Worker)
```

**Invariants:**
- I1: `register_defaults` registers exactly two handlers: `"echo"` and `"sleep"`.
- I2: Calling `register_defaults` twice is idempotent (second call overwrites with identical handlers).
- I3: Neither handler panics — both return `Result<Bytes, String>`.

## Contract: echo handler

```
Input:  ActivityTask { payload: Bytes, ... }
Output: Ok(payload.clone())
```

**Invariants:**
- I1: Output bytes are byte-identical to input payload.
- I2: Allocation: clones the payload (required because handler takes owned `ActivityTask` and result must be independent).

## Contract: sleep handler

```
Input:  ActivityTask { payload: Bytes, ... }  where payload = {"ms": <u64>}
Output: Ok(Bytes::from_static(b"\"slept\""))  after tokio::time::sleep(Duration::from_millis(ms))
Error:  Err("sleep handler: invalid payload: expected {\"ms\": <u64>}")
```

**Invariants:**
- I1: Sleep duration equals the `ms` field value in the payload.
- I2: Returns `Err(String)` for non-JSON payload or missing `"ms"` key.
- I3: Uses `tokio::time::sleep` (cooperative, not `std::thread::sleep`).

---

# Section 2.5: Research

**Existing handler registration pattern:**

```rust
// worker.rs:119-126
pub fn register<F, Fut>(&mut self, activity_type: impl Into<String>, handler: F)
where
    F: Fn(ActivityTask) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Bytes, String>> + Send + 'static,
{
    let boxed: ActivityHandler = Arc::new(move |task| Box::pin(handler(task)));
    self.handlers.insert(activity_type.into(), boxed);
}
```

**Worker processing loop** (`worker.rs:210-303`):
1. Looks up handler by `task.activity_type` in `self.handlers` HashMap.
2. If no handler: warns and acks (line 214-223).
3. If handler found: calls `handler(task_clone).await` (line 227).
4. On `Ok(result)`: calls `complete_activity(...)` then acks (line 231-250).
5. On `Err(error)`: handles retry logic via `retries_exhausted` / `calculate_backoff_delay` / `fail_activity`, then acks (line 252-299).

**Integration test pattern** (`worker_integration_tests.rs:184-210`):
```rust
let mut worker = Worker::new(server.js.clone(), "test-worker", None);
worker.register("send_email", |task| async move {
    Ok(Bytes::from_static(b"\"sent\""))
});
let (shutdown_tx, shutdown_rx) = watch::channel(false);
// spawn shutdown after delay, run worker with timeout
```

**Key types from `vo_common`:**
- `VoError` — error type used across crates
- `ActivityId`, `InstanceId`, `NamespaceId`, `RetryPolicy` — all in `vo_common`

**Key types from `vo_worker::queue`:**
- `ActivityTask { activity_id, activity_type, payload, namespace, instance_id, attempt, retry_policy, timeout }` at `queue.rs:41-59`

---

# Section 3: Inversions

| # | Dependency | Strategy |
|---|-----------|----------|
| 1 | `tokio::time::sleep` runtime | Already available — `tokio` is a workspace dependency in `Cargo.toml` |
| 2 | `serde_json` for parsing sleep payload | NOT currently a dependency of `vo-worker` — use `rmp_serde` (already available) or add `serde_json` to `Cargo.toml` |
| 3 | Live NATS server for integration tests | Same pattern as existing tests — `NatsTestServer` helper at `worker_integration_tests.rs:28-66` |

---

# Section 4: ATDD Tests (Unit)

### T1: `register_defaults` registers echo and sleep handlers

```rust
#[test]
fn register_defaults_adds_echo_and_sleep_handlers() {
    // Cannot fully test without NATS Context, but verify the function exists
    // and compiles. Actual behavior verified in integration tests.
}
```

### T2: echo handler returns payload unchanged

```rust
#[tokio::test]
async fn echo_handler_returns_payload_unchanged() {
    let task = ActivityTask {
        activity_id: ActivityId::new("act-echo"),
        activity_type: "echo".to_owned(),
        payload: Bytes::from_static(b"hello world"),
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("inst-1"),
        attempt: 1,
        retry_policy: RetryPolicy::default(),
        timeout: None,
    };
    let result = builtin::echo_handler(task).await;
    assert_eq!(result.unwrap(), Bytes::from_static(b"hello world"));
}
```

### T3: sleep handler sleeps and returns success

```rust
#[tokio::test]
async fn sleep_handler_sleeps_and_returns_slept() {
    let task = ActivityTask {
        activity_id: ActivityId::new("act-sleep"),
        activity_type: "sleep".to_owned(),
        payload: Bytes::from_static(b"{\"ms\":10}"),
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("inst-1"),
        attempt: 1,
        retry_policy: RetryPolicy::default(),
        timeout: None,
    };
    let start = std::time::Instant::now();
    let result = builtin::sleep_handler(task).await;
    assert!(result.is_ok());
    assert!(start.elapsed() >= Duration::from_millis(10));
}
```

### T4: sleep handler rejects invalid JSON payload

```rust
#[tokio::test]
async fn sleep_handler_rejects_invalid_json() {
    let task = ActivityTask {
        activity_id: ActivityId::new("act-sleep"),
        activity_type: "sleep".to_owned(),
        payload: Bytes::from_static(b"not json"),
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("inst-1"),
        attempt: 1,
        retry_policy: RetryPolicy::default(),
        timeout: None,
    };
    let result = builtin::sleep_handler(task).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid payload"));
}
```

### T5: sleep handler rejects missing ms field

```rust
#[tokio::test]
async fn sleep_handler_rejects_missing_ms_field() {
    let task = ActivityTask {
        activity_id: ActivityId::new("act-sleep"),
        activity_type: "sleep".to_owned(),
        payload: Bytes::from_static(b"{\"other\": 42}"),
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("inst-1"),
        attempt: 1,
        retry_policy: RetryPolicy::default(),
        timeout: None,
    };
    let result = builtin::sleep_handler(task).await;
    assert!(result.is_err());
}
```

---

# Section 5: E2E Tests (Integration)

### E1: Echo task round-trip through worker (requires NATS)

```rust
#[tokio::test]
async fn echo_task_round_trip_through_worker() {
    // 1. NatsTestServer::new().await + provision
    // 2. Create ActivityTask { activity_type: "echo", payload: b"test-payload", ... }
    // 3. enqueue_activity(&server.js, &task).await
    // 4. Create Worker, call register_defaults(&mut worker)
    // 5. worker.run(shutdown_rx) with 350ms shutdown delay
    // 6. Assert worker.run returns Ok
    // 7. Assert task was consumed (no redelivery on second pull with timeout)
}
```

### E2: Sleep task round-trip through worker (requires NATS)

```rust
#[tokio::test]
async fn sleep_task_round_trip_through_worker() {
    // 1. NatsTestServer::new().await + provision
    // 2. Create ActivityTask { activity_type: "sleep", payload: b"{\"ms\":10}", ... }
    // 3. enqueue_activity(&server.js, &task).await
    // 4. Create Worker, call register_defaults(&mut worker)
    // 5. worker.run(shutdown_rx) with 500ms shutdown delay (enough for sleep)
    // 6. Assert worker.run returns Ok
}
```

### E3: Worker with defaults ignores unknown activity types (requires NATS)

```rust
#[tokio::test]
async fn worker_with_defaults_ignores_unknown_type() {
    // 1. Enqueue task with activity_type: "unknown_type"
    // 2. Create Worker, call register_defaults(&mut worker)
    // 3. worker.run(shutdown_rx) — should complete without error
    // 4. Assert task was acked (no redelivery)
}
```

---

# Section 5.5: Verification Gates

```bash
# Unit tests
cargo test -p vo-worker -- builtin

# Integration tests (requires NATS)
cargo test --test worker_integration -- builtin_defaults

# Lint
cargo clippy -p vo-worker -- -D warnings

# Full workspace check
cargo check --workspace
```

---

# Section 6: Implementation Tasks

1. **Create `crates/vo-worker/src/builtin.rs`** — new module with:
   - `pub fn register_defaults(worker: &mut Worker)` — calls `worker.register("echo", echo_handler)` and `worker.register("sleep", sleep_handler)`
   - `async fn echo_handler(task: ActivityTask) -> Result<Bytes, String>` — returns `Ok(task.payload)`
   - `async fn sleep_handler(task: ActivityTask) -> Result<Bytes, String>` — parses `{"ms": u64}` from `task.payload`, `tokio::time::sleep(Duration::from_millis(ms)).await`, returns `Ok(Bytes::from_static(b"\"slept\""))`; returns `Err` on parse failure
   - Add `#![deny(clippy::unwrap_used)]`, `#![deny(clippy::expect_used)]`, `#![deny(clippy::panic)]`, `#![warn(clippy::pedantic)]`, `#![forbid(unsafe_code)]` at module top
   - Unit tests in `#[cfg(test)] mod tests`

2. **Add `serde_json` to `crates/vo-worker/Cargo.toml`** — needed for parsing the sleep payload JSON. Alternatively, since `rmp-serde` is already available and `serde_json::from_slice` is the standard approach, add `serde_json = { workspace = true }` to `[dependencies]`.

3. **Update `crates/vo-worker/src/lib.rs`** — add `pub mod builtin;` and `pub use builtin::register_defaults;`.

4. **Add integration tests to `crates/vo-worker/tests/worker_integration_tests.rs`**:
   - Import `vo_worker::register_defaults`
   - Add `echo_task_round_trip_through_worker` test
   - Add `sleep_task_round_trip_through_worker` test
   - Add `worker_with_defaults_ignores_unknown_type` test

---

# Section 7: Failure Modes

| Failure | Detection | Mitigation |
|---------|-----------|------------|
| `serde_json::from_slice` fails on sleep payload | Returns `Err(String)` from handler | Worker calls `fail_activity` with error string, acks the message |
| Sleep payload has `"ms"` but not a u64 | Type mismatch in `serde_json` | Returns `Err("sleep handler: invalid payload: expected {\"ms\": <u64>}")` |
| NATS publish of `ActivityCompleted` fails after echo/sleep succeeds | `complete_activity` returns `Err` at `worker.rs:231-249` | Worker naks the message — task will be redelivered and handler runs again (idempotent for echo, safe retry for sleep) |
| `register_defaults` called on worker that already has custom `"echo"` handler | HashMap insert overwrites | Idempotent — documented in invariants. Caller decides order of registration. |

---

# Section 7.5: Anti-Hallucination

- `Worker::register()` exists at `crates/vo-worker/src/worker.rs:119-126` — do NOT reimplement it.
- `ActivityTask` struct is at `crates/vo-worker/src/queue.rs:41-59` — do NOT add fields to it.
- `complete_activity` function is at `crates/vo-worker/src/activity/reporting.rs` — called by the Worker internally, NOT by the handlers.
- `vo-common` types `ActivityId`, `InstanceId`, `NamespaceId`, `RetryPolicy` are used in `ActivityTask` construction — they already exist.
- The handler type is `ActivityHandler` (type alias at `worker.rs:78-82`) — do NOT change it.
- `tokio::time::sleep` requires `tokio` which is already a dependency (`Cargo.toml:13`).
- `serde_json` may NOT be in the workspace — check `Cargo.toml` workspace `[dependencies]` before adding it. If not present, add it to the workspace root `Cargo.toml` first.
- The `make_task()` helper in integration tests at `worker_integration_tests.rs:68-79` constructs `ActivityTask` — reuse it for new tests.

---

# Section 7.6: Context Survival

If the LLM context is lost, the following files contain the complete picture:
- `crates/vo-worker/src/worker.rs` — `Worker::register()` at line 119-126, `process_task` at line 210-303
- `crates/vo-worker/src/queue.rs` — `ActivityTask` struct at line 41-59
- `crates/vo-worker/src/lib.rs` — module re-exports, add `pub mod builtin;` and `pub use builtin::register_defaults;`
- `crates/vo-worker/Cargo.toml` — dependencies list, add `serde_json` if not present
- `crates/vo-worker/tests/worker_integration_tests.rs` — existing integration test pattern, `NatsTestServer` helper, `make_task()` helper

---

# Section 8: Completion Criteria

- [ ] `crates/vo-worker/src/builtin.rs` exists with `register_defaults`, `echo_handler`, `sleep_handler`
- [ ] `echo_handler` returns `Ok(task.payload)` — unit test passes
- [ ] `sleep_handler` parses `{"ms": u64}`, sleeps, returns `Ok(Bytes::from_static(b"\"slept\""))` — unit tests pass
- [ ] `sleep_handler` returns `Err` on invalid JSON / missing `"ms"` — unit tests pass
- [ ] `register_defaults` wired into `lib.rs` as `pub use builtin::register_defaults`
- [ ] Integration test: echo task round-trips through worker and is acked
- [ ] Integration test: sleep task round-trips through worker and is acked
- [ ] `cargo clippy -p vo-worker -- -D warnings` passes
- [ ] `cargo test -p vo-worker` passes (unit + integration)
- [ ] Zero `unwrap()` or `expect()` in new code
- [ ] Module-level clippy lints match existing pattern (`#![deny(clippy::unwrap_used)]` etc.)

---

# Section 9: Context

This bead proves the full activity dispatch chain works end-to-end: NATS enqueue → pull consumer → handler dispatch → result reporting → ack. The existing integration tests at `worker_integration_tests.rs` use inline closures as handlers (`worker.register("send_email", |task| async move { ... })`). This bead adds built-in handlers so callers can get a working worker with zero custom registration:

```rust
let mut worker = Worker::new(js, "builtin-worker", None);
vo_worker::register_defaults(&mut worker);
worker.run(shutdown_rx).await?;
```

The `"echo"` handler is the simplest possible activity (identity function). The `"sleep"` handler proves the dispatch chain handles async work that takes time. Together they validate the entire `Worker::process_task` code path at `worker.rs:210-303`.

---

# Section 10: AI Hints

- The handler closure must capture nothing from the environment — it only receives `ActivityTask` as an argument. This is why `echo_handler` is a standalone `async fn`, not a closure that captures.
- For the sleep handler JSON parsing, `serde_json::from_slice::<serde_json::Value>(&task.payload)` is the simplest approach. Extract `v["ms"].as_u64()` — returns `None` if missing or wrong type.
- Use `Bytes::from_static(b"\"slept\"")` for the sleep result — the quotes make it valid JSON string value, matching the pattern used in existing tests (`b"\"sent\""`, `b"\"ok\""`, `b"\"checkout_complete\""`).
- In integration tests, use `make_task("echo", 1)` / `make_task("sleep", 1)` then override `payload` and `activity_type` to match. This avoids duplicating the full struct construction.
- The shutdown delay for the sleep test must exceed the sleep duration: if payload is `{"ms":10}`, shutdown delay should be at least `350ms` (10ms sleep + processing overhead).
- The `register_defaults` function takes `&mut Worker` — callers must create the worker as `let mut worker = ...` before calling it.
