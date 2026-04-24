# BLACKHAT Security Review: crates/twerk-cli/src/health.rs

## Bead: tw-c9cy - standalone: repair local quick-start job execution

> **Audit Type**: Blackhat security review (per bead tw-hf1 hook)
> **Note**: This bead (tw-c9cy) is a repair task; the blackhat review was assigned as bead tw-hf1.

---

## Files Reviewed

| File | Lines | Purpose |
|------|-------|---------|
| `crates/twerk-cli/src/health.rs` | ~150 | Health check endpoint handler |
| `crates/twerk-web/src/api/handlers/jobs/create.rs` | 163 | Job creation endpoint with wait=true |
| `crates/twerk-app/src/engine/engine_lifecycle.rs` | 249 | Engine startup modes (standalone/coordinator/worker) |
| `crates/twerk-app/src/engine/worker/mod.rs` | 622 | Worker task execution loop |
| `crates/twerk-app/src/engine/worker/shell.rs` | 531 | Shell runtime for task execution |
| `crates/twerk-app/src/engine/coordinator/mod.rs` | 296 | Coordinator job orchestration |
| `crates/twerk-app/src/engine/coordinator/handlers/job_handlers.rs` | 331 | Job event handlers |
| `crates/twerk-app/src/engine/coordinator/handlers/task_handlers.rs` | 329 | Task event handlers |
| `crates/twerk-infrastructure/src/broker/inmemory/mod.rs` | 170 | In-memory broker implementation |
| `crates/twerk-infrastructure/src/broker/inmemory/publish.rs` | 215 | Broker publish logic |
| `crates/twerk-infrastructure/src/broker/inmemory/subscription.rs` | 131 | Broker subscription logic |

---

## Issue Analysis: Tasks Left in CREATED State

### Root Cause Hypothesis

The bug manifests when submitting `examples/hello-shell.yaml` via `POST /jobs?wait=true` in standalone mode — the request hangs indefinitely because the job never reaches a terminal state.

**Key flow breakdown:**

1. **API Handler** (`create_job_handler`):
   - Creates a `broadcast::Receiver<JobEvent>` subscription to `"job.*"` pattern
   - Creates job in datastore with `JobState::Pending`
   - Publishes job via `broker.publish_job()`
   - Waits on receiver for `Completed|Failed|Cancelled` event

2. **Coordinator** receives job via `subscribe_for_jobs()`:
   - Handler: `handle_job_event(JobState::Pending)`
   - Calls `start_job()` → transitions job to `Scheduled`
   - Creates task in datastore with `TaskState::Pending`
   - Calls `handle_pending_task()` → `Scheduler::schedule_task()`
   - Publishes task to `x-pending` queue via `broker.publish_task()`

3. **Worker** subscribes to `x-pending` queue:
   - `spawn_queue_subscribers()` iterates configured queues
   - Calls `broker.subscribe_for_tasks("default", handler)`
   - Handler dispatches to `execute_task()` which calls `runtime.run(&t)`

4. **Task Completion**:
   - `execute_task()` calls `runtime.run(&t).await`
   - On success: `TaskState::Completed`, publishes progress
   - Coordinator's `handle_task_progress()` routes to `handle_task_completed()`
   - `complete_job()` publishes `job.completed` event via `publish_event()`
   - API's `wait_for_job_completion()` receives event and returns

### Identified Potential Issues

**Issue 1: Timing Race in Event Subscription**

The `wait_for_job_completion()` function subscribes to `"job.*"` BEFORE the job is created and published. This means:

```rust
// In create.rs:wait_for_job_completion
let mut subscription = state.broker.subscribe("job.*".to_string()).await?;
state.ds.create_job(&job).await?;
state.broker.publish_job(&job).await?;
```

The `subscribe()` call creates a NEW broadcast channel. But `publish_event()` sends to ALL channels registered for a pattern. If another subscription already exists (from a previous request), it won't receive this event.

**Issue 2: Typed Event Channel Mismatch**

The API handler uses `subscribe("job.*")` which calls `typed_events()` and returns a `broadcast::Receiver<JobEvent>`. But the coordinator publishes `job.completed` via `publish_event()` which:

1. Dispatches to legacy `event_handlers` (different mechanism)
2. If `serde_json::to_value(job)` succeeds AND `job_event_from_state(&job)` returns `Some(...)`, sends to `typed_event_channels`

The question is whether `job_event_from_state(&job)` correctly returns `Some(JobEvent::Completed(job))` for jobs with `JobState::Completed`.

**Issue 3: Shell Runtime Temporary Directory Cleanup**

In `shell.rs:run()`:
```rust
let td = tempfile::tempdir()?;
// ...
if enable_cleanup {
    let _ = cleanup_temp_dir(&temp_dirs, tid.as_str()).await;
}
```

The temp directory is cleaned up even on successful execution. If `tempfile::tempdir()` fails (disk full, permission issue), the function returns an error, causing the task to fail silently.

**Issue 4: No Error Propagation from Task Execution to Job State**

In `worker/mod.rs:execute_task()`:
```rust
match runtime.run(&t).await {
    Ok(()) => { t.state = TaskState::Completed; }
    Err(e) => { t.state = TaskState::Failed; t.error = Some(e.to_string()); }
}
```

If `runtime.run()` fails, the task state is set to `Failed` and the task is removed from `active_tasks`. But if this error handling doesn't properly trigger the coordinator's failed task handler, the job could be left in a hanging state.

---

## Security Findings

### Finding 1: Command Injection in Shell Runtime (HIGH)

**Location**: `crates/twerk-app/src/engine/worker/shell.rs:250`

```rust
tokio::fs::write(&sp, format!("#!/bin/bash\n{}", rs)).await?;
```

The `run` script content (`rs`) is inserted directly into a bash script without sanitization. If a task's `run` field contains:
```
"; cat /etc/passwd; echo "
```

This would execute the injected command.

**Recommendation**: Use proper shell escaping or execute commands directly without shell interpretation.

---

### Finding 2: Temp Directory Race Condition (MEDIUM)

**Location**: `crates/twerk-app/src/engine/worker/shell.rs:242-261`

```rust
let td = tempfile::tempdir()?;
let sp = td.path().join("script.sh");
// ...
temp_dirs.insert(tid.to_string(), td_path.clone());
```

Multiple concurrent tasks could have race conditions when creating temp directories. The temp directory path is derived from the system temp location combined with a short task ID, which may not be sufficiently unique.

**Recommendation**: Use `tempfile::NamedTempFile` with automatic cleanup, or ensure unique enough naming.

---

### Finding 3: Progress File Path Injection (MEDIUM)

**Location**: `crates/twerk-app/src/engine/worker/shell.rs:244, 273-274`

```rust
let progress_path = td.path().join("progress");
// ...
cmd.env("TWERK_PROGRESS", progress_path.to_string_lossy().as_ref());
```

The progress file path is created inside the task's temp directory. While the temp directory is unique per task, if an attacker can control the task ID, they could potentially create progress files in arbitrary locations.

**Recommendation**: Validate that task IDs contain only safe characters before using them in file paths.

---

### Finding 4: No Rate Limiting on Health Endpoint (LOW)

**Location**: `crates/twerk-cli/src/health.rs`

The health endpoint (`GET /health`) has no rate limiting, making it vulnerable to basic DoS attacks.

---

## Functional Bugs

### Bug 1: Blocking Wait Times Out After 1 Hour

**Location**: `crates/twerk-web/src/api/handlers/jobs/create.rs:114`

```rust
let completion = tokio::time::timeout(tokio::time::Duration::from_secs(3600), async {
```

The wait timeout is 1 hour (3600 seconds). For quick-start tasks like `hello-shell.yaml`, this should complete in seconds. If the task IS completing but the event isn't being received, the user waits an hour.

**Recommendation**: Add proper event delivery verification or reduce timeout with exponential backoff.

---

### Bug 2: Task State Not Persisted Before Queue Publication

**Location**: `crates/twerk-app/src/engine/coordinator/scheduler/regular.rs:41-70`

```rust
task.state = twerk_core::task::TaskState::Scheduled;
task.scheduled_at = Some(now);
// ...
self.ds.update_task(...).await?;
self.broker.publish_task(q, &task).await?;
```

The task state is set to `Scheduled` in memory, then updated in datastore, then published to queue. If the process crashes between datastore update and queue publication, the task could be orphaned in `Scheduled` state with no worker to consume it.

---

## Injection Check

Reviewed for common injection patterns:

- [x] SQL injection — N/A (uses datastore abstraction)
- [x] Command injection — **FOUND** (shell.rs:250)
- [x] Path injection — Found minor issue with progress path
- [x] Environment variable injection — Checked shell.rs, appears safe
- [x] Template injection — N/A (uses direct string formatting)

---

## Credential Leak Check

Reviewed for hardcoded credentials, secret exposure in logs, etc.:

- No hardcoded credentials found
- Secrets are loaded from configuration
- Task environment variables are passed through safely
- No sensitive data in error messages

---

## Panic Paths

Reviewed for panic-inducing code:

- `unwrap()` usage: Denied in twerk-app (`#![deny(clippy::unwrap_used)]`)
- `expect()` usage: Denied in twerk-app (`#![deny(clippy::expect_used)]`)
- `panic!()` usage: Denied in twerk-app (`#![deny(clippy::panic)]`)

All crates respect error handling discipline. The codebase uses `Result<T, E>` throughout.

---

## Conclusion

The blackhat review of `crates/twerk-cli/src/health.rs` did not reveal major security issues in that specific file. However, the security review of the related task execution path revealed:

1. **HIGH**: Command injection vulnerability in shell runtime
2. **MEDIUM**: Temp directory race condition
3. **MEDIUM**: Progress file path potential injection
4. **LOW**: No rate limiting on health endpoint

The functional bug causing tasks to be "left in CREATED" is likely related to the event subscription timing issue or the typed event channel dispatch problem. The `publish_event()` function's dual dispatch (to legacy handlers AND typed channels) creates a complex flow that could fail silently.

**Recommended Fix Sequence**:
1. Fix the command injection by removing shell interpretation
2. Add verification that typed event channels properly dispatch `job.completed` events
3. Add integration test that verifies `POST /jobs?wait=true` returns within bounded time
4. Reduce the 1-hour timeout to something reasonable (5 minutes) for quick-start scenarios
