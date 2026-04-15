# Architecture Drift Check - ve-pfqgo

## Summary

**STATUS: DRIFT DETECTED** - 9 files exceed 300-line limit. Refactor bead ve-j5bh2 filed.

## Files Analyzed

| File | Lines | Status | Notes |
|------|-------|--------|-------|
| `lease_partition/mod.rs` | 267 | ✅ PASS | DDD-compliant, proper Data-Calc-Actions layering |
| `lease_partition/in_memory_lease.rs` | 141 | ✅ PASS | Clean in-memory implementation |
| `lease_partition/fjall_lease_store.rs` | 392 | ❌ FAIL | Tests mixed with implementation |
| `lease_partition/tests_codec.rs` | 479 | ❌ FAIL | Split into codec_unit, codec_integration |
| `lease_partition/tests_integration_acquire.rs` | 547 | ❌ FAIL | Split by scenario |
| `lease_partition/tests_integration_expiry.rs` | 659 | ❌ FAIL | Split by scenario |
| `lease_partition/tests_integration_release.rs` | 606 | ❌ FAIL | Split by scenario |
| `lease_partition/tests_integration_stale.rs` | 584 | ❌ FAIL | Split by scenario |
| `lease_partition/tests_lease_entry.rs` | 385 | ❌ FAIL | Split into entry_unit, entry_validation |
| `vo-core/src/lease_calc.rs` | 710 | ❌ FAIL | Pure logic with embedded tests |

## DDD Compliance Assessment

### Strengths
- ✅ Core trait (`LeaseStore`) properly separated in `mod.rs`
- ✅ Pure encoding/decoding functions in calc layer
- ✅ Error types are well-structured enums with descriptive variants
- ✅ `lease_calc.rs` follows Scott Wlaschin pattern (Data → Calc)

### Violations
- ❌ 9 files exceed 300-line architectural limit
- ⚠️ Test files mixed with implementation files
- ⚠️ Monolithic test files should be split by scenario

## Refactor Action

**Bead ve-j5bh2 filed** for splitting oversized files into smaller modules.

## Core Architecture (DDD-Compliant)

```
┌─────────────────────────────────────────────────────────────┐
│ Data Layer                                                   │
│ ───────────────────────────────────────────────────────────  │
│ • LeaseStoreError (enum)                                    │
│ • LeaseEntry (struct with expiry)                           │
│ • LeaseState (enum: Vacant, Held, Expired)                  │
│ • LeaseTransition (enum: Acquire, Renew, Tick, Release)     │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Calc Layer (Pure Functions)                                  │
│ ───────────────────────────────────────────────────────────  │
│ • encode_lease_key(instance_id, step_id) → Vec<u8>          │
│ • decode_lease_key(bytes) → Result<(InstanceId, StepId)>    │
│ • encode_lease_entry(entry) → Result<Vec<u8>>               │
│ • decode_lease_entry(bytes) → Result<LeaseEntry>            │
│ • apply(state, transition) → Result<LeaseState>             │
│ • is_expired(expires_at_ms, now_ms) → bool                  │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Actions Layer (Side Effects)                                 │
│ ───────────────────────────────────────────────────────────  │
│ • LeaseStore trait (acquire, release, check_stale_fence)    │
│ • FjallLeaseStore (persistent implementation)               │
│ • InMemoryLeaseStore (test implementation)                  │
└─────────────────────────────────────────────────────────────┘
```
