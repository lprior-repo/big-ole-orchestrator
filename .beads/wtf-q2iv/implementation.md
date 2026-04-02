# Implementation Summary — wtf-q2iv

```yaml
bead_id: wtf-q2iv
bead_title: "serve: Scaffold built-in worker"
phase: STATE-3
updated_at: "2026-03-23T00:00:00Z"
```

## Files Modified

| File | Change |
|------|--------|
| `crates/wtf-cli/src/commands/serve.rs` | Added Worker spawn + extended drain_runtime to 4 tasks + 2 new tests |

## Files Unchanged (Verified)

- `crates/wtf-worker/src/worker.rs` — NOT modified (spec constraint Section 1.5)
- `crates/wtf-worker/src/lib.rs` — NOT modified
- `crates/wtf-cli/Cargo.toml` — NOT modified (wtf-worker already a dependency)

## Implementation Details

### 1. Import Added
```rust
use wtf_worker::Worker;
```

### 2. Worker Spawn in `run_serve`
- Created `Worker::new(nats.jetstream().clone(), "builtin-worker", None)` — no handlers, no filter subject
- Spawned with `tokio::spawn(async move { worker.run(worker_shutdown).await })`
- Worker receives a `watch::Receiver<bool>` clone from the shared shutdown channel
- Error type is `Result<(), WtfError>` which satisfies the `drain_runtime` generic bound (`WtfError: Error + Send + Sync + 'static`)
- No `map_err` to `anyhow::Error` needed — avoids the `anyhow::Error: !Error` trait bound issue

### 3. `drain_runtime` Extended
- Added `EWorker` type parameter: `EWorker: std::error::Error + Send + Sync + 'static`
- Added `worker_task: JoinHandle<Result<(), EWorker>>` as 4th argument
- Awaits worker after heartbeat: `worker_task.await.context("worker task join failed")?`
- Propagates: `worker_result.context("builtin worker failed")?`

### 4. Adaptation from Spec
The spec assumed 2 tasks (api + timer). The actual codebase already had 3 (api + timer + heartbeat from wtf-40m5). The implementation correctly extended from 3 to 4 tasks rather than 2 to 3.

## Tests Written

| Test | Status | Description |
|------|--------|-------------|
| `drain_runtime_signals_shutdown_and_waits_for_four_tasks` | ✅ PASS | Verifies all 4 tasks (api, timer, heartbeat, worker) receive shutdown signal and drain. Extracted `make_drained_task` helper to reduce duplication. |
| `drain_runtime_propagates_worker_error` | ✅ PASS | Verifies that a `worker_task` returning `Err(io::Error("worker boom"))` is propagated through the error chain. Checks both the context message (`"builtin worker failed"`) and the source chain (`"worker boom"`). |

## Constraint Adherence

| Constraint | Status | Evidence |
|------------|--------|----------|
| Zero `unwrap()`/`expect()` in source | ✅ | No unwrap/expect in serve.rs source code |
| `wtf-worker` crate unchanged | ✅ | `git diff` confirms no modifications |
| No new Cargo.toml dependencies | ✅ | wtf-worker already in wtf-cli deps |
| No unsafe code | ✅ | No `unsafe` blocks |
| Data→Calc→Actions | ✅ | Worker construction is Data, drain is Actions (I/O) |
| `mut` banned in source | ✅ | Only `mut` in test code (allowed per functional-rust skill) |

## Verification Output

```
$ cargo check -p wtf-cli
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test -p wtf-cli
    Running unittests src/lib.rs
    running 10 tests
    test serve::tests::drain_runtime_signals_shutdown_and_waits_for_four_tasks ... ok
    test serve::tests::drain_runtime_propagates_worker_error ... ok
    (8 other tests) ... ok
    test result: ok. 10 passed; 0 failed

$ cargo test -p wtf-worker
    running 37 tests (unit) ... ok
    running 19 tests (integration) ... ok
    test result: ok. 56 passed; 0 failed

$ cargo clippy -p wtf-cli -p wtf-worker
    Zero errors in wtf-cli and wtf-worker source.
    All warnings are pre-existing in dependency crates (wtf-common, wtf-storage, wtf-actor, wtf-linter).
```
