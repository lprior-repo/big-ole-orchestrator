# Findings: tw-cwmt — Hardline: Replace assert!/expect! with Result

**Bead**: tw-cwmt
**Title**: Replace assert!/expect! with Result in production panic paths
**Type**: Bug (P0 - Security/Crash on corrupt input)
**Auditor**: brahmin
**Date**: 2026-04-24

---

## Executive Summary

Identified production panic paths that crash on invalid/corrupt input instead of returning Result. These violate defensive programming principles and can cause cascading failures in production.

---

## Finding 1: ReservedPermitBudget assert! (vo-actor)

**File**: `/home/lewis/gt/crates/vo-actor/src/lib.rs:226`

**Code**:
```rust
pub fn new(max_per_class: u32) -> Self {
    assert!(max_per_class > 0, "max_per_class must be > 0");
    // ...
}
```

**Issue**: `assert!` panics if `max_per_class == 0`. While technically a programming error (non-zero u32 is a class invariant), a constructor should return `Result<Self, Error>` rather than panic.

**Fix**: Return `Result<Self, StartError>` or use `NonZeroU32` type.

---

## Finding 2: SystemTime unwrap (vo-actor/probe.rs)

**Files**: `/home/lewis/gt/crates/vo-actor/src/probe.rs` (multiple locations)

**Locations**:
- Line 389-391: `HttpProbe::check()`
- Line 456-458: `TcpProbe::check()`
- Line 540: `ExecProbe::check()`
- Line 722: `ProbeScheduler` health check
- Line 1655, 1697, 1714: Additional probe implementations

**Code pattern**:
```rust
last_check_ms: std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_millis() as u64,
```

**Issue**: `.unwrap()` panics if `SystemTime::now()` returns a time before UNIX_EPOCH (possible on some systems with incorrect RTC/timezone). Also panics if clock skew goes negative.

**Fix**: Use `.ok()` or return `Result<u64, SystemTimeError>`:
```rust
std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_millis() as u64)
    .unwrap_or(0) // or proper error handling
```

---

## Finding 3: LazyLock expect (vo-executor/state.rs)

**File**: `/home/lewis/gt/crates/vo-executor/src/state.rs`

**Code**:
```rust
static STATE: LazyLock<DashMap<String, StepState>> = LazyLock::new(DashMap::new);
static LAST_ERROR: LazyLock<DashMap<String, ExecuteNodeError>> = LazyLock::new(DashMap::new);
```

**Issue**: `LazyLock` initialization with `Mutex` can panic if lock is poisoned. The `.expect()` on mutex operations will propagate panic.

**Status**: **ACCEPTABLE** — `LazyLock` usage is in global state initialization. The static initialization is part of the process startup contract and cannot fail in normal operation.

---

## Finding 4: RwLock expect (vo-core/config_hot_reload)

**File**: `/home/lewis/gt/crates/vo-core/src/config_hot_reload/hot_reload.rs`

**Count**: 6+ instances

**Locations**:
- Line 58: `self.current.read().expect(...)`
- Line 70: `self.pending.write().expect(...)`
- Line 80: `self.pending.write().expect(...)`
- Line 82-83: `self.current.write().expect(...)`
- Line 96: `self.pending.write().expect(...)`
- Line 133-136: `self.current.write().expect(...)`

**Code pattern**:
```rust
self.current
    .read()
    .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock")
```

**Issue**: While the comment claims safety, `RwLock` poisoning is a defensive mechanism. In presence of corrupt input or resource exhaustion, panicking is not ideal. Proper error propagation is preferred.

**Fix**: Return `Result<T, PoisonError<T>>` via `.into_inner()` after acquiring, or use a wrapper that maps poison to a known error state.

---

## Finding 5: ConfigManager expect

**Status**: **NOT FOUND** — No `ConfigManager` struct found in the codebase. May be a false positive from the original audit or renamed/moved.

---

## Recommendations

1. **ReservedPermitBudget**: Change `new()` to return `Result<Self, StartError>` with `#[track_caller]` for better error reporting.

2. **SystemTime**: Replace `.unwrap()` with `.ok().unwrap_or(0)` or better yet, track `SystemTimeError` and propagate it.

3. **RwLock**: Consider wrapping poison errors or using `std::panic::catch_unwind` to convert panics to errors.

4. **Audit Coverage**: These findings cover the production-critical paths. Test files are exempt from panic-free requirements as test failures are expected behavior.

---

## Risk Assessment

| Finding | Severity | Likelihood | Impact |
|---------|----------|------------|--------|
| ReservedPermitBudget | Medium | Low | Programming error, would panic on bad init |
| SystemTime | **HIGH** | Low-Medium | Clock skew could cause production crash |
| LazyLock | Low | Very Low | Global init, unrecoverable anyway |
| RwLock | Medium | Low | Resource exhaustion could trigger |

**Priority**: SystemTime issues should be fixed first as they can trigger from external conditions (clock skew) rather than internal programming errors.
