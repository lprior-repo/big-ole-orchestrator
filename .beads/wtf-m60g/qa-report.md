# QA Report: wtf-m60g — "instance: Publish InstanceStarted event"

## Check 1: `publish_instance_started` exists in init.rs

**PASS** — Function found at `init.rs:156`. Public async fn with correct signature:
`pub async fn publish_instance_started(args: &InstanceArguments, event_log: &[WorkflowEvent]) -> Result<(), ActorProcessingErr>`

## Check 2: `WorkflowEvent::InstanceStarted` is used

**PASS** — 8 occurrences across init.rs:
- Production: `init.rs:169` constructs `WorkflowEvent::InstanceStarted { instance_id, workflow_type, input }`
- Production: `init.rs:167` error message references "InstanceStarted"
- Production: `init.rs:180-183` tracing info log
- Tests: lines 288, 297 validate the variant and fields

## Check 3: Non-empty event_log guard (crash recovery skip)

**PASS** — `init.rs:160-162`:
```rust
if !event_log.is_empty() {
    return Ok(());
}
```
Returns `Ok(())` immediately without publishing when event_log has entries (crash recovery path).

## Check 4: Missing event_store returns Err

**PASS** — `init.rs:164-167`:
```rust
let store = args.event_store.as_ref()
    .ok_or_else(|| ActorProcessingErr::from("No event store available for InstanceStarted publish"))?;
```
Uses `ok_or_else` to convert `None` into an `ActorProcessingErr`.

## Check 5: No unwrap/expect in production code

**PASS** — 4 matches found, ALL in `#[cfg(test)]` module (lines 228, 284, 312, 331 — lines 188+ are test-only). Zero `unwrap()`/`expect()` in production code (lines 1-187).

## Check 6: Unit tests for instance_started

**PASS** — All 3 tests pass:
```
test instance::init::tests::fresh_instance_publishes_started_event ... ok
test instance::init::tests::crash_recovery_skips_started_event ... ok
test instance::init::tests::no_event_store_returns_error ... ok
```

## Check 7: Full wtf-actor lib test suite

**PASS** — 123 passed, 0 failed, 0 ignored.

## Check 8: Call site in actor.rs

**PASS** — `actor.rs:51`:
```rust
init::publish_instance_started(&state.args, &event_log).await?;
```
Called after `spawn_live_subscription` (line 48) and before `state.phase = InstancePhase::Live` (line 53). Uses `?` to propagate errors.

## Check 9: Line count

**PASS** — 338 lines (init.rs). Under 300-line production threshold (tests start at line 188, production code is 187 lines).

## Summary

| # | Check | Result |
|---|-------|--------|
| 1 | `publish_instance_started` exists | PASS |
| 2 | `WorkflowEvent::InstanceStarted` used | PASS |
| 3 | Non-empty event_log guard | PASS |
| 4 | Missing event_store returns Err | PASS |
| 5 | No unwrap/expect in production | PASS |
| 6 | 3 unit tests pass | PASS |
| 7 | Full suite: 123/123 pass | PASS |
| 8 | Correct call site in actor.rs | PASS |
| 9 | Line count: 338 (prod: 187) | PASS |

## Overall Verdict: **PASS**

All contract requirements verified. Function correctly implements crash-recovery guard, error handling for missing event_store, and publishes InstanceStarted only for fresh instances. Call site is correctly positioned between spawn_live_subscription and phase transition to Live.
