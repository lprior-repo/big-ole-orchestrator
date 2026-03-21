# Contract Specification: Per-Activity Timeout Support

## Context
- **Feature**: Add per-activity timeout support to wtf-worker
- **Bead ID**: wtf-qsdw
- **Domain terms**:
  - `ActivityTask`: Work item dispatched by engine, pulled by worker
  - `timeout`: Maximum duration an activity may run before being cancelled
  - `ActivityHandler`: User-defined async function that executes activity logic
- **Assumptions**:
  - Timeout is specified at task dispatch time, not globally
  - Timeout cancellation triggers `fail_activity` with a timeout-specific error
  - `tokio::time::timeout` is used for timeout enforcement
- **Open questions**: None

## Preconditions
- [ ] `timeout` field is `Some(Duration)` for timed activities, `None` for no timeout
- [ ] `timeout` >= 1 millisecond when `Some`
- [ ] `ActivityTask` is well-formed before worker processes it

## Postconditions
- [ ] Activity execution completes within `timeout` duration OR is cancelled
- [ ] When cancelled, `ActivityFailed` is appended with `ActivityError::TimeoutElapsed`
- [ ] `duration_ms` in `ActivityCompleted` reflects actual elapsed time (not truncated)
- [ ] Task is acked after completion (success or failure)
- [ ] Task is nak'd only on transient NATS errors, not on timeout

## Invariants
- [ ] `ActivityTask` fields are never mutated during processing
- [ ] Handler is always looked up before execution
- [ ] Every `process_task` call results in exactly one ack or nak

## Error Taxonomy
- `ActivityError::TimeoutElapsed` - Activity ran longer than its configured timeout
- `ActivityError::NatsPublish` - Failed to append completion/failure event
- `ActivityError::HandlerNotFound` - No handler registered for activity type
- `ActivityError::HandlerPanicked` - Handler future panicked (caught by tokio)

## Contract Signatures

### ActivityTask (modified)
```rust
pub struct ActivityTask {
    pub activity_id: ActivityId,
    pub activity_type: String,
    pub payload: Bytes,
    pub namespace: NamespaceId,
    pub instance_id: InstanceId,
    pub attempt: u32,
    pub retry_policy: RetryPolicy,
    pub timeout: Option<Duration>,  // NEW: per-activity timeout
}
```

### Worker::process_task (modified)
```rust
async fn process_task(&self, ackable: AckableTask) {
    // ... timeout enforcement using tokio::time::timeout
}
```

## Type Encoding
| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| timeout None means no limit | Runtime-checked | `Option<Duration>` |
| timeout >= 1ms when Some | Runtime-checked constructor | `Duration::from_millis >= 1` |
| timeout is Serialize/Deserialize | Type-level | `serde` derives on ActivityTask |
| Handler exists for activity_type | Runtime-checked | `handlers.get()` returns Some |

## Violation Examples (REQUIRED)
- VIOLATES <P1>: `ActivityTask { timeout: Some(Duration::ZERO), .. }` — timeout of 0ms should be rejected at construction
- VIOLATES <P2>: `ActivityTask { timeout: Some(Duration::from_nanos(1)), .. }` — sub-millisecond timeout should be rejected at construction
- VIOLATES <Q1>: Activity runs 500ms with `timeout: Some(Duration::from_millis(100))` — should produce `Err(ActivityError::TimeoutElapsed)`

## Ownership Contracts
- `ActivityTask` is cloned before passing to handler (ownership transfer to user code)
- `timeout` field is read-only during processing (no mutation)
- `&self` receiver on `process_task` (shared access to handlers map)

## Non-goals
- Global worker timeout (per-task only)
- Retry on timeout (timeout is treated as permanent failure)
- Timeout extension / renewal mid-execution
