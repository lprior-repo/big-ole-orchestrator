# Implementation Summary

## Bead: wtf-7c1w
## Title: Harden serve run-loop runtime correctness

## Changes completed

- Extended `crates/wtf-cli/src/commands/serve.rs` with explicit runtime drain path:
  - Extracted `drain_runtime(...)` to coordinate shutdown signal fan-out and task draining.
  - Ensures API task and timer task are both awaited before returning.
  - Ensures orchestrator stop is executed after subsystem tasks resolve.
- Kept run-loop wiring in `run_serve` and routed shutdown path through `drain_runtime`.

## Tests added

- Added `serve::tests::drain_runtime_signals_shutdown_and_waits_for_tasks`:
  - Simulates signal propagation using `tokio::sync::watch`.
  - Verifies both api/timer task loops observe shutdown.
  - Verifies orchestrator stop closure executes.
  - Verifies drain function returns success only after all components complete.

## Functional Rust constraint adherence

- No `unwrap()`/`expect()`/`panic!()` in runtime implementation path.
- Runtime correctness factored into typed function (`drain_runtime`) with explicit error propagation.
- Side-effects are pushed to shell boundary; drain coordination remains explicit and testable.

## Verification run

- `cargo test -p wtf-cli serve::tests::drain_runtime_signals_shutdown_and_waits_for_tasks`

## Files changed

- `crates/wtf-cli/src/commands/serve.rs`
