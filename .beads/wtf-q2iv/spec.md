# vo-q2iv — serve: Scaffold built-in worker in serve.rs

```yaml
bead_id: vo-q2iv
title: "serve: Scaffold built-in worker in serve.rs"
effort_estimate: "1hr"
status: planned
files:
  - crates/vo-cli/src/commands/serve.rs
  - crates/vo-worker/src/worker.rs
```

---

## Section 0 — Clarifications

| # | Question | Decision | Rationale |
|---|----------|----------|-----------|
| 0.1 | What activity types should the built-in worker handle? | None — empty handler map. This bead only scaffolds the spawn + shutdown wiring. | The built-in worker is a no-op dispatcher initially; real handlers are registered by plugins in future beads. |
| 0.2 | Should `Worker::new` take the raw `async_nats::Client` or the `jetstream::Context`? | `async_nats::jetstream::Context` (matches existing `Worker::new(js, ...)` signature). | Consistent with `vo-worker/src/worker.rs:102-113`. |
| 0.3 | What durable consumer name for the built-in worker? | `"builtin-worker"` — hardcoded constant in serve.rs. | Single-node default; overridable via config in a future bead. |
| 0.4 | Should worker failure crash the server? | No — log the error, continue draining. The `drain_runtime` function will `context()` the join error. | Matches existing pattern: `timer_task` failure is non-fatal to the join but logged. |
| 0.5 | Shutdown ordering? | Worker receives `shutdown_rx` clone same as api_task and timer_task. All three are awaited in `drain_runtime`. | Existing watch channel pattern — all tasks observe the same shutdown signal. |

---

## Section 1 — EARS Requirements (THE SYSTEM SHALL)

**1.1** WHEN `run_serve` is invoked, THE SYSTEM SHALL `tokio::spawn` a `Worker::run()` loop using the connected NATS JetStream context, a durable consumer name `"builtin-worker"`, no subject filter (`None`), and no registered handlers.

**1.2** THE SYSTEM SHALL pass a cloned `watch::Receiver<bool>` from the shared shutdown channel to the worker task so that it observes SIGINT/SIGTERM.

**1.3** THE SYSTEM SHALL await the worker `JoinHandle` inside `drain_runtime` alongside the existing API and timer tasks, propagating any join error via `anyhow::Context`.

**1.4** IF the worker task returns an error, THE SYSTEM SHALL log the error and return it from `run_serve` as part of the combined drain result (non-fatal to other tasks).

**1.5** THE SYSTEM SHALL NOT modify `vo-worker/src/worker.rs` — the `Worker` struct already provides the required `new()`, `register()`, and `run()` API.

---

## Section 2 — KIRK Contracts

### Contract 2.1: `run_serve` spawns built-in worker

```rust
// FILE: crates/vo-cli/src/commands/serve.rs

// PRE-CONDITION: nats is a connected async_nats::Client with JetStream
// PRE-CONDITION: shutdown_rx is a watch::Receiver<bool> clone (initial value false)
// INVARIANT: Worker is created with js = nats.jetstream().clone()
// INVARIANT: Worker durable name is "builtin-worker"
// INVARIANT: Worker filter_subject is None (consumes all wtf.work.*)

let js = nats.jetstream().clone();
let worker = vo_worker::Worker::new(js, "builtin-worker", None);
let worker_shutdown = shutdown_rx.clone();
let worker_task = tokio::spawn(async move {
    worker.run(worker_shutdown).await.map_err(|e| anyhow::anyhow!("builtin worker: {e}"))
});
```

### Contract 2.2: `drain_runtime` generic over 3 tasks

```rust
// PRE-CONDITION: All three JoinHandles are alive
// POST-CONDITION: shutdown_tx sends true
// POST-CONDITION: All three tasks are awaited (order: api, timer, worker)
// POST-CONDITION: stop_master() is called after tasks drain

async fn drain_runtime<EApi, ETimer, EWorker, FStop>(
    shutdown_tx: watch::Sender<bool>,
    api_task: JoinHandle<Result<(), EApi>>,
    timer_task: JoinHandle<Result<(), ETimer>>,
    worker_task: JoinHandle<Result<(), EWorker>>,
    stop_master: FStop,
) -> anyhow::Result<()>
where
    EApi: std::error::Error + Send + Sync + 'static,
    ETimer: std::error::Error + Send + Sync + 'static,
    EWorker: std::error::Error + Send + Sync + 'static,
    FStop: FnOnce(),
```

### Contract 2.3: `Worker` SDK (unchanged — reference only)

```rust
// FILE: crates/vo-worker/src/worker.rs
// Worker::new(js: Context, worker_name: impl Into<String>, filter_subject: Option<String>) -> Worker
// Worker::register(&mut self, activity_type: impl Into<String>, handler: F)
// Worker::run(&self, shutdown_rx: tokio::sync::watch::Receiver<bool>) -> Result<(), VoError>
```

---

## Section 2.5 — Research

| Topic | Finding | Relevance |
|-------|---------|-----------|
| Existing spawn pattern in serve.rs | Lines 80-85: `tokio::spawn` wraps `serve_api()` and `run_timer_loop()`. Both take a `watch::Receiver<bool>` clone. | Worker spawn follows identical pattern. |
| `Worker::run` signature | `pub async fn run(&self, shutdown_rx: tokio::sync::watch::Receiver<bool>) -> Result<(), VoError>` — takes `&self` so Worker must be moved into the spawn closure. | Worker is moved into the `async move` block. |
| `drain_runtime` generic bounds | Lines 93-115: generic over `EApi`, `ETimer`, error types, plus a `FStop` closure. | Must extend to 3 type params (`EWorker`) and 4th argument. |
| `VoError` impl | `vo_common::VoError` — implements `std::error::Error + Send + Sync + 'static` | Satisfies the `drain_runtime` generic bound for `EWorker`. |
| Existing test | `drain_runtime_signals_shutdown_and_waits_for_tasks` (line 160) — uses 2 task handles. | Must extend to 3 task handles. |
| `vo-cli` already depends on `vo-worker` | Cargo.toml line 12: `vo-worker = { path = "../vo-worker" }` | No Cargo.toml changes needed. |
| `vo-worker::Worker` re-exported | `lib.rs` line 22: `pub use worker::Worker;` | Import path is `vo_worker::Worker`. |

---

## Section 3 — Inversions (Control Flow)

```
main()
  └─► run_serve(config)
        ├─► connect NATS
        ├─► provision storage
        ├─► spawn MasterOrchestrator
        ├─► build_app(master, kv)
        ├─► watch::channel(false) → (shutdown_tx, shutdown_rx)
        ├─► tokio::spawn(serve_api(..., api_shutdown))      [existing]
        ├─► tokio::spawn(run_timer_loop(..., timer_shutdown)) [existing]
        ├─► Worker::new(js, "builtin-worker", None)          [NEW]
        ├─► tokio::spawn(worker.run(worker_shutdown))         [NEW]
        ├─► wait_for_shutdown_signal()                        [existing]
        └─► drain_runtime(tx, api, timer, worker, || stop)   [MODIFIED: +worker arg]
              ├─► shutdown_tx.send(true)
              ├─► api_task.await
              ├─► timer_task.await
              ├─► worker_task.await                            [NEW]
              └─► stop_master()
```

---

## Section 4 — ATDD Tests (Unit)

### Test 4.1: `drain_runtime` signals shutdown and waits for all three tasks

```rust
// FILE: crates/vo-cli/src/commands/serve.rs (mod tests)

#[tokio::test]
async fn drain_runtime_signals_shutdown_and_waits_for_three_tasks() {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let api_drained = Arc::new(AtomicBool::new(false));
    let timer_drained = Arc::new(AtomicBool::new(false));
    let worker_drained = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));

    let api_handle = {
        let mut rx = shutdown_rx.clone();
        let drained = Arc::clone(&api_drained);
        tokio::spawn(async move {
            let changed = rx.changed().await;
            if changed.is_ok() { drained.store(true, Ordering::SeqCst); }
            Result::<(), std::io::Error>::Ok(())
        })
    };

    let timer_handle = {
        let mut rx = shutdown_rx.clone();
        let drained = Arc::clone(&timer_drained);
        tokio::spawn(async move {
            let changed = rx.changed().await;
            if changed.is_ok() { drained.store(true, Ordering::SeqCst); }
            Result::<(), std::io::Error>::Ok(())
        })
    };

    let worker_handle = {
        let mut rx = shutdown_rx;
        let drained = Arc::clone(&worker_drained);
        tokio::spawn(async move {
            let changed = rx.changed().await;
            if changed.is_ok() { drained.store(true, Ordering::SeqCst); }
            Result::<(), std::io::Error>::Ok(())
        })
    };

    let drain_result = drain_runtime(shutdown_tx, api_handle, timer_handle, worker_handle, {
        let stopped = Arc::clone(&stopped);
        move || { stopped.store(true, Ordering::SeqCst); }
    }).await;

    assert!(drain_result.is_ok());
    assert!(api_drained.load(Ordering::SeqCst));
    assert!(timer_drained.load(Ordering::SeqCst));
    assert!(worker_drained.load(Ordering::SeqCst));
    assert!(stopped.load(Ordering::SeqCst));
}
```

### Test 4.2: `drain_runtime` propagates worker error

```rust
#[tokio::test]
async fn drain_runtime_propagates_worker_error() {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let rx1 = shutdown_rx.clone();
    let rx2 = shutdown_rx.clone();
    let rx3 = shutdown_rx;

    let api = tokio::spawn(async move { rx1.changed().await; Ok::<(), std::io::Error>(()) });
    let timer = tokio::spawn(async move { rx2.changed().await; Ok::<(), std::io::Error>(()) });
    let worker = tokio::spawn(async move {
        rx3.changed().await;
        Err::<(), std::io::Error>(std::io::Error::new(std::io::ErrorKind::Other, "worker boom"))
    });

    let result = drain_runtime(shutdown_tx, api, timer, worker, || {}).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("worker boom"), "expected 'worker boom', got: {err_msg}");
}
```

---

## Section 5 — E2E Tests

### Test 5.1: `run_serve` starts and stops with worker (requires NATS)

```rust
// FILE: crates/vo-cli/tests/serve_worker_integration.rs
// Requires: NATS running (vo-nats-test container)

#[tokio::test]
async fn serve_starts_builtin_worker_and_shutdown_drains_it() {
    use vo_cli::commands::serve::ServeConfig;
    use std::path::PathBuf;
    use tokio::time::{timeout, Duration};

    let tmp = tempfile::tempdir().unwrap();
    let config = ServeConfig {
        port: 0, // will fail to bind — that's fine, we test spawn
        nats_url: "nats://127.0.0.1:4222".to_owned(),
        embedded_nats: false,
        data_dir: tmp.path().to_path_buf(),
        max_concurrent: 1,
    };

    let handle = tokio::spawn(async move { vo_cli::commands::serve::run_serve(config).await });

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send SIGINT equivalent — in test, verify the worker was created
    // by checking NATS consumer exists
    let client = async_nats::connect("nats://127.0.0.1:4222").await.unwrap();
    let js = client.jetstream();
    let consumer = js.get_consumer("vo-work", "builtin-worker").await;
    assert!(consumer.is_ok(), "builtin-worker consumer should exist");

    drop(client);
    handle.abort();
    let _ = handle.await;
}
```

---

## Section 5.5 — Verification

| Gate | Command | Expected |
|------|---------|----------|
| Compile | `cargo check -p vo-cli` | No errors |
| Unit tests | `cargo test -p vo-cli` | All pass |
| Clippy | `cargo clippy -p vo-cli -- -D warnings` | No warnings |
| Integration | `cargo test --workspace` (requires NATS) | No regressions |

---

## Section 6 — Implementation Tasks

| # | Task | File | LOC est. |
|---|------|------|----------|
| 6.1 | Add `use vo_worker::Worker;` import to serve.rs | `serve.rs:14` | 1 |
| 6.2 | Create `Worker::new(nats.jetstream().clone(), "builtin-worker", None)` after timer_task spawn | `serve.rs:85-88` | 3 |
| 6.3 | Clone `shutdown_rx` for worker: `let worker_shutdown = shutdown_rx.clone();` | `serve.rs:78` | 1 |
| 6.4 | `tokio::spawn` the worker: `let worker_task = tokio::spawn(async move { worker.run(worker_shutdown).await.map_err(\|e\| anyhow::anyhow!("builtin worker: {e}")) });` | `serve.rs:89-93` | 5 |
| 6.5 | Extend `drain_runtime` signature: add `EWorker` type param + `worker_task: JoinHandle<Result<(), EWorker>>` argument | `serve.rs:93-115` | 4 |
| 6.6 | Inside `drain_runtime`: add `let worker_result: Result<(), EWorker> = worker_task.await.context("worker task join failed")?;` after timer_result | `serve.rs:108` | 2 |
| 6.7 | Add `worker_result.context("builtin worker failed")?;` after timer_result check | `serve.rs:112` | 1 |
| 6.8 | Update call site: `drain_runtime(shutdown_tx, api_task, timer_task, worker_task, \|\| master.stop(None))` | `serve.rs:88` | 1 |
| 6.9 | Update existing 2-task unit test to 3-task version (add `worker_drained` handle) | `serve.rs:160-202` | 15 |
| 6.10 | Add `drain_runtime_propagates_worker_error` test | `serve.rs` (mod tests) | 20 |

**Total estimated diff:** ~50 lines added, ~10 lines modified.

---

## Section 7 — Failure Modes

| # | Failure | Detection | Mitigation |
|---|---------|-----------|------------|
| 7.1 | `Worker::new` panics (clippy deny) | N/A — `Worker::new` is infallible, returns `Self` | Not possible. |
| 7.2 | `Worker::run` fails to create JetStream consumer (stream not provisioned) | Returns `Err(VoError::NatsPublish(...))` | `drain_runtime` propagates via `context("worker task join failed")`. |
| 7.3 | `Worker::run` panics inside spawn | `JoinHandle` returns `Err(JoinError)` | `drain_runtime` `context("worker task join failed")` catches it. |
| 7.4 | Shutdown signal race — worker sees shutdown before consumer is created | Worker breaks out of `run` immediately | Safe — `ShutdownResult` with zero counts. |
| 7.5 | NATS connection drops mid-worker-run | `WorkQueueConsumer::next_task` returns `Err(VoError)` | Worker logs error, continues loop. |

---

## Section 7.5 — Anti-Hallucination

| Claim | Verification |
|-------|-------------|
| `vo_worker::Worker` is already a dependency of `vo-cli` | `crates/vo-cli/Cargo.toml:12` — confirmed |
| `Worker::new` takes `Context`, `impl Into<String>`, `Option<String>` | `crates/vo-worker/src/worker.rs:102-113` — confirmed |
| `Worker::run` takes `watch::Receiver<bool>`, returns `Result<(), VoError>` | `crates/vo-worker/src/worker.rs:142-149` — confirmed |
| `VoError` implements `Error + Send + Sync + 'static` | `crates/vo-common/src/lib.rs` — derives `thiserror::Error` (implicitly `std::error::Error`) |
| `drain_runtime` currently takes 2 tasks | `crates/vo-cli/src/commands/serve.rs:93-115` — confirmed |
| `drain_runtime` test uses 2 handles | `crates/vo-cli/src/commands/serve.rs:160-202` — confirmed |
| No Cargo.toml changes needed | `vo-worker` already in `vo-cli` dependencies |

---

## Section 7.6 — Context Survival

If this bead is interrupted and resumed mid-implementation:

| Step | What to verify | How |
|------|---------------|-----|
| Import added | `use vo_worker::Worker;` exists at top of serve.rs | `rg "use vo_worker" crates/vo-cli/src/commands/serve.rs` |
| Worker spawned | `worker_task` variable exists and is passed to `drain_runtime` | `rg "worker_task" crates/vo-cli/src/commands/serve.rs` |
| `drain_runtime` extended | Function signature has 4 generic params (`EApi, ETimer, EWorker`) | `rg "EWorker" crates/vo-cli/src/commands/serve.rs` |
| Tests updated | 3-task drain test exists | `rg "worker_drained" crates/vo-cli/src/commands/serve.rs` |

---

## Section 8 — Completion

```yaml
done_when:
  - cargo check -p vo-cli succeeds
  - cargo test -p vo-cli passes (including new 3-task drain tests)
  - cargo clippy -p vo-cli -- -D warnings passes
  - crates/vo-worker/src/worker.rs is UNCHANGED (git diff shows no modifications)
  - drain_runtime accepts 3 JoinHandles
  - run_serve spawns a Worker with name "builtin-worker" and no filter
```

---

## Section 9 — Context

```yaml
domain: "Durable workflow execution engine"
architecture: "Ractor actors + NATS JetStream event sourcing + Dioxus WASM frontend"
runtime: "tokio async, Rust edition 2021"
key_types:
  - "vo_worker::Worker — high-level activity dispatch loop"
  - "vo_common::VoError — domain error type (thiserror)"
  - "async_nats::jetstream::Context — JetStream context"
  - "tokio::sync::watch::Receiver<bool> — shutdown signal channel"
key_modules:
  - "crates/vo-cli/src/commands/serve.rs — server entrypoint"
  - "crates/vo-worker/src/worker.rs — Worker SDK (DO NOT MODIFY)"
  - "crates/vo-worker/src/lib.rs — re-exports Worker"
constraints:
  - "No changes to vo-worker crate"
  - "No new Cargo.toml dependencies"
  - "No unsafe code"
  - "clippy::unwrap_used + clippy::expect_used + clippy::panic denied"
```

---

## Section 10 — AI Hints

1. **Pattern match exactly** — the worker spawn should mirror the timer_task spawn on lines 81-85 of serve.rs. Same `tokio::spawn(async move { ... })` shape, same `shutdown_rx.clone()` pattern.

2. **`drain_runtime` modification** — add `EWorker` as a third type parameter, add `worker_task` as a fourth function argument, await it after `timer_task`, and add `worker_result.context(...)`. Do NOT reorder the existing awaits.

3. **Test update strategy** — the existing `drain_runtime_signals_shutdown_and_waits_for_tasks` test creates 2 `AtomicBool` + 2 handles. Add a third `worker_drained` bool and third handle following the exact same pattern. Rename the test to include "three" in the name.

4. **No handler registration** — `Worker::register` is NOT called in this bead. The built-in worker starts with an empty handler map. Unregistered activity types are acked and logged as warnings (see `worker.rs:214-222`).

5. **`Worker::run` takes `&self`** — the `Worker` value must be moved into the `async move` closure, not borrowed. Since `run` takes `&self`, the moved `Worker` is accessed by reference inside the closure.

6. **Error type bridging** — `VoError` is not `anyhow::Error`. The spawn closure must `.map_err(|e| anyhow::anyhow!("builtin worker: {e}"))` to convert `VoError` → `anyhow::Error` for the `JoinHandle<Result<(), anyhow::Error>>` that `drain_runtime` expects.
