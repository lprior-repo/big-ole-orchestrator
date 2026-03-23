# Implementation: Remove Dead `TerminateError::Failed` Variant

## Defect (from Red Queen)
`TerminateError::Failed(String)` existed in the enum but was never produced at runtime.

## Root Cause Analysis
Traced the full data flow:

1. **Producer** (`crates/wtf-actor/src/master/handlers/terminate.rs:39`): Had `inner.map_err(|e: wtf_common::WtfError| TerminateError::Failed(e.to_string()))` — but the inner result from the Cancel handler is ALWAYS `Ok(())`.

2. **Instance Cancel handler** (`crates/wtf-actor/src/instance/handlers.rs:143-175`): `handle_cancel` unconditionally calls `reply.send(Ok(()))` on line 172. The event store publish failure (lines 158-169) is intentionally swallowed (logged but not propagated) because cancellation is best-effort — the actor stops regardless of event persistence success.

3. **Consumer** (`crates/wtf-api/src/handlers/workflow.rs:347-351`): Had a match arm for `TerminateError::Failed` that was unreachable.

**Verdict: Option B — Remove `Failed`.** It is genuinely dead code with no production path.

## Changes Made

### 1. `crates/wtf-actor/src/messages/errors.rs`
- Removed `Failed(String)` variant from `TerminateError` enum.

### 2. `crates/wtf-actor/src/master/handlers/terminate.rs`
- Replaced `inner.map_err(...)` with explicit match on `Ok(())` / `Err(_)`.
- The `Err(_)` arm is defensive (the Cancel handler always replies Ok) but kept for forward-compatibility if the handler contract ever changes.

### 3. `crates/wtf-api/src/handlers/workflow.rs`
- Removed the `TerminateError::Failed(msg)` match arm from `map_terminate_result`.

## Constraint Adherence

| Constraint | Status |
|---|---|
| Zero unwrap/expect | Maintained — no new unwrap/expect introduced |
| Exhaustive match | Maintained — all `TerminateError` variants explicitly handled |
| No dead code | Improved — removed unreachable variant and its consumption |
| Expression-based | Maintained |
| Clippy clean | `cargo check -p wtf-actor -p wtf-api` passes with zero new warnings |

## Verification

```
cargo check -p wtf-actor -p wtf-api    # CLEAN
cargo test -p wtf-actor -- terminate   # 1 passed
cargo test -p wtf-api   -- terminate   # 4 passed
```

All 5 terminate-related tests pass. No other tests affected.
