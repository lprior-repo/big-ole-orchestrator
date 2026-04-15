# Black-Hat Adversarial Review: Load Shedding Semaphore (ADR-006)

**Bead**: ve-m8kp4
**Target**: `crates/vo-actor/src/semaphore.rs` + `crates/vo-actor/tests/semaphore_tests.rs`
**Reviewer**: nuka (black-hat inquisition)
**Date**: 2026-04-15

## VERDICT: REJECTED

---

## PHASE 1: Contract & Bead Parity

### ADR-006 Contract Compliance

| ADR Requirement | Status | Notes |
|---|---|---|
| Global `tokio::sync::Semaphore` with fixed permits | PASS | `ExecutionSemaphore` wraps `Semaphore` with configurable permits |
| Zero-cost yielding when permits exhausted | PASS | Uses `semaphore.acquire().await` |
| Ingress load shedding at waiters > threshold | PASS | `calculate_backpressure_status` checks `max_waiters_for_shed` |
| HTTP 429/503 response on shed | **NOT IN SCOPE** | No HTTP integration, semaphore logic only |

### Missing Contract Enforcement

- **LETHAL**: `available_permits` tracking uses `Ordering::Relaxed` atomics (line 253, 299, 300) while decrementing on acquire. The actual semaphore and the atomic counter can diverge. If a permit is acquired via `semaphore.acquire()` but the atomic update races, `current_status()` returns stale data. This breaks the invariant "shedding active when waiters >= threshold."

- **LETHAL**: `acquire()` (line 282-319) takes `Arc<Self>` but `try_acquire()` (line 250) takes `&self`. This API inconsistency means the caller must own an `Arc` for one path but not the other. The `acquire()` method increments `waiting_count` before checking status (line 283), then decrements if rejected (line 289). Between fetch_add and fetch_sub, another thread reads a falsely inflated waiting count, potentially triggering premature load shedding.

---

## PHASE 2: Farley Engineering Rigor

### Function Length Violations

| Function | Lines | Status |
|---|---|---|
| `acquire()` | 282-319 (37 lines) | **LETHAL**: Over 25-line limit |
| `AdmissionDecision::eq()` | 105-131 (26 lines) | **LETHAL**: Over 25-line limit |
| `ExecutionSemaphore::new()` | 227-238 (11 lines) | PASS |
| All other functions | < 25 lines | PASS |

### I/O in Calculations

- Line 282-319: `acquire()` mixes pure decision logic (load shedding check at line 288-294) with async I/O (semaphore acquire at line 297). The decision and the action should be separated per Data-Calc-Actions architecture.

### Test Quality

- **MAJOR**: `execution_semaphore_acquire_and_release` (line 726-744) drops the `Arc<ExecutionSemaphore>` at line 739, then creates a NEW semaphore at line 742 to verify permits. This test does NOT verify permit release — it creates a fresh object. The original permit is leaked when the Arc is dropped (the semaphore is destroyed, not released).

- **MAJOR**: Multiple tests use `assert!(permit.is_some())` / `assert!(permit.is_none())` without checking the exact permit state. These would pass even if `try_acquire` always returned `Some(())`.

---

## PHASE 3: NASA-Level Functional Rust (The Big 6)

### Make Illegal States Unrepresentable

- **LETHAL**: `SemaphoreConfig` has no validation. You can construct `SemaphoreConfig { max_concurrent_binaries: 0, reserved_permits: 100, .. }` — which means reserved permits exceed total permits. There is no constructor that validates invariants.

- **LETHAL**: `InvariantCheck { allowed: false, status: BackpressureStatus::Healthy, error: None }` is representable but semantically invalid. If not allowed, there should always be an error reason.

### Parse, Don't Validate

- No parsing at boundary. `SemaphoreConfig` values are used directly without validation. The only "validation" is a default constructor.

### Types as Documentation

- **MINOR**: `InvariantCheck.allowed: bool` is a boolean field carrying meaning. Should be an enum `CheckResult::Allowed | Denied(InvariantError)`.

### Newtypes

- **MINOR**: `RetryAfterSecs` is `u32`, `position` in `Queued` is `usize`, `estimated_wait_ms` is `u64`. All are raw primitives in the domain model.

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin)

### Option-based State Machine

- **LETHAL**: `InvariantCheck { error: Option<InvariantError> }` is an option-based state machine. When `allowed` is true, `error` should be guaranteed `None`. When `allowed` is false, `error` should be guaranteed `Some`. This is not enforced by types. Use:
  ```rust
  enum CheckResult {
      Allowed { status: BackpressureStatus },
      Denied(InvariantError),
  }
  ```

### The Panic Vector

| Location | Pattern | Severity |
|---|---|---|
| Line 414 | `self.semaphores.read().unwrap()` | **LETHAL**: Panics on poisoned lock |
| Line 421 | `self.semaphores.write().unwrap()` | **LETHAL**: Panics on poisoned lock |
| Line 441 | `self.semaphores.read().unwrap().len()` | **LETHAL**: Panics on poisoned lock |
| Line 447 | `self.semaphores.read().unwrap().is_empty()` | **LETHAL**: Panics on poisoned lock |
| Line 454 | `self.semaphores.write().unwrap()` | **LETHAL**: Panics on poisoned lock |
| Line 764 | `WorkflowName::parse("test-workflow").unwrap()` | MINOR (test code) |

5 `unwrap()` calls on `RwLock` in production code. A single poisoned lock (from any panic in any thread holding the lock) will cascade-panic the entire system. This is exactly the failure mode ADR-006 is designed to prevent.

### Unnecessary `let mut`

- Lines 421, 454: `let mut semaphores = self.semaphores.write().unwrap()` — these are needed for write access, PASS.

### CUPID Properties

- **Predictable**: The `Relaxed` ordering on atomics means the observed state is not deterministic across threads. This violates predictability.

---

## PHASE 5: The Bitter Truth (Velocity & Legibility)

### YAGNI Violations

- **MAJOR**: `reserved_semaphore` / `reserved_permits` system (lines 206, 230-231, 268-277, 354-362). This is infrastructure for "recovery tasks" that isn't used anywhere in the codebase. There are no callers of `try_acquire_recovery()` in production code (only in tests). Build it when needed.

- **MAJOR**: `WorkflowSemaphoreMap` — per-workflow limiting. No production code uses this. Only tests exercise it. This is speculative complexity.

- **MAJOR**: `InvariantEnforcer<S>` — generic over a registry interface. Has one method (`check_activation`) and no production usage. Classic abstract trait with zero implementers.

### Clever Code

- Line 161-165: Division to compute `usage_ratio` with `if total_permits > 0` guard but `1.0` default. This means "0 permits = fully loaded" which is correct, but the comment doesn't explain why. Make it explicit.

- Line 284: `let _ = waiting; // suppress unused warning` — the `waiting` variable from `fetch_add` is computed but discarded. Why increment a counter you immediately ignore? The waiting count tracking is incomplete.

---

## Summary of Findings

### LETHAL (7)

1. **Atomic/Semaphore divergence**: `available_permits` Relaxed atomic can diverge from real semaphore state
2. **Race in `acquire()`**: `fetch_add` then `fetch_sub` on `waiting_count` creates false load shedding
3. **`acquire()` over 25 lines**: 37 lines, mixing pure decision with async I/O
4. **`SemaphoreConfig` unvalidated**: Can construct invalid configurations
5. **`InvariantCheck` option-based state machine**: `allowed` + `Option<error>` is not type-safe
6. **5x `unwrap()` on `RwLock`**: Cascade-panic risk in the very component that prevents overload
7. **Dead `update_status` pattern**: `available_permits` is decremented on acquire but never incremented on release (permit drop). The counter only goes down, never up.

### MAJOR (4)

1. `acquire_and_release` test doesn't test release (creates new semaphore)
2. Tests use `is_some()`/`is_none()` without verifying permit validity
3. YAGNI: reserved semaphore system unused in production
4. YAGNI: `InvariantEnforcer` generic with zero implementers

### MINOR (3)

1. Boolean field `allowed` in `InvariantCheck` should be enum
2. Raw primitives for domain types (`u32`, `u64`, `usize`)
3. `let _ = waiting` is dead code

---

## MANDATE

1. **Replace all `RwLock` unwrap with `unwrap_or_else(|e| e.into_inner())`** — or switch to `parking_lot::RwLock` which doesn't poison
2. **Fix `available_permits` counter** — decrement on acquire is tracked but increment on release is missing; permits leak
3. **Replace `InvariantCheck` with `enum CheckResult { Allowed, Denied(InvariantError) }`** — make illegal states unrepresentable
4. **Add `SemaphoreConfig::validated()` constructor** — reject invalid configs (reserved > total, zero total, etc.)
5. **Fix `acquire()` race** — use `fetch_update` or rethink the atomic tracking to avoid false inflation
6. **Remove YAGNI code** — reserved semaphore, `InvariantEnforcer`, `WorkflowSemaphoreMap` if unused
7. **Fix `acquire_and_release` test** — verify actual permit release, don't create new object

After fixes, re-submit for full 5-phase re-review.
