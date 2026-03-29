# Kani Justification: vo-40m5

- **bead_id**: vo-40m5
- **phase**: STATE-5.7
- **updated_at**: 2026-03-23T19:00:00Z

## Critical State Machines
None. This bead wires an existing actor (`run_heartbeat_watcher`) into `serve.rs` startup. The heartbeat watcher itself is an infinite loop that scans KV and sends messages — no state machine transitions.

## Why Kani Adds Nothing
1. The spawn site is a single `tokio::spawn` call — no branching logic beyond error propagation
2. `drain_runtime` is a sequential `.await` chain — each task is awaited independently, no shared mutable state
3. Error handling is straightforward: `JoinError` → `anyhow::Error` → propagate up
4. The shutdown mechanism uses `tokio::sync::watch` — a well-tested stdlib primitive
5. No `unsafe` code, no raw pointer manipulation, no arithmetic on indices

## What Tests Already Guarantee
- `drain_runtime_signals_shutdown_and_waits_for_three_tasks` verifies all tasks drain
- `drain_runtime_propagates_worker_error` verifies error propagation
- Clippy strict mode passes on vo-cli source

## Conclusion
Kani model checking would not find any reachable invalid states that the borrow checker and test suite don't already exclude. The code is a wiring layer with no internal state machine.
