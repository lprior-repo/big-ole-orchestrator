# Implementation Summary — Bead wtf-0qg: Defect Repairs

**Date:** 2026-03-23
**Agent:** functional-rust (repair)
**Status:** FIXES APPLIED

---

## Defects Addressed

### N-01 (CRITICAL): Silently swallowed metadata persistence error

**File:** `crates/wtf-actor/src/master/handlers/start.rs`

**Problem:** Line 94 had `let _ = store.put_instance_metadata(metadata).await;` which silently discarded any durability write failure. In a durable execution engine, a failed metadata write leaves the instance live in memory but invisible to crash recovery — an orphan actor with no heartbeat, no recovery path.

**Fix:**
1. Changed `persist_metadata` return type from `()` to `Result<(), WtfError>`.
2. When `state_store` is `None` (no store configured), returns `Ok(())` — nothing to persist, expected state.
3. When `state_store` IS configured, propagates the error from `put_instance_metadata` via `?`.
4. In `spawn_and_register`, the `persist_metadata` result is matched:
   - `Ok(())` → continue silently
   - `Err(e)` → log at `error!` level with structured fields (`instance_id`, `namespace`, `error`) and a message explaining the orphan risk
5. Spawn still succeeds — the actor IS running — but the operator is alerted via structured error log.

**Constraint adherence:**
- Zero `unwrap`/`expect` in production code
- Expression-based error propagation via `?`
- Structured tracing with field interpolation
- Function remains under 25 lines (`spawn_and_register`: 21 lines, `persist_metadata`: 17 lines)

### N-04 (MEDIUM): Missing test for AlreadyExists validation branch

**File:** `crates/wtf-actor/src/master/handlers/start.rs` (test module)

**Problem:** `validate_request` had two error branches (`AtCapacity` and `AlreadyExists`) but only `AtCapacity` was tested. The duplicate-instance guard had zero test coverage.

**Fix:**
Added `validate_request_rejects_when_instance_already_exists` test that:
1. Creates `OrchestratorState` with capacity for 10 instances
2. Registers a dummy instance via `state.register(id, actor_ref)` using a minimal `NullActor` (message-discarding actor)
3. Calls `validate_request` with the same ID
4. Asserts `Err(StartError::AlreadyExists(_))` via `matches!`

**Constraint adherence:**
- `expect` used only in test code (acceptable per rules)
- Test uses `#[tokio::test]` for async actor spawn
- `NullActor` is a minimal, purpose-built test double that correctly implements `ractor::Actor`

---

## Files Changed

| File | Change |
|------|--------|
| `crates/wtf-actor/src/master/handlers/start.rs` | N-01: `persist_metadata` returns `Result`, error logged in `spawn_and_register`. N-04: Added `AlreadyExists` test with `NullActor` test double. |

## Verification

```
cargo check -p wtf-actor   → clean (0 errors, 0 new warnings)
cargo test -p wtf-actor    → 68 unit tests + 27 integration tests passed
  including: validate_request_rejects_when_instance_already_exists ✓
```
