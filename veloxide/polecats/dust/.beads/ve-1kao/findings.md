# ADR-043 Review Findings: Exact-Once Verification Strategy

**Bead:** ve-1kao
**Task:** ADR-REVIEW: ADR-043 exact-once
**Date:** 2026-04-24
**Auditor:** polecat/dust

---

## Executive Summary

ADR-043 defines the crash-oriented exact-once verification strategy for Veloxide. The framework is **substantially implemented** with all 12 crash points, verification harness, and component failure tests in place. However, **TOCTOU races in the storage layer** (identified in blackhat audit) undermine the exact-once guarantees that ADR-043 aims to verify.

---

## ADR-043 Requirements vs. Implementation Status

### 1. Crash-Point Matrix ✅ FULLY IMPLEMENTED

| Crash Point | Location | Status |
|--------------|----------|--------|
| DedupeWrite | `vo-core/src/exact_once_verification/crash_points.rs:36` | ✅ Implemented |
| StepScheduled | `crash_points.rs:42` | ✅ Implemented |
| FenceAcquisition | `crash_points.rs:48` | ✅ Implemented |
| ChildStart | `crash_points.rs:54` | ✅ Implemented |
| EffectPrepared | `crash_points.rs:60` | ✅ Implemented |
| ConnectorCommit | `crash_points.rs:66` | ✅ Implemented |
| EffectCommitted | `crash_points.rs:72` | ✅ Implemented |
| StepCompleted | `crash_points.rs:78` | ✅ Implemented |
| TimerPersistence | `crash_points.rs:84` | ✅ Implemented |
| SignalAcceptance | `crash_points.rs:90` | ✅ Implemented |
| LineageRollover | `crash_points.rs:96` | ✅ Implemented |
| Compensation | `crash_points.rs:102` | ✅ Implemented |

**All 12 crash points have Before/After positions defined.**

### 2. Required Properties ✅ DEFINED

Seven required properties documented in `vo-core/src/exact_once_verification/mod.rs:24-34`:
1. ✅ Duplicate ingress does not create duplicate logical work
2. ✅ Stale fence completions cannot win
3. ✅ Replay after any injected crash reaches the same legal state
4. ✅ Connector ambiguity always routes through reconciliation
5. ✅ Projection rebuild reproduces the same operator state
6. ✅ Lineage rollover preserves correct signal routing
7. ✅ Compensation never runs for an effect that was never durably committed

### 3. Verification Harness ✅ IMPLEMENTED

`vo-core/src/exact_once_verification/harness.rs` provides:
- `VerificationHarness::new()` - creates harness without crash injection
- `VerificationHarness::with_crash_scenario()` - creates harness with crash point
- `should_crash()` / `should_crash_at()` - crash point checks
- `verify_lineage_rollover_deterministic()` - lineage rollover verification
- `build_lineage_rollover_sequence()` - test event sequence builder

### 4. Component Failure Simulation Tests ⚠️ EXISTS (compile errors)

`vo-core/src/exact_once_verification/component_failure_simulation_tests.rs` contains 547 lines of tests covering:
- Step component failures
- Timer component failures
- Signal component failures
- Child workflow failures
- Effect component failures
- Dedupe component failures
- Lineage rollover failures
- Data integrity verification
- Integration failure scenarios

**ISSUE:** Tests fail to compile due to type inference errors in `segment_tree.rs:195,476`.

### 5. Dedupe Store Implementation ✅ STRUCTURALLY SOUND

`vo-storage/src/dedupe_partition/fjall_dedupe.rs`:
- Uses striped `parking_lot::Mutex` (64 stripes) for per-key locking
- `check_and_insert()` is atomic (lines 56-95) - lock held during get+insert
- Binary wire format for entries (efficient encoding)

---

## Critical Issues Affecting Exact-Once Guarantees

### M11: TOCTOU in dedupe store `contains`
**File:** `vo-storage/src/dedupe_partition/fjall_dedupe.rs:141-155`

```rust
fn contains(&self, key: &DedupeKey) -> Result<bool, DedupeStoreError> {
    let encoded_key = super::encode_dedupe_key(key);
    let now_ms = Self::now_ms();
    // NO LOCK - potential TOCTOU
    match self.partition.get(&encoded_key) { ... }
}
```

**Risk:** Between `get()` returning and checking expiry, another thread could delete the key.

**Mitigation:** `contains()` is read-only; race window is small. Primary idempotency uses `check_and_insert()` which is locked.

### H8: TOCTOU Race in Lease Acquisition
**File:** `vo-storage/src/lease_partition/fjall_lease_store.rs:140-178`

Two concurrent callers could both acquire lease for same instance → double-execution of side effects.

**Impact:** Violates exactly-once admission contract.

### M12: TOCTOU in receipt/effect journal inserts
**File:** `vo-storage/src/fjall_receipt_store.rs:43-61`

Check-then-insert without atomic batching allows duplicate effect execution.

---

## Build/Test Status

| Command | Result |
|---------|--------|
| `cargo check` | ✅ Pass (warnings only) |
| `cargo build -p vo-core` | ✅ Pass |
| `cargo build -p vo-storage` | ✅ Pass |
| `cargo test -p vo-storage dedupe_partition::tests` | ✅ 99 tests pass |
| `cargo test -p vo-core exact_once_verification` | ❌ Compile error (segment_tree.rs type inference) |

---

## Recommendations

1. **Fix segment_tree.rs type inference** to enable vo-core tests
2. **Audit TOCTOU fixes** for H8 (lease), M11 (dedupe contains), M12 (receipt journal)
3. **Add integration tests** that verify exact-once behavior end-to-end with crash injection
4. **Run exact-once verification suite** on release gates per ADR-043 Section 4

---

## Files Reviewed

- `docs/adr/v2/ADR-043-v2-exact-once-verification-strategy.md`
- `crates/vo-core/src/exact_once_verification/mod.rs`
- `crates/vo-core/src/exact_once_verification/crash_points.rs`
- `crates/vo-core/src/exact_once_verification/harness.rs`
- `crates/vo-core/src/exact_once_verification/assertions.rs`
- `crates/vo-core/src/exact_once_verification/component_failure_simulation_tests.rs`
- `crates/vo-storage/src/dedupe_partition/mod.rs`
- `crates/vo-storage/src/dedupe_partition/fjall_dedupe.rs`
- `crates/vo-types/src/dedupe.rs`
- `polecats/shiny/hardline/.beads/ha-d35/findings.md` (blackhat audit)