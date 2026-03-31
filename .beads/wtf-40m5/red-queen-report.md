# Red Queen Report — wtf-40m5: "serve: Start heartbeat watcher in serve.rs"

**Date:** 2026-03-23
**Files under test:**
- `crates/wtf-cli/src/commands/serve.rs` (354 lines)
- `crates/wtf-actor/src/heartbeat.rs` (167 lines)

---

## Attack 1: Shutdown ordering — heartbeat_shutdown cloned before timer_shutdown consumes shutdown_rx

**Attack:** If `timer_shutdown = shutdown_rx.clone()` moved `shutdown_rx` before `heartbeat_shutdown` could clone it, the heartbeat watcher would never receive the shutdown signal and the task would hang forever during drain.

**Code path examined:** `serve.rs:85-89`
```rust
let (shutdown_tx, shutdown_rx) = watch::channel(false);
let api_shutdown = shutdown_rx.clone();
let heartbeat_shutdown = shutdown_rx.clone();  // line 87
let timer_shutdown = shutdown_rx.clone();       // line 88
let worker_shutdown = shutdown_rx;              // line 89 — move, not clone
```

**Result: SURVIVED**

`watch::Receiver::clone()` is cheap and does not consume `shutdown_rx`. All four clones happen before the final move on line 89. The heartbeat watcher will receive the shutdown signal.

---

## Attack 2: drain_runtime error handling — .map_err chain for heartbeat

**Attack:** The heartbeat task returns `Result<(), String>`. The `drain_runtime` function handles it differently from the other tasks (which return `Result<(), impl Error>`). Check if the error message propagates correctly to the caller.

**Code path examined:** `serve.rs:165,179,187-188`
```rust
heartbeat_task: JoinHandle<Result<(), String>>,   // line 165
let heartbeat_result = heartbeat_task.await.context("heartbeat watcher task join failed")?;  // line 179
heartbeat_result
    .map_err(|e| anyhow::anyhow!("heartbeat watcher failed: {e}"))?;  // lines 187-188
```

**Analysis:** The heartbeat watcher returns `Result<(), String>`. Since `String` doesn't implement `std::error::Error`, `.context()` can't be used. The author correctly used `.map_err()` instead. The JoinError case is covered by `.context()` on line 179.

**Result: SURVIVED**

The error chain correctly wraps both JoinError (task join failure) and the inner String error (watcher logic failure) into anyhow errors.

---

## Attack 3: Task join on panic — does drain_runtime handle JoinError?

**Attack:** If `run_heartbeat_watcher` panics, `heartbeat_task.await` returns `Err(JoinError)`. Does drain_runtime handle this?

**Code path examined:** `serve.rs:179`
```rust
let heartbeat_result = heartbeat_task.await.context("heartbeat watcher task join failed")?;
```

**Analysis:** `JoinHandle::await` returns `Result<T, JoinError>`. The `.context()` on line 179 catches any `JoinError` (including panic) and converts it to an anyhow error with message "heartbeat watcher task join failed". This is consistent with all other task joins (api on line 177, timer on line 178, worker on line 180).

The `run_heartbeat_watcher` function itself has `#![deny(clippy::panic)]` (heartbeat.rs:21), and the function body has no unwrap/expect — so a panic from within is unlikely.

**Result: SURVIVED**

Panic propagation is handled correctly via `.context()` on the JoinError.

---

## Attack 4: Double spawn — is there any path where heartbeat could be spawned twice?

**Attack:** Grep the entire codebase for any other `tokio::spawn` of `run_heartbeat_watcher`.

**Command:** `rg 'tokio::spawn.*heartbeat|run_heartbeat_watcher' crates/ -n`

**Findings:**
- `serve.rs:16` — import
- `serve.rs:97` — the single spawn site
- `heartbeat.rs:49` — doc comment example (`/// tokio::spawn(run_heartbeat_watcher(...))`)
- `heartbeat.rs:55` — function definition
- `heartbeat.rs:166` — comment about integration tests

**Result: SURVIVED**

Exactly one spawn site in production code. No path for double spawn.

---

## Attack 5: Test isolation — run `cargo test -p wtf-cli` twice

**Attack:** Flaky tests that depend on shared state (NATS, ports, filesystem) could pass once and fail on repeat.

**Commands:**
```
cargo test -p wtf-cli -- --nocapture  # Pass 1
cargo test -p wtf-cli -- --nocapture  # Pass 2
```

**Results:**
- Pass 1: 10 passed, 0 failed
- Pass 2: 10 passed, 0 failed

Both passes identical. All 10 tests stable across runs.

**Result: SURVIVED**

---

## Attack 6: Clippy strict — unwrap_used + expect_used

**Attack:** Run clippy with `clippy::unwrap_used` and `clippy::expect_used` warnings enabled on both crates.

**Commands:**
```
cargo clippy -p wtf-cli -- -W clippy::unwrap_used -W clippy::expect_used
cargo clippy -p wtf-actor -- -W clippy::unwrap_used -W clippy::expect_used
```

**Findings:**
- `wtf-cli/serve.rs`: No unwrap/expect in production code. One `.expect("already asserted is_err")` on line 338, but this is in `#[cfg(test)]` block — acceptable (test-only assertion after prior `assert!`).
- `wtf-actor/heartbeat.rs`: No unwrap/expect anywhere. The file already has `#![deny(clippy::unwrap_used)]` and `#![deny(clippy::expect_used)]` at module level (lines 19-20).
- No clippy errors (only pedantic doc-markdown warnings in dependency crates, not in the target files).

**Result: SURVIVED**

---

## Summary

| # | Attack Vector | Result | Severity |
|---|--------------|--------|----------|
| 1 | Shutdown ordering | SURVIVED | Critical |
| 2 | drain_runtime error chain | SURVIVED | High |
| 3 | Task join on panic | SURVIVED | High |
| 4 | Double spawn | SURVIVED | Medium |
| 5 | Test isolation (2x) | SURVIVED | Medium |
| 6 | Clippy strict | SURVIVED | Medium |

**Verdict: ALL 6 ATTACKS SURVIVED. The heartbeat watcher integration is solid.**

**Minor observations (non-blocking):**
- `serve.rs:187-188` uses `.map_err` instead of `.context` for heartbeat error — this is correct since `String` doesn't impl `Error`, but it breaks the pattern used for api/timer/worker errors. Could consider returning `Result<(), anyhow::Error>` from `run_heartbeat_watcher` instead of `Result<(), String>`.
