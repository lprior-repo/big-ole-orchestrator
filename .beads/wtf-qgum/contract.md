# Contract Specification

## Metadata
- **bead_id**: wtf-qgum
- **bead_title**: worker: Implement echo and sleep activity handlers
- **phase**: STATE 2 (contract-first)
- **updated_at**: 2026-03-23

## Context

### Feature
Add two built-in activity handlers to `Worker` via `register_defaults()` in a new `builtin` module:
- `"echo"` — identity function: returns `task.payload` unchanged.
- `"sleep"` — parses `{"ms": <u64>}`, sleeps cooperatively via `tokio::time::sleep`, returns `"slept"`.

### Domain Terms
| Term | Definition |
|------|-----------|
| `ActivityTask` | Owned struct at `queue.rs:41-59` with `activity_id`, `activity_type`, `payload: Bytes`, `namespace`, `instance_id`, `attempt`, `retry_policy`, `timeout` |
| `Worker` | Dispatch loop at `worker.rs:88-93` with `handlers: HashMap<String, ActivityHandler>` |
| `ActivityHandler` | Type alias at `worker.rs:78-82`: `Arc<dyn Fn(ActivityTask) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send>> + Send + Sync>` |
| `Worker::register` | Method at `worker.rs:119-126` — inserts handler into HashMap (overwrites on duplicate key) |
| `process_task` | Dispatch loop at `worker.rs:210-303` — clones task, calls handler, acks on success/fail, naks on JetStream append failure |

### Assumptions
- `serde_json` is already a workspace dependency (root `Cargo.toml:38`); safe to add to `wtf-worker/Cargo.toml`.
- `tokio::time::sleep` is already available via the existing `tokio` workspace dependency.
- Handlers are standalone `async fn` values — they capture nothing from environment.
- The `Bytes` type in `ActivityTask.payload` allows move semantics (no clone required for echo).

### Open Questions
- None. Spec is fully resolved against source.

---

## Preconditions

### P1: `register_defaults` requires a mutable `Worker` reference
- `worker: &mut Worker` must be borrowable — caller must create worker as `let mut worker`.
- `Worker::new()` does not require NATS connectivity; `register_defaults` is purely in-memory.

### P2: `echo_handler` requires an owned `ActivityTask`
- The handler receives `ActivityTask` by value (owned move).
- `task.payload` is `Bytes` — moved into handler via `process_task` (worker.rs:225 clones before dispatch).

### P3: `sleep_handler` requires valid JSON payload with `"ms"` field
- `task.payload` must be a valid JSON object containing a `"ms"` key with a `u64` value.
- Any deviation (invalid UTF-8, non-JSON, missing key, wrong type) is a contract violation.

---

## Postconditions

### Q1: After `register_defaults(&mut worker)`, worker.handlers contains exactly `"echo"` and `"sleep"`
- `worker.handlers.contains_key("echo") == true`
- `worker.handlers.contains_key("sleep") == true`
- Additional pre-existing handlers are NOT removed.

### Q2: `echo_handler(task)` returns `Ok(task.payload)`
- Result is `Ok(Bytes)` where the `Bytes` are byte-identical to the input `task.payload`.
- The handler does NOT read, validate, or interpret the payload content.

### Q3: `sleep_handler(task)` returns `Ok(Bytes::from_static(b"\"slept\""))` after sleeping
- Sleep duration is exactly `Duration::from_millis(ms)` where `ms` is parsed from payload.
- The returned `Bytes` are the 7-byte JSON string `"slept"` (with quotes: `22 73 6c 65 70 74 22`).
- The handler uses `tokio::time::sleep` (cooperative, does NOT block the tokio runtime).

### Q4: `sleep_handler(task)` returns `Err(String)` for invalid payload
- Error message contains `"invalid payload"` substring.
- Error message includes the expected format: `expected {"ms": <u64>}`.

### Q5: `register_defaults` is idempotent
- Calling twice replaces handlers with identical logic; no duplicate entries, no panic, no error.

---

## Invariants

### I1: Handler count invariant
- `register_defaults` registers exactly 2 handlers: `"echo"` and `"sleep"`.
- Pre-existing handlers in the map are preserved (HashMap insert on existing key overwrites only that key).

### I2: No-panic invariant
- Neither `echo_handler` nor `sleep_handler` panics under any input.
- All failure modes return `Err(String)`.

### I3: Cooperative async invariant
- `sleep_handler` uses `tokio::time::sleep` — never `std::thread::sleep`.
- The handler does not block the tokio executor thread.

### I4: Ownership contract
- `echo_handler` moves `task.payload` into the return value (zero-copy on the `Bytes` inner buffer).
- `sleep_handler` reads from `task.payload` (borrows the bytes for JSON parsing), then drops it.

### I5: Error type contract
- Both handlers return `Result<Bytes, String>` matching the `ActivityHandler` type alias.
- No custom error types; the `String` error message is human-readable and suitable for logging.

### I6: Module-level lint invariant
- `builtin.rs` must contain: `#![deny(clippy::unwrap_used)]`, `#![deny(clippy::expect_used)]`, `#![deny(clippy::panic)]`, `#![warn(clippy::pedantic)]`, `#![forbid(unsafe_code)]`.

---

## Error Taxonomy

| Error Variant | Source | Condition | Message Pattern |
|---------------|--------|-----------|-----------------|
| `Err(String)` | `sleep_handler` | Payload is not valid JSON | `"sleep handler: invalid payload: expected {\"ms\": <u64>}"` |
| `Err(String)` | `sleep_handler` | JSON is valid but `"ms"` key missing | `"sleep handler: invalid payload: expected {\"ms\": <u64>}"` |
| `Err(String)` | `sleep_handler` | `"ms"` value is not `u64` (e.g. string, float, nested object) | `"sleep handler: invalid payload: expected {\"ms\": <u64>}"` |
| (never) | `echo_handler` | Echo never fails — all payloads are valid | N/A |

### Error propagation (handled by `process_task` at worker.rs:252-299)
- `Err(String)` from a handler triggers `retries_exhausted()` check.
- If retries remain: task is re-enqueued with `attempt + 1`.
- If retries exhausted: `fail_activity()` appends `ActivityFailed` event.
- If `fail_activity()` itself fails: message is nacked for redelivery.

---

## Contract Signatures

```rust
// builtin.rs — public entry point
/// Register built-in activity handlers on a Worker.
///
/// Registers `"echo"` (returns payload unchanged) and `"sleep"` (parses `{"ms": u64}`, sleeps, returns `"slept"`).
/// Idempotent — calling twice overwrites with identical handlers.
pub fn register_defaults(worker: &mut Worker)

// builtin.rs — internal handler functions (pub for unit testing)
/// Returns `task.payload` as-is. Never fails.
pub async fn echo_handler(task: ActivityTask) -> Result<Bytes, String>

/// Parses `{"ms": u64}` from payload, sleeps for `ms` milliseconds via `tokio::time::sleep`,
/// then returns `Ok(Bytes::from_static(b"\"slept\""))`.
///
/// # Errors
/// Returns `Err("sleep handler: invalid payload: expected {\"ms\": <u64>}")` if:
/// - payload is not valid UTF-8
/// - payload is not valid JSON
/// - JSON object does not contain a `"ms"` field with a `u64` value
pub async fn sleep_handler(task: ActivityTask) -> Result<Bytes, String>
```

---

## Non-goals

- Adding new fields to `ActivityTask`.
- Modifying `Worker::register()` or the `ActivityHandler` type alias.
- Implementing custom error types (using `String` errors per existing convention).
- Adding a built-in `"http"` or `"grpc"` call handler (future work).
- Modifying the `process_task` dispatch loop.
- Timeout enforcement within the handlers (the `task.timeout` field is handled by the Worker loop, not by handlers).
