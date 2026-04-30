# ADR 050: Cancellation Safety Contract

## Status

Proposed

## Context

The Veloxide engine spawns dozens of background async tasks across `vo-actor`, `vo-worker`, `vo-sdk`, and `vo-executor`. Every one of these tasks uses `tokio::select!` with a shutdown channel (typically `broadcast::channel(1)`) and `tokio::spawn()` without structured cancellation propagation.

This creates three classes of silent data corruption:

1. **Aborted in-flight cycles lose state**: The `SpawnSupervisor` loop (ADR-046) claims cancellation safety in Section 8, but uses `broadcast::channel(1)` for shutdown with no CancellationToken. When the `tokio::spawn`ed loop task is aborted, any in-flight `process_cycle()` that was mid-write to Fjall storage is silently lost. No Drop ordering, no structured concurrency, no propagation of cancellation from parent to child.

2. **SDK user tasks abort without cleanup**: The SDK runtime (`vo-sdk/src/runtime.rs`) spawns user tasks via `tokio::spawn()` and calls `abort_handle().abort()` on SIGTERM. The user's `Drop` implementations on any shared state are skipped. If the user task held a `MmapCache` write guard, a `SemaphorePermit`, or a database transaction, these resources are leaked or left inconsistent.

3. **Subprocess escape**: `run_subprocess` uses `tokio::timeout` + SIGTERM + SIGKILL. If the timeout fires, the subprocess is SIGKILL'd immediately. Any IPC pipes (ADR-018) the subprocess had open are severed without cleanup. The parent's file descriptor readers/writers may block forever or read garbage.

**Root cause**: Zero use of `CancellationToken` anywhere in the codebase. All shutdown uses one-shot `broadcast::channel(1)` which signals "stop" but does not propagate cancellation to children, does not coordinate Drop ordering, and does not distinguish between "graceful shutdown" and "forceful cancel."

This ADR defines the Cancellation Safety Contract: what cancellation means at different levels, how CancellationToken is used, Drop ordering guarantees, structured concurrency requirements, and the contract for each subsystem.

## Decision

### 1. Cancellation Safety Levels

We define three levels of cancellation safety, aligned with tokio's semantics:

#### Level 1: Cancellation-Oblivious (Current State - UNSHIPPABLE)

A task or resource is cancellation-oblivious if it makes no guarantees about state when dropped/cancelled mid-operation.

**Current examples:**
- `SpawnSupervisor` loop: mid-cycle cancellation loses state
- SDK user tasks: `abort()` skips all `Drop`
- Subprocess pipes: SIGKILL severs FDs without cleanup
- All `tokio::spawn()` tasks: no cancellation propagation

**Policy**: No new code may be cancellation-oblivious. Existing cancellation-oblivious code SHALL be upgraded to Level 2 minimum.

#### Level 2: Drop-Safe (Minimum for All Background Tasks)

A task or resource is drop-safe if cancellation via `tokio::task::abort()` or `CancellationToken::cancel()` leaves all state in a valid, recoverable state. This does NOT mean "operation completes." It means "state is not corrupted."

**Requirements:**
1. All `Drop` implementations on shared state MUST be cancellation-safe: calling `drop()` after a cancellation must not leave state inconsistent.
2. Any in-flight operation that may be cancelled MUST checkpoint its state before each await point such that dropping mid-operation leaves the system in a valid state.
3. No `Drop` implementation may block on I/O, network, or other async operations. `Drop` is synchronous.

**Implementation pattern:**
```rust
// BEFORE (drop-unsafe): in-flight write may be aborted mid-commit
async fn process_cycle(&self, record: &mut SpawnRecord) -> Result<(), Error> {
    let data = self.storage.prepare_write(&record).await?;  // await 1
    self.storage.commit(data).await?;                        // await 2 - ABORT HERE = LOST DATA
    Ok(())
}

// AFTER (drop-safe): checkpoint before each await
async fn process_cycle(&self, record: &mut SpawnRecord) -> Result<(), Error> {
    let prepared = self.storage.prepare_write(&record).await?;  // checkpoint 1
    let result = prepared.commit().await?;                       // if aborted here, prepared is still valid
    self.storage.finalize(result).await;                         // if aborted here, prepare is idempotent
    Ok(())
}
```

**Policy**: All background tasks spawned via `tokio::spawn()` MUST be drop-safe. This is the minimum bar.

#### Level 3: Cancellation-Propagating (Required for All Coordination-Critical Tasks)

A task or resource is cancellation-propagating if:
1. It holds a `CancellationToken` that is propagated from its parent.
2. When the parent cancels, all children receive the cancellation signal and shut down in order.
3. Drop ordering is respected: children drop before parents.
4. The task uses `CancellationToken::cancelled()` (not `broadcast::Receiver`) as its primary shutdown signal.

**Policy**: All coordination-critical tasks (supervisors, event loops, I/O loops, storage watchers) MUST be cancellation-propagating. Tasks that manage state machines, handle user work, or coordinate other tasks fall in this category.

### 2. CancellationToken Propagation Model

Every task that spawns child tasks MUST receive and propagate a `CancellationToken`. The propagation chain follows structured concurrency principles:

```
Engine (root CancellationToken)
  ├── ReanimatorLoop (child token)
  │     └── TimerScanner (grandchild token)
  ├── SpawnSupervisor (child token)
  │     └── ProcessCycle (grandchild token, per instance)
  ├── StorageWatchdog (child token)
  ├── LockSupervisor (child token)
  ├── HeartbeatRunner (child token)
  ├── TimerSupervisor (child token)
  └── WorkerPool (child token)
        ├── Worker(0) (grandchild token)
        ├── Worker(1) (grandchild token)
        └── ...
```

**Rules:**

1. **Root token is owned by the engine's main task**. It is the only token that can trigger a full engine shutdown.
2. **Each background task receives a child token via `token.child_token()`**. The child token is automatically cancelled when the parent is cancelled.
3. **Tasks select on `token.cancelled()` as their primary shutdown signal**, NOT `broadcast::Receiver`. Broadcast channels may still be used for one-time shutdown requests from external sources, but the token is the propagation mechanism.
4. **Tasks MUST await their children before exiting their own select loop**. This is enforced via `tokio::select!` ordering or explicit `JoinHandle` awaiting.
5. **No `tokio::spawn` without a CancellationToken**. If a task needs to spawn, it must hold a token and create a child token for the spawned task.

### 3. Structured Concurrency Enforcement

All background task lifecycles SHALL follow structured concurrency:

#### 3.1 Parent-Awaits-Child

A parent task MUST await the completion of all its child `JoinHandle`s before it itself completes:

```rust
// Pattern for background loops:
async fn run(mut self, token: CancellationToken) -> Result<(), Error> {
    let child_token = token.child_token();

    // Spawn child tasks
    let handle1 = tokio::spawn(self.child_task_1(child_token.clone()));
    let handle2 = tokio::spawn(self.child_task_2(child_token.clone()));

    // Main loop
    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            result = self.work_tick() => { /* ... */ }
        }
    }

    // CHILDREN MUST BE AWAITED BEFORE RETURNING
    // This guarantees Drop ordering: children drop first
    handle1.await?;
    handle2.await?;
    Ok(())
}
```

#### 3.2 No Orphaned Tasks

No `tokio::spawn()` call may exist without a corresponding cancellation token and await. The following patterns are prohibited:

- `tokio::spawn(...)` with no token propagation
- `tokio::spawn(...)` whose `JoinHandle` is never awaited
- `tokio::spawn(...)` inside a `tokio::select!` without cleanup

#### 3.3 Task Groups

Tasks that need to manage multiple dynamic children (e.g., WorkerPool with N workers) SHALL use a task group pattern:

```rust
pub struct TaskGroup {
    handles: Vec<JoinHandle<()>>,
    token: CancellationToken,
}

impl TaskGroup {
    pub fn spawn<F>(&mut self, f: F)
    where F: Future<Output = ()> + Send + 'static
    {
        let child_token = self.token.child_token();
        let handle = tokio::spawn(f.with_cancel(child_token));
        self.handles.push(handle);
    }

    pub async fn shutdown(&mut self) {
        self.token.cancel();
        for handle in std::mem::take(&mut self.handles) {
            let _ = handle.await;
        }
    }
}
```

### 4. Drop Ordering Guarantees

#### 4.1 Drop Is Synchronous and Must Not Block

All `Drop` implementations MUST complete synchronously and MUST NOT:
- Await on async operations
- Lock mutexes that may be held by the same task (deadlock risk)
- Send on channels that may be full (deadlock risk)
- Perform I/O that may block

#### 4.2 Drop Ordering Contract

When a task is cancelled or completes, the following Drop ordering applies:

```
Task cancellation order (LIFO - last spawned, first dropped):
1. Child tasks' local state (already dropped by child completion)
2. Parent task's local state (dropped when parent's async fn returns)
3. Shared state references (dropped when Arc refcount reaches 0)
```

**Requirement**: Shared state that is accessed by multiple tasks MUST be safe to drop from any task's `Drop` implementation, regardless of drop order. Use `Arc<Mutex<T>>` or `RwLock` for shared state, and ensure `Drop` on the underlying type is lock-free.

#### 4.3 Resource Cleanup in Drop

`Drop` implementations that need to clean up resources SHALL use a best-effort pattern:

```rust
impl Drop for SomeResource {
    fn drop(&mut self) {
        // Best-effort: do what you can, ignore failures
        let _ = self.flush();
        let _ = self.close();
        // Never block, never await
    }
}
```

### 5. Panic vs Cancellation

#### 5.1 Panics Are Not Cancellation

A panic in an async task does NOT trigger cancellation-safe cleanup. `Drop` IS called on the task's local variables (Rust guarantees this), but the task's `JoinHandle` receives a `JoinError`.

**Policy**: All async tasks MUST be panic-isolated:
- User tasks (SDK): `tokio::spawn(async { ... }).catch_unwind().await`
- Internal tasks: `tokio::spawn(async { ... }).catch_unwind().await`
- Panic details are logged via tracing, then the task is treated as "cancelled"

#### 5.2 Catch-All Unwind Wrapper

All `tokio::spawn` calls wrapping user-facing or coordination-critical code MUST use `.catch_unwind()`:

```rust
let handle = tokio::spawn(std::panic::AssertUnwindSafe(async {
    // task body
}).catch_unwind());
```

### 6. Subprocess Cancellation Contract

#### 6.1 Graceful Termination (Level 2)

The `run_subprocess` contract SHALL be updated:

1. On cancellation, send SIGTERM to the subprocess.
2. Wait up to 2 seconds for the subprocess to exit.
3. If subprocess does not exit, send SIGKILL.
4. After SIGKILL, wait up to 500ms for the process to fully exit.
5. Close all IPC pipes (FD3, FD4) regardless of subprocess state.
6. Drain any remaining pipe data before closing.

#### 6.2 Pipe Cleanup Guarantee

Per ADR-018, all IPC pipe FDs are managed by `FdGuard`. On subprocess cancellation:
- `FdGuard` Drop closes both ends of each pipe
- The parent's reader tasks receive `poll()` returning `Ready(None)` (EOF)
- The parent's writer tasks receive `BrokenPipeError` on next write
- No blocking on pipe I/O after cancellation

#### 6.3 No SIGKILL Without Pipe Cleanup

SIGKILL is never sent without first attempting pipe cleanup. The only exception is if the subprocess is a confirmed zombie (PID has no process table entry but parent hasn't reaped it). In that case, SIGKILL is sent and the FDs are closed immediately.

### 7. SDK Runtime Cancellation Contract

#### 7.1 User Task Lifecycle

The SDK runtime (`vo-sdk/src/runtime.rs`) SHALL provide:

1. **Graceful abort**: `task_handle.abort()` is replaced by `CancellationToken::cancel()`. The user task receives the cancellation signal at its next await point.
2. **Drop ordering**: User tasks MUST check `is_shutdown_requested()` at their entry point and at each await. On shutdown, the task completes its current iteration and exits cleanly.
3. **Resource cleanup**: User tasks that hold resources (DB transactions, file handles, locks) MUST clean them up before exiting, even on shutdown.

#### 7.2 SIGTERM Handling

The dedicated SIGTERM thread SHALL:
1. Send `CancellationToken::cancel()` to the engine's root token.
2. Wait up to 5 seconds for the engine to shut down.
3. If 5 seconds elapse, call `std::process::exit(1)`.

**Change from current**: The current 2-second grace period is increased to 5 seconds to allow for graceful Drop ordering. The 2-second timeout was too aggressive for tasks with pending I/O.

#### 7.3 `is_shutdown_requested()` API

```rust
pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_TOKEN.is_cancelled()
}
```

This replaces the current `shutdown_tx.try_send(())` pattern which is fire-and-forget and provides no feedback.

### 8. Broadcast Channel Deprecation

All existing uses of `broadcast::channel(1)` for shutdown signals SHALL be migrated to `CancellationToken` over time. This migration follows this priority:

| Priority | Component | Current Pattern | Target Pattern |
|----------|-----------|-----------------|----------------|
| 1 | SDK Runtime | `watch::channel` | `CancellationToken` |
| 2 | SpawnSupervisor | `broadcast::channel(1)` | `CancellationToken` |
| 3 | ReanimatorLoop | `broadcast::channel(1)` | `CancellationToken` |
| 4 | Background supervisors | `broadcast::channel(1)` | `CancellationToken` |
| 5 | SSE/WS handlers | `broadcast::channel(1)` | `CancellationToken` |
| 6 | Storage watchdog | `broadcast::channel(1)` | `CancellationToken` |

**Exception**: `broadcast::channel(N)` (where N > 1) for multi-consumer events is NOT deprecated. Only the one-shot shutdown pattern (`broadcast::channel(1)`) is deprecated.

### 9. Verification and Testing

#### 9.1 Cancellation Stress Tests

Every cancellation-propagating task MUST have a stress test that:
1. Spawns the task.
2. Cancels it at a random time during execution.
3. Verifies that all shared state is in a valid state after cancellation.
4. Verifies that no resources are leaked (FDs, memory, channels).

#### 9.2 Drop Order Tests

Every component with shared state and Drop implementations MUST have a test that:
1. Creates the component with multiple references.
2. Drops references in random order.
3. Verifies no panics, leaks, or inconsistent state.

#### 9.3 Orphan Detection

A CI check SHALL run `cargo clippy` with a custom lint that flags `tokio::spawn` without `CancellationToken` parameter in function signature. This is a compile-time check enforced via function signature convention.

#### 9.4 Mutation Testing

Cancellation safety code MUST pass mutation testing with >= 90% kill rate. Mutants include:
- Removing `token.cancelled()` from `tokio::select!`
- Removing child await from parent
- Adding infinite loop in Drop

## Consequences

### Positive

- **Data integrity**: No more silent state corruption from aborted async tasks
- **Predictable shutdown**: Shutdown ordering is deterministic and testable
- **Resource safety**: No leaked file descriptors, memory, or locks on cancellation
- **Debuggability**: Cancellation issues become compile-time errors (orphan detection) or test failures (stress tests), not production bugs
- **Structured concurrency**: Parent-child task relationships are explicit, making code easier to reason about

### Negative

- **Migration cost**: All existing `broadcast::channel(1)` shutdown patterns must be migrated
- **API surface**: `CancellationToken` must be threaded through many function signatures
- **Code volume**: Task groups, spawn wrappers, and stress tests increase code size
- **Complexity**: Structured concurrency adds ceremony to task spawning

### Neutral

- **Performance**: `CancellationToken` adds negligible overhead compared to `broadcast::Receiver`
- **tokio dependency**: No new dependencies; `CancellationToken` is in `tokio`

## References

- Tokio CancellationToken: https://docs.rs/tokio/latest/tokio/sync/struct.CancellationToken.html
- Tokio `select!` documentation: https://docs.rs/tokio/latest/tokio/macro.select.html
- Structured Concurrency (Howard and Swinnerton, 2018): https://www.microsoft.com/en-us/research/uploads/prod/2018/02/structured-concurrency.pdf
- ADR-005 (Hibernation and Timers): Reanimator shutdown
- ADR-018 (Pipe Deadlocks and I/O): IPC pipe cleanup
- ADR-019 (SIGTERM Races Handling): SDK SIGTERM thread
- ADR-046 (Async Process Supervisor Contract): SpawnSupervisor loop
