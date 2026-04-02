# Implementation Summary: wtf-40m5

- **bead_id:** wtf-40m5
- **bead_title:** serve: Start heartbeat watcher in serve.rs
- **phase:** STATE-3
- **updated_at:** 2026-03-23T12:00:00Z

## Files Modified

| File | Lines | Change |
|------|-------|--------|
| `crates/wtf-cli/src/commands/serve.rs` | L14 | Added `use wtf_actor::heartbeat::run_heartbeat_watcher;` import |
| `crates/wtf-cli/src/commands/serve.rs` | L79 | Added `let heartbeat_shutdown = shutdown_rx.clone();` before `timer_shutdown` move |
| `crates/wtf-cli/src/commands/serve.rs` | L88-92 | Spawned heartbeat watcher task with `kv.heartbeats.clone()`, `master.clone()`, `heartbeat_shutdown` |
| `crates/wtf-cli/src/commands/serve.rs` | L95 | Updated call site to pass `heartbeat_task` to `drain_runtime` |
| `crates/wtf-cli/src/commands/serve.rs` | L100-126 | Updated `drain_runtime` signature to accept `JoinHandle<Result<(), String>>` for heartbeat, await it, and propagate errors |
| `crates/wtf-cli/src/commands/serve.rs` | L162-228 | Updated test to provide third heartbeat handle and assert it drains |

## Design Decision: Error Type Handling

The spec suggested Option 1 (wrap `String` error in `anyhow::Error` at spawn site) to fit the existing generic bound. However, `anyhow::Error` deliberately does NOT implement `std::error::Error`, so this approach would not compile with the generic constraint `E: std::error::Error`.

**Chosen approach (Option 3 variant):** The heartbeat task keeps its natural `JoinHandle<Result<(), String>>` type. The `drain_runtime` function accepts this concrete type directly (not generic), and uses `.map_err(|e| anyhow::anyhow!(...))` to convert the `String` error into `anyhow::Error` for propagation. This is the cleanest approach that doesn't introduce spurious type wrapping.

## Constraint Adherence

| Constraint | Status | Evidence |
|------------|--------|----------|
| Zero `unwrap()`/`expect()` | PASS | No new unwrap/expect calls in any modified code |
| Zero `mut` in core logic | PASS | No `mut` added |
| Data->Calc->Actions | PASS | Spawn site is Actions (I/O boundary); `drain_runtime` is Actions (shutdown propagation) |
| Error propagation | PASS | `JoinError` handled via `.context()`, inner `String` error via `.map_err()` |
| Clone ordering correctness | PASS | `shutdown_rx` cloned for heartbeat on L79, BEFORE move to `timer_shutdown` on L80 |

## Tests Written

| Test Name | Status | Description |
|-----------|--------|-------------|
| `drain_runtime_signals_shutdown_and_waits_for_tasks` | PASS | Updated existing test to verify all three tasks (api, timer, heartbeat) receive shutdown and complete |

## Verification

```
$ cargo check -p wtf-cli --tests
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

$ cargo test -p wtf-cli
running 9 tests
test serve::tests::drain_runtime_signals_shutdown_and_waits_for_tasks ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo clippy -p wtf-cli
No new warnings introduced. Pre-existing warnings in wtf-common (missing_errors_doc) block
`-D warnings` but are unrelated to this bead.
```

## Spec Checklist

- [x] `use wtf_actor::heartbeat::run_heartbeat_watcher;` added to imports
- [x] `heartbeat_task` spawned with `kv.heartbeats.clone()`, `master.clone()`, `shutdown_rx.clone()`
- [x] `drain_runtime` accepts and awaits the heartbeat `JoinHandle`
- [x] Error type correctly handled (`String` -> `anyhow::Error` via `.map_err()`)
- [x] Call site passes `heartbeat_task` to `drain_runtime`
- [x] `cargo check -p wtf-cli --tests` succeeds
- [x] `cargo test -p wtf-cli` passes (9/9)
- [x] No `unwrap()` or `expect()` introduced
