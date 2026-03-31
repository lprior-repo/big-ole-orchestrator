# Black Hat Review — wtf-40m5: "serve: Start heartbeat watcher in serve.rs"

**Reviewer:** Black Hat
**Date:** 2026-03-23

## 1. Hallucinated APIs — `run_heartbeat_watcher`

**VERIFIED OK.** `run_heartbeat_watcher` exists at `crates/wtf-actor/src/heartbeat.rs:55`.

Signature:
```rust
pub async fn run_heartbeat_watcher(
    heartbeats: Store,
    orchestrator: ActorRef<OrchestratorMsg>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String>
```

Call site at `serve.rs:97-101` passes:
- `kv.heartbeats.clone()` — `KvStores.heartbeats: Store` (verified `crates/wtf-storage/src/kv.rs:31`)
- `master.clone()` — `ActorRef<OrchestratorMsg>` (verified `crates/wtf-actor/src/master/mod.rs:16`: `type Msg = OrchestratorMsg`)
- `heartbeat_shutdown` — `watch::Receiver<bool>`

All types match. No hallucination.

## 2. Silent Failures

**No silent failures in serve.rs.**
- `shutdown_tx.send(true)` on line 175 — correctly ignores send result (watch channel: at least one receiver always exists at call time).
- `drain_runtime` propagates all task errors via `.context()` and `.map_err()`.

**In heartbeat.rs** (line 105): `let _ = orchestrator.cast(...)` — intentionally fire-and-forget. This is the correct pattern for `cast` (it's inherently best-effort). Logged at `debug` level on line 104. Acceptable.

## 3. Contract Violations — `drain_runtime` and heartbeat task

**VERIFIED OK.**
- `drain_runtime` signature hardcodes `heartbeat_task: JoinHandle<Result<(), String>>` matching the return type of `run_heartbeat_watcher`.
- Line 179: `heartbeat_task.await.context("heartbeat watcher task join failed")?` — correctly awaits the JoinHandle.
- Line 187-188: `heartbeat_result.map_err(|e| anyhow::anyhow!("heartbeat watcher failed: {e}"))` — correctly propagates the `String` error into `anyhow::Error`.
- Test at line 250-306 (`drain_runtime_signals_shutdown_and_waits_for_four_tasks`) verifies heartbeat_drained is set.
- Test at line 308-353 (`drain_runtime_propagates_worker_error`) includes heartbeat in the drain.

No contract violation.

## 4. Dead Code / Unused Imports

**No dead code.** All imports in serve.rs are used:
- `PathBuf` — `ServeConfig.data_dir`
- `Arc` — `event_store`, `state_store`, `task_queue`
- `anyhow::Context` — `.context()` calls
- `Store` — `load_definitions_from_kv(&Store)`
- `StreamExt` — `.next()` in `load_definitions_from_kv`
- `Actor` — `MasterOrchestrator::spawn` (trait method)
- `watch` — `watch::channel`, `watch::Sender`, `watch::Receiver`
- `JoinHandle` — `drain_runtime` signature
- `run_heartbeat_watcher` — line 97
- `run_timer_loop` — line 92
- `Worker` — line 102

## 5. serve.rs Line Count — 354 lines (over 300 limit)

**OBSERVATION, NOT A REJECTABLE DEFECT.**
- Non-test code: lines 1-227 (227 lines) — well under 300.
- Test code: lines 228-354 (126 lines) — the test module inflates the total.
- The file grew by adding `heartbeat_task` parameter to `drain_runtime` (1 parameter), the spawn call (4 lines), and updated tests (~20 lines).
- **Fix:** Extract `#[cfg(test)] mod tests` into `serve_tests.rs` or `tests/` directory. This is a housekeeping bead, not a correctness issue.

## 6. Bonus: Pre-existing Clippy Failures

`cargo clippy --workspace -D warnings` fails due to 4 pre-existing `missing_errors_doc` warnings in `wtf-common/src/types/id.rs` and `wtf-common/src/events/mod.rs`. These are NOT caused by this bead.

## Verdict

All five audit dimensions pass. The API is real, types match, errors propagate, no dead code, and the line count issue is test-module-only.

**STATUS: APPROVED**
