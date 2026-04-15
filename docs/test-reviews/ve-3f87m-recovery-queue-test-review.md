# Test Plan Review: Recovery Queue Throttling & Orphan Detection

**Bead**: ve-3f87m
**Target**: `crates/vo-core/src/recovery.rs` (tests + implementation)
**Reviewer**: nuka (adversarial inquisition)
**Date**: 2026-04-15

## VERDICT: REJECTED

---

### Tier 0 — Static Analysis

| Check | Status | Notes |
|-------|--------|-------|
| Banned pattern scan | PASS | No bare `is_ok()`/`is_err()` assertions found |
| Silent error suppression | PASS (test code) | `let _ =` on line 217 and `.ok()` on line 213 are in production code, not tests |
| Ignored tests | PASS | No `#[ignore]` found |
| Sleep in tests | PASS | No sleep calls found |
| Holzmann loop scan | PASS | Only loop at line 343 is in production `run_sweep`, not in test bodies |
| Mock interrogation | PASS | No mocks used |
| Integration test purity | N/A | Tests are inline `#[cfg(test)]` module |
| Error variant completeness | **LETHAL** | `RecoveryError::SweepTimeout` has ZERO test coverage |
| Density audit | **LETHAL** | 10 tests / 19 pub fns = 0.53x (target >= 5x) |

### Tier 1 — Execution

| Check | Status | Notes |
|-------|--------|-------|
| Clippy | **BLOCKED** | vo-storage compilation errors prevent build |
| Tests pass | **BLOCKED** | Cannot compile due to pre-existing vo-storage mmap_cache.rs errors |
| Ordering probe | **BLOCKED** | Same compilation failure |

### Tier 2 — Coverage

**BLOCKED** — Cannot run coverage due to compilation failure in vo-storage.

### Tier 3 — Mutation

**BLOCKED** — Cannot run mutants. Analysis below is thought-experiment only.

---

## LETHAL FINDINGS (3)

### L1: `RecoveryError::SweepTimeout` has no test

**File**: `crates/vo-core/src/recovery.rs:226`

The `RecoveryError` enum defines 4 variants:
- `QueueFull` — tested (line 466, 519)
- `ChannelClosed` — tested (line 524)
- `OrphanDetectionFailed` — tested (line 527)
- `SweepTimeout` — **ZERO test coverage**

No test constructs a `SweepTimeout`, no test asserts on it, no test verifies its `Display` output. If the sweep timeout path was deleted from production, no test would catch it.

**Required test**: `recovery_error_sweep_timeout_display` asserting exact display string.

### L2: Density ratio 0.53x (10 tests / 19 public functions)

**File**: `crates/vo-core/src/recovery.rs`

19 public functions/methods, only 10 tests. Critical untested surface:

| Untested function | Risk |
|---|---|
| `ThrottledRecoveryChannel::enqueue_timer_recovery` (async) | Timer recovery hot path never exercised |
| `ThrottledRecoveryChannel::take_receiver` | Receiver handoff logic untested — could lose items |
| `ThrottledRecoveryChannel::with_receiver` | Alternate constructor untested |
| `ThrottledRecoveryChannel::config` | Accessor (low risk) |
| `RecoverySweeper::new` | Sweeper construction untested |
| `RecoverySweeper::run_sweep` | **Critical**: The entire sweep loop is untested. No test exercises orphan detection -> throttled enqueue flow. |
| `RecoverySweeper::run_periodic_sweep` | **Critical**: Periodic sweep timer untested. No test verifies MissedTickBehavior::Skip behavior. |
| `RecoverySweeper::channel` | Accessor (low risk) |
| `RecoverySweeper::state` | State accessor (low risk, but `unwrap()` on line 323 is concerning) |
| `OrphanDetector::detect_orphans` | Trait method untested (would need mock/integration) |
| `OrphanDetector::is_orphan_candidate` | Trait method untested |

### L3: `unwrap()` in production code

**File**: `crates/vo-core/src/recovery.rs:323`

```rust
pub fn state(&self) -> std::sync::MutexGuard<'_, OrphanSweepState> {
    self.state.lock().unwrap()
}
```

This `unwrap()` on a `std::sync::Mutex` will panic if the lock is poisoned (i.e., another thread panicked while holding it). In a recovery system — the very thing that handles failures — this is a reliability inversion. The recovery sweeper should be the most resilient component, not one that panics on unrelated failures.

---

## MAJOR FINDINGS (4)

### M1: No test for concurrent enqueue contention

The `ThrottledRecoveryChannel` uses `mpsc::Sender` (which is `Sync`) and a `watch::Sender` for status. Multiple threads could call `try_enqueue_orphan` concurrently. There is zero test for:
- Two threads racing to enqueue when queue has capacity 1
- Status watch updating correctly under concurrent modifications
- `is_full()` returning stale data after status change

### M2: No test for `RecoverySweeper::run_sweep` with mock detector

The `run_sweep` method is the core business logic — it takes detected orphans and enqueues them with throttling. This is a perfect candidate for testing with a mock `OrphanDetector`. No such test exists. Specifically untested:
- Batch size limiting (`max_orphan_batch_size`)
- Correct metric tracking (`orphans_detected`, `orphans_enqueued`, `orphans_rejected`)
- Partial rejection (some enqueued, some rejected) metric accuracy
- Empty orphan list early return

### M3: No proptest for queue bounds

The `RecoveryChannelConfig` has `queue_capacity`, `max_orphan_batch_size`, and `sweep_interval`. No proptest verifies:
- Queue capacity boundary (enqueue exactly capacity, one more fails)
- Batch size boundary (detect exactly batch_size orphans, verify only batch_size attempted)
- Rejection rate calculation with edge cases (0 detected, all rejected)

### M4: `update_status` is dead code

**File**: `crates/vo-core/src/recovery.rs:216-218`

```rust
fn update_status(&self, status: RecoveryQueueStatus) {
    let _ = self.status_sender.send(status);
}
```

This method is private and never called. The status channel is created with `RecoveryQueueStatus::Ready` and never updated to `Full` or `Closed` anywhere in the code. This means:
1. `is_full()` always returns `false` (status never changes from `Ready`)
2. `can_enqueue()` always returns `true`
3. The throttling via status is **completely non-functional**

The actual throttling only works through `mpsc::channel` backpressure, not the status-based gating. The status watch channel is decorative.

---

## MINOR FINDINGS (3)

### m1: Test `recovery_error_display` only checks substring containment

Line 520: `assert!(err.to_string().contains("Queue full"))` — the actual display string is `"Recovery queue full, cannot enqueue orphan..."`. The test would pass even if the display string was `"Queue full of something else"`. Should assert exact format or at least a more specific substring.

### m2: No test for `OrphanRecord` equality/clone

`OrphanRecord` derives `Clone, PartialEq, Eq` but no test verifies clone produces equal value or that equality works correctly across all `OrphanReason` variants.

### m3: No test for `RecoveryItem` variants

`RecoveryItem::TimerRecovery` is tested indirectly via `try_enqueue_timer_recovery`, but there is no test verifying the `RecoveryItem` enum equality or that timer recovery items are received in correct form on the receiver side.

---

## MANDATE (required before APPROVED)

1. **Add test for `RecoveryError::SweepTimeout` display** — exact variant assertion
2. **Add integration test for `RecoverySweeper::run_sweep`** — mock detector, verify batch limiting, metric tracking, partial rejection
3. **Fix or remove dead `update_status` method** — either wire status tracking to queue depth, or remove the status watch channel entirely (currently decorative)
4. **Replace `unwrap()` at line 323** — use `lock().unwrap_or_else(|e| e.into_inner())` to handle poisoned mutex
5. **Add proptest for queue boundary invariants** — capacity enforcement, batch size enforcement
6. **Add concurrent enqueue test** — verify thread-safety of throttling
7. **Resolve vo-storage compilation errors** — blocking all test execution and coverage analysis

After fixes, re-submit for full re-review from Tier 0.
