# QA Report — vo-q2iv: "serve: Scaffold built-in worker"

**Date:** 2026-03-23
**Reviewer:** opencode (automated)
**Verdict:** APPROVED

---

## 1. QA Checklist

### 1.1 Worker::new spawn verified
**PASS.** `serve.rs:102-105`:
```rust
let worker = Worker::new(nats.jetstream().clone(), "builtin-worker", None);
let worker_task = tokio::spawn(async move {
    worker.run(worker_shutdown).await
});
```
- Uses correct name `"builtin-worker"` and `None` filter (consume all activity types).
- Spawns as a `tokio::task::JoinHandle` matching the contract.

### 1.2 No unwrap/expect in production code
**PASS.** Grep for `unwrap|expect(` in serve.rs returned zero matches. All errors use `?` with `.context()`.

### 1.3 Test execution
**PASS.** Both runs succeed identically:
```
running 2 tests
test serve::tests::drain_runtime_signals_shutdown_and_waits_for_four_tasks ... ok
test serve::tests::drain_runtime_propagates_worker_error ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out
```

### 1.4 Line count
**PASS.** `serve.rs` = **230 lines** (limit: 300). Test module extracted to `serve_tests.rs` (127 lines).

---

## 2. Red Queen Review

### 2.1 What happens if Worker::new fails?
**N/A — not applicable.** `Worker::new` is infallible (returns `Self`, not `Result`). It only stores fields — no I/O, no allocation that can fail. This is correct design.

### 2.2 Drain order — is worker awaited correctly?
**PASS.** `drain_runtime` (serve.rs:161-192) signature now accepts 4 `JoinHandle` args:
```rust
async fn drain_runtime<EApi, ETimer, EWorker, FStop>(
    shutdown_tx, api_task, timer_task, heartbeat_task, worker_task, stop_master,
) -> anyhow::Result<()>
```
Drain sequence:
1. `shutdown_tx.send(true)` — signals all tasks
2. `api_task.await` then `timer_task.await` then `heartbeat_task.await` then `worker_task.await` — sequential join
3. `stop_master()` — stops orchestrator last

Worker is awaited *before* master stop, which is correct — ensures in-flight activities drain before orchestrator shuts down. Error propagation for worker: `worker_result.context("builtin worker failed")?` — tested in `drain_runtime_propagates_worker_error`.

### 2.3 Test isolation
**PASS.** Two consecutive `cargo test -p vo-cli -- serve` runs both pass. No shared state leaks.

### 2.4 Clippy strict
**PASS (for this crate's code).** `cargo clippy -p vo-cli -- -D warnings` fails, but all 4 errors are in `vo-common` (pre-existing `missing_errors_doc` on `to_msgpack`/`from_msgpack`/`try_new`). Zero clippy issues originate from serve.rs or serve_tests.rs.

---

## 3. Black Hat Review

### 3.1 Worker::new signature verified
**PASS.** `worker.rs:102-113`:
```rust
pub fn new(
    js: Context,
    worker_name: impl Into<String>,
    filter_subject: Option<String>,
) -> Self
```
Exact signature match with call site `Worker::new(nats.jetstream().clone(), "builtin-worker", None)`.

### 3.2 Hallucinated APIs?
**NONE FOUND.** All imports resolve:
- `vo_worker::Worker` — re-exported from `crates/vo-worker/src/lib.rs:22`
- `worker.run(shutdown_rx)` — method `run(&self, shutdown_rx: watch::Receiver<bool>) -> Result<(), VoError>` at worker.rs:142-149
- `tokio::sync::watch::Receiver<bool>` — shutdown_rx type matches `run()` parameter
- `JoinHandle<Result<(), EWorker>>` — `VoError` implements `std::error::Error + Send + Sync + 'static`, satisfying the trait bounds in `drain_runtime`

### 3.3 Import correctness
**PASS.** Line 18: `use vo_worker::Worker;` — clean, minimal import.

---

## Summary

| Check | Result |
|-------|--------|
| Worker::new spawn | PASS |
| No unwrap/expect | PASS |
| Tests (2/2 pass) | PASS |
| Line count (230/300) | PASS |
| Worker::new infallible | N/A (correct) |
| Drain order | PASS |
| Test isolation | PASS |
| Clippy (no new issues) | PASS |
| Signature match | PASS |
| No hallucinated APIs | PASS |
| Import correctness | PASS |

**Verdict: APPROVED** — Implementation matches contract exactly. Clean shutdown drain, proper error propagation, no hallucinated APIs, tests cover both happy path and worker failure.
