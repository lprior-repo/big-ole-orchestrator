# Bead Spec: vo-40m5

**Title:** serve: Start heartbeat watcher in serve.rs
**Effort:** 15min
**Priority:** 1
**Type:** task

---

## Section 0: Clarifications

- None. The function `run_heartbeat_watcher` already exists at `vo_actor::heartbeat::run_heartbeat_watcher` (line 55 of `crates/vo-actor/src/heartbeat.rs`). The `serve.rs` already spawns `api_task` and `timer_task` with the same shutdown pattern. This bead wires the heartbeat watcher into the serve startup in an identical fashion.

---

## Section 1: EARS Requirements

- **WHEN** `run_serve` is called, **THE SYSTEM SHALL** spawn a heartbeat watcher task via `tokio::spawn` that calls `vo_actor::heartbeat::run_heartbeat_watcher(kv.heartbeats.clone(), master.clone(), shutdown_rx)`.
- **THE SYSTEM SHALL** pass a cloned `shutdown_rx` (from the existing `watch::channel`) to the heartbeat watcher so it shuts down with the rest of the server.
- **WHEN** the heartbeat watcher task panics or returns `Err`, **THE SYSTEM SHALL** propagate the error during `drain_runtime`.

---

## Section 2: KIRK Contracts

### Pre-conditions
- `kv.heartbeats: Store` is a valid NATS KV handle (returned by `provision_kv_buckets` which runs before the spawn site).
- `master: ActorRef<OrchestratorMsg>` is the live `MasterOrchestrator` actor ref (spawned before the spawn site).
- `shutdown_rx: watch::Receiver<bool>` has not yet fired.

### Post-conditions
- `run_heartbeat_watcher` is running as a Tokio background task.
- The task calls `heartbeats.watch_all()` and begins listening for TTL-expired entries.
- On shutdown signal, the task breaks its loop and returns `Ok(())`.

### Invariants
- The heartbeat watcher uses the same `shutdown_rx` clone as `timer_task` — all three background tasks (api, timer, heartbeat) observe the same shutdown signal.
- The `master` actor ref passed to the watcher is the same `master` used by `build_app`.

---

## Section 2.5: Research Requirements

None. All types and functions are already implemented and tested:
- `vo_actor::heartbeat::run_heartbeat_watcher(Store, ActorRef<OrchestratorMsg>, watch::Receiver<bool>) -> Result<(), String>` — `crates/vo-actor/src/heartbeat.rs:55`
- `kv.heartbeats: Store` — `crates/vo-storage/src/kv.rs:31`
- `master: ActorRef<OrchestratorMsg>` — returned by `MasterOrchestrator::spawn()` at `crates/vo-cli/src/commands/serve.rs:65`

---

## Section 3: Inversions

None. No new traits, no DI containers, no strategy pattern. Direct function call spawned on Tokio.

---

## Section 4: ATDD Acceptance Tests

### Test 1: Compilation
```bash
cargo check -p vo-cli
```
Must succeed with zero errors.

### Test 2: Clippy
```bash
cargo clippy -p vo-cli -- -D warnings
```
Must pass.

### Test 3: Existing tests still pass
```bash
cargo test -p vo-cli
```
The existing `drain_runtime_signals_shutdown_and_waits_for_tasks` test must pass unchanged.

### Test 4: Workspace tests
```bash
cargo test --workspace
```
All existing tests pass.

---

## Section 5: E2E Tests

No new E2E tests. The heartbeat watcher itself is integration-tested via NATS (see `crates/vo-actor/src/heartbeat.rs:166`). This bead only wires it into serve startup.

**Manual verification:** Run `wtf serve`, observe log line `"heartbeat expiry watcher started"` from the spawned task.

---

## Section 5.5: Verification Checkpoints

1. `cargo check -p vo-cli` compiles
2. `cargo clippy -p vo-cli -- -D warnings` passes
3. `cargo test --workspace` passes
4. `drain_runtime` signature updated to accept the third `JoinHandle`
5. `shutdown_rx` is cloned for the heartbeat watcher (not moved — timer_task already uses the original)
6. The import `vo_actor::heartbeat::run_heartbeat_watcher` is added

---

## Section 6: Implementation Tasks

### Task 1: Add import (1 line)
**File:** `crates/vo-cli/src/commands/serve.rs`
**Change:** Add to the existing imports block:
```rust
use vo_actor::heartbeat::run_heartbeat_watcher;
```

### Task 2: Spawn heartbeat watcher (1 line)
**File:** `crates/vo-cli/src/commands/serve.rs`
**Change:** After line 78 (where `timer_shutdown` is assigned), before line 80 (api_task spawn), add:
```rust
let heartbeat_shutdown = shutdown_rx.clone();
let heartbeat_task = tokio::spawn(run_heartbeat_watcher(
    kv.heartbeats.clone(),
    master.clone(),
    heartbeat_shutdown,
));
```

### Task 3: Update `drain_runtime` to await the heartbeat task
**File:** `crates/vo-cli/src/commands/serve.rs`
**Change:**
- Add `heartbeat_task: JoinHandle<Result<(), String>>` parameter to `drain_runtime`.
- Inside the function body, add: `let heartbeat_result = heartbeat_task.await.context("heartbeat watcher task join failed")?;`
- Add: `heartbeat_result.context("heartbeat watcher failed")?;` after the existing result checks.
- Update the call site at line 88 to pass `heartbeat_task`.
- Update the generic bounds — the heartbeat task's error type is `String` (not `std::error::Error`), so either convert with `.map_err()` at the spawn site, or use a separate `.await` in the drain function. Simplest: `.map_err(|e| anyhow::anyhow!("{e}"))` in the task, making it `JoinHandle<Result<(), anyhow::Error>>`.

### Task 4: Update call site
**File:** `crates/vo-cli/src/commands/serve.rs`
**Change:** Line 88 becomes:
```rust
drain_runtime(shutdown_tx, api_task, timer_task, heartbeat_task, || master.stop(None)).await?;
```

---

## Section 7: Failure Modes

| Failure | Impact | Mitigation |
|---------|--------|------------|
| `watch_all()` fails (NATS unavailable) | `run_heartbeat_watcher` returns `Err(String)` | `drain_runtime` propagates error, server exits with context message |
| Heartbeat watcher panics | `JoinHandle::await` returns `Err(JoinError)` | `drain_runtime` context-propagates the panic |
| `kv.heartbeats` bucket not provisioned | `watch_all()` call fails | Already handled — returns `Err` |
| Shutdown signal not received | Heartbeat watcher never stops | Same risk as timer_task — mitigated by `drain_runtime` sending shutdown |

---

## Section 7.5: Anti-Hallucination

- `run_heartbeat_watcher` signature is `pub async fn run_heartbeat_watcher(heartbeats: Store, orchestrator: ActorRef<OrchestratorMsg>, shutdown_rx: tokio::sync::watch::Receiver<bool>) -> Result<(), String>` — confirmed at `crates/vo-actor/src/heartbeat.rs:55`.
- `kv.heartbeats` is `pub heartbeats: Store` — confirmed at `crates/vo-storage/src/kv.rs:31`.
- `master` is `ActorRef<OrchestratorMsg>` — confirmed by `MasterOrchestrator::spawn()` return type at `crates/vo-cli/src/commands/serve.rs:65`.
- `vo_actor::heartbeat` is a public module — confirmed at `crates/vo-actor/src/lib.rs:6`.
- `drain_runtime` currently accepts exactly 2 `JoinHandle` params (api_task, timer_task) — confirmed at `crates/vo-cli/src/commands/serve.rs:93-97`. Adding a third requires updating the function signature.
- `Result<(), String>` does NOT implement `std::error::Error` — the generic bound `E: std::error::Error + Send + Sync + 'static` on lines 100-101 will NOT match. Must map the error.

---

## Section 7.6: Context Survival

- **Files modified:** 1 file (`crates/vo-cli/src/commands/serve.rs`)
- **Lines added:** ~10 (import + spawn + drain update)
- **Lines removed:** ~5 (drain_runtime signature/body restructured)
- **New dependencies:** None
- **Crate API changes:** None
- **Risk level:** Low — additive change, no existing behavior altered

---

## Section 8: Completion Checklist

- [ ] `use vo_actor::heartbeat::run_heartbeat_watcher;` added to imports
- [ ] `heartbeat_task` spawned with `kv.heartbeats.clone()`, `master.clone()`, `shutdown_rx.clone()`
- [ ] `drain_runtime` accepts and awaits the heartbeat `JoinHandle`
- [ ] Error type correctly handled (`String` → `anyhow::Error`)
- [ ] Call site passes `heartbeat_task` to `drain_runtime`
- [ ] `cargo check -p vo-cli` succeeds
- [ ] `cargo clippy -p vo-cli -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] No `unwrap()` or `expect()` introduced

---

## Section 9: Context

- **Bead:** vo-40m5
- **Depends on:** vo-r4aa (heartbeat watcher implementation — already merged)
- **ADR:** ADR-014 (heartbeat KV bucket, TTL=10s)
- **Pattern:** Same spawn pattern as `timer_task` at `serve.rs:81-85`
- **Precedent:** `api_task` and `timer_task` both use `tokio::spawn` + `shutdown_rx.clone()` + drain in `drain_runtime`

---

## Section 10: AI Hints

- The `drain_runtime` generic bounds use `E: std::error::Error`. Since `run_heartbeat_watcher` returns `Result<(), String>`, you cannot directly use the generic. Options:
  1. Wrap at spawn: `tokio::spawn(async move { run_heartbeat_watcher(...).await.map_err(|e| anyhow::anyhow!("{e}")) })` — then it fits the existing generic.
  2. Add a third generic param `EHeart: std::error::Error + Send + Sync + 'static` to `drain_runtime`.
  3. Drop the heartbeat task's generic, just do `heartbeat_task.await.context("...")?` and handle `Result<Result<(), String>, JoinError>` directly.
  Option 1 is cleanest — keeps `drain_runtime` generic unchanged.
- Remember `shutdown_rx` is already moved into `timer_shutdown` on line 78. You need an additional `.clone()` before that move. The order of clones matters: clone for heartbeat **before** line 78 assigns to `timer_shutdown`.
- `master.clone()` is cheap — `ActorRef` is `Arc`-based.
