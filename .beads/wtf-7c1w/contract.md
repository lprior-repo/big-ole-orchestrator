# Contract Specification

## Context
- **Feature**: Harden serve run-loop runtime correctness — explicit drain path for graceful shutdown
- **Bead ID**: wtf-7c1w
- **Phase**: contract-synthesis
- **Updated**: 2026-03-22T00:00:00Z

## Domain Terms
- `drain_runtime` — async function coordinating shutdown signal fan-out and task draining
- `shutdown_tx` — `tokio::sync::watch::Sender<bool>` broadcasting shutdown signal
- `api_task` — `JoinHandle<Result<(), EApi>>` wrapping the API server task
- `timer_task` — `JoinHandle<Result<(), ETimer>>` wrapping the timer loop task
- `stop_master` — `FnOnce()` closure to halt the MasterOrchestrator
- `wait_for_shutdown_signal` — blocks until SIGTERM or SIGINT received

## Assumptions
- SIGTERM/SIGINT is the sole shutdown trigger (unix signal handling)
- Both api_task and timer_task are independent; neither blocks the other's drain
- stop_master is idempotent-safe to call once only
- watch channel has multiple receivers (api_task, timer_task) that must all observe shutdown

## Open Questions
- None

---

## Preconditions

| # | Precondition | Enforcement Level | Type/Pattern |
|---|---|---|---|
| P1 | `shutdown_tx` is not closed (can send) | Compile-time via type | `watch::Sender<bool>` is always sendable until `drop`ped |
| P2 | `api_task` and `timer_task` are not yet completed | Runtime-checked | `JoinHandle` is `Pending` until awaited; joining after completion yields the result |
| P3 | `stop_master` has not been called yet | Compile-time via `FnOnce()` | `FnOnce` can only be invoked once; compiler enforces |

---

## Postconditions

| # | Postcondition | Enforcement | Violation Example |
|---|---|---|---|
| Q1 | `shutdown_tx.send(true)` returns `Ok(())` — signal broadcast to all receivers | Runtime | If channel closed early: `Err(SendError(false))` |
| Q2 | `api_task.await` resolves — API server task is drained | Runtime | `Err(JoinError)` if task panicked or was cancelled |
| Q3 | `timer_task.await` resolves — timer loop is drained | Runtime | `Err(JoinError)` if task panicked or was cancelled |
| Q4 | `stop_master()` is called exactly once | Compile-time via `FnOnce` | `FnOnce` cannot be called twice — compile error |
| Q5 | `drain_runtime` returns `Ok(())` iff both tasks returned `Ok(())` | Runtime | If either task returns `Err(...)`, final `context()` wraps it as `Err(...)` |
| Q6 | `api_drained` flag is set to `true` after `api_task` completes | Test invariant | N/A — test only |

---

## Invariants

| # | Invariant | Enforcement |
|---|---|---|
| I1 | Orchestrator and tasks are running until `drain_runtime` is called | Static — caller ensures `wait_for_shutdown_signal()` completes first |
| I2 | After `drain_runtime` returns `Ok(())`, all spawned tasks are guaranteed terminated | Dynamic — `await` on both `JoinHandle`s before returning |
| I3 | `shutdown_tx` is `drop`ped only after all receivers have observed the signal | Dynamic — receivers cloned from `shutdown_rx` before spawning tasks |

---

## Error Taxonomy

| Variant | Trigger | Context |
|---|---|---|
| `Error::ApiTaskFailed(EApi)` | `api_task.await` returns `Err(EApi)` | Wrapped via `context("api server failed")` |
| `Error::TimerTaskFailed(ETimer)` | `timer_task.await` returns `Err(ETimer)` | Wrapped via `context("timer loop failed")` |
| `Error::ApiTaskJoinFailed` | `api_task.await` panics or is cancelled | Wrapped via `context("api task join failed")` |
| `Error::TimerTaskJoinFailed` | `timer_task.await` panics or is cancelled | Wrapped via `context("timer task join failed")` |
| `Error::ShutdownSendFailed` | `shutdown_tx.send(true)` on closed channel | Propagated as `Err(SendError)` |

Note: `anyhow::Error` is used in the implementation; the above taxonomy describes the semantic categories.

---

## Contract Signatures

```rust
/// Drains the runtime: signals shutdown, awaits tasks, stops orchestrator.
/// Returns Ok(()) only if all tasks completed successfully.
/// 
/// # Errors
/// Returns error if any task join or task result is Err.
async fn drain_runtime<EApi, ETimer, FStop>(
    shutdown_tx: watch::Sender<bool>,
    api_task: JoinHandle<Result<(), EApi>>,
    timer_task: JoinHandle<Result<(), ETimer>>,
    stop_master: FStop,
) -> anyhow::Result<()>
where
    EApi: std::error::Error + Send + Sync + 'static,
    ETimer: std::error::Error + Send + Sync + 'static,
    FStop: FnOnce(),
```

---

## Type Encoding

| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| P1: shutdown_tx not closed | Compile-time | `watch::Sender<bool>` — cannot send if dropped |
| P2: tasks not completed | Runtime | `JoinHandle` join semantics |
| P3: stop_master callable once | Compile-time | `FnOnce()` — once-only enforced by type |

---

## Violation Examples (REQUIRED)

### Precondition Violations

- **VIOLATES P1**: `drop(shutdown_tx)` before calling `drain_runtime`
  - Call: `drain_runtime(dropped_sender, api_task, timer_task, stop_master)`
  - Expected: `Err(anyhow::Error)` containing `SendError`

- **VIOLATES P2**: Pass an already-completed `JoinHandle`
  - Call: `drain_runtime(shutdown_tx, futures::future::ready(Ok(())).await, timer_task, stop_master)`
  - Expected: `Ok(())` if other task succeeds, but `drain_runtime` should not be called with pre-completed tasks — caller responsibility

- **VIOLATES P3**: `drain_runtime` panics if `stop_master` is called twice via `FnOnce`
  - Call: `drain_runtime(shutdown_tx, api_task, timer_task, duplicate_stop_fn)`
  - Expected: Panic (Rust `FnOnce` enforcement) — not a Result

### Postcondition Violations

- **VIOLATES Q1**: `shutdown_tx.send(true)` returns `Err` when channel closed
  - State: `shutdown_tx` dropped before send
  - Expected: `Err(SendError)` propagated as part of the `anyhow::Result`

- **VIOLATES Q2**: `api_task.await` returns `Err` when API server errored
  - State: `api_task` resolves to `Err(EApi)`
  - Expected: `Err(anyhow::Error)` wrapping API error via `context("api server failed")`

- **VIOLATES Q5**: `drain_runtime` returns `Err` when either task fails
  - State: `api_task` returns `Ok(())`, `timer_task` returns `Err(timer_err)`
  - Expected: `Err(anyhow::Error)` with `context("timer loop failed")`

---

## Ownership Contracts

| Parameter | Mode | Contract |
|---|---|---|
| `shutdown_tx` | Ownership transfer | Caller retains ownership of `shutdown_tx`; `drain_runtime` consumes it; channel is closed on drop |
| `api_task` | Ownership transfer | `drain_runtime` takes `JoinHandle` ownership; awaits and consumes it |
| `timer_task` | Ownership transfer | `drain_runtime` takes `JoinHandle` ownership; awaits and consumes it |
| `stop_master` | Ownership transfer | `FnOnce` consumed on call; cannot be called again |

---

## Non-goals
- Handling shutdown signals other than SIGTERM/SIGINT
- Force-killing tasks that do not drain within a timeout (graceful drain only)
- Restarting tasks after drain
