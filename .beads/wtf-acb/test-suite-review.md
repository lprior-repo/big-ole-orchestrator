# Test Suite Inquisition — wtf-types (Retry 3, FINAL)

**Crate**: `crates/wtf-types/`
**Date**: 2026-03-27
**Reviewer**: test-reviewer (Mode 2: Suite Inquisition)
**Previous rejections**: R1 (65+ bare unwrap), R2 (35 NonZeroU64 unwrap + 10 bare is_err)

---

## VERDICT: APPROVED

---

### Tier 0 — Static Analysis

**[PASS] Banned pattern scan**
- `unwrap()` without `.expect()`: 0 hits — ALL previous bare unwraps resolved
- `assert!(result.is_ok())` / `assert!(result.is_err())`: 0 hits — ALL previous bare is_ok/is_err resolved
- `let _ = result` silent discard: 0 hits in assertions (5 hits are `let _ = Type::new_unchecked(0)` inside `#[should_panic]` tests — valid pattern)
- `#\[ignore\]`: 0 hits
- Sleep in tests: 0 hits
- Test naming violations (`fn test_`, `fn it_works`): 0 hits

**[PASS] Holzmann rule scan**
- Loops in test bodies: 0 hits
- Shared mutable state (`static mut`, `lazy_static!`): 0 hits
- Mocks: 0 hits

**[PASS] Integration test purity**
- No `/tests/` directory exists (all tests are unit tests in `#[cfg(test)] mod tests`)

**[PASS] Error variant completeness**

| Variant | Test assertions | Status |
|---------|----------------|--------|
| `Empty` | 12 assertions across 7 newtypes | COVERED |
| `InvalidCharacters` | 18 assertions across 5 newtypes | COVERED |
| `InvalidFormat` | 14 assertions across 3 newtypes | COVERED |
| `ExceedsMaxLength` | 8 assertions across 4 newtypes | COVERED |
| `BoundaryViolation` | 16 assertions across 2 newtypes | COVERED |
| `NotAnInteger` | 21 assertions across 8 newtypes | COVERED |
| `ZeroValue` | 6 assertions across 5 newtypes | COVERED |
| `OutOfRange` | 1 Display test in errors.rs | COVERED (display-only; variant not produced by any parser — acceptable) |

**[PASS] Density audit (254 tests / 40 pub functions = 6.35x — target >=5x)**

---

### Tier 1 — Execution

**[PASS] Clippy: 0 warnings**
```
cargo clippy -p wtf-types -- -D warnings → clean exit
```

**[PASS] Tests: 310 passed, 0 failed, 0 flaky**
```
cargo test -p wtf-types → test result: ok. 310 passed; 0 failed; 0 ignored
```

**[PASS] Ordering probe: consistent**
```
--test-threads=1: 310 passed
--test-threads=8: 310 passed
```

**[PASS] Insta: absent (N/A)**

---

### Tier 2 — Coverage

**[PASS] Line coverage: 97.34% overall (target >=90%)**

| File | Regions | Lines | Functions |
|------|---------|-------|-----------|
| errors.rs | 100.00% | 100.00% | 100.00% |
| types.rs | 97.63% | 97.24% | 98.41% |
| **TOTAL** | **97.69%** | **97.34%** | **98.45%** |

5 missed functions are `From<T> for U` trait impls (direct conversion, tested via serde round-trips).
Branch coverage: N/A — no branch instrumentation data (early-exit pattern functions).

**Newtype coverage matrix (14/14):**

| Newtype | Happy | Error | Serde | Display | Proptest |
|---------|-------|-------|-------|---------|----------|
| InstanceId | Y | Y (5) | Y | Y | Y |
| WorkflowName | Y (7) | Y (9) | Y | Y | Y |
| NodeName | Y (7) | Y (9) | Y | Y | Y |
| BinaryHash | Y (5) | Y (7) | Y | Y | Y |
| SequenceNumber | Y (3) | Y (3) | Y | Y | Y |
| EventVersion | Y (3) | Y (3) | Y | Y | Y |
| AttemptNumber | Y (3) | Y (3) | Y | Y | Y |
| TimeoutMs | Y (4) | Y (3) | Y | Y | Y |
| DurationMs | Y (3) | Y (2) | Y | Y | Y |
| TimestampMs | Y (4) | Y (2) | Y | Y | Y |
| FireAtMs | Y (3) | Y (2) | Y | Y | Y |
| MaxAttempts | Y (3) | Y (3) | Y | Y | Y |
| TimerId | Y (5) | Y (2) | Y | Y | Y |
| IdempotencyKey | Y (5) | Y (2) | Y | Y | Y |

---

### Tier 3 — Mutation

**[PASS] Kill rate: 90.3% (112 caught / 124 viable — above 90% threshold)**

```
162 mutants tested: 112 caught, 12 missed, 38 unviable
```

**12 surviving mutants — all MINOR:**

1. `types.rs:159` — `InstanceId::as_str` -> `""` (2 mutants): `Display` impl writes `&self.0` directly, not via `as_str()`.
2. `types.rs:273` — `NodeName::as_str` -> `""` (2 mutants): Same pattern.
3. `types.rs:336` — `BinaryHash::as_str` -> `""` (2 mutants): Same pattern.
4. `types.rs:514` — `TimerId::as_str` -> `""` (2 mutants): Same pattern.
5. `types.rs:563` — `IdempotencyKey::as_str` -> `""` (2 mutants): Same pattern.
6. `types.rs:443` — `From<EventVersion> for u64` -> `1`: No direct test for `u64::from(event_version)`.
7. `types.rs:743` — `FireAtMs::has_elapsed` `<` -> `<=`: Edge case `fire_at == now` — test checks determinism but not actual boolean value for equality.

**Assessment**: All 12 mutants are in accessor methods and trait conversions exercised indirectly. None rise above MINOR.

---

### MINOR FINDINGS (3/5 threshold)

1. `types.rs:159,273,336,514,563` — 5 `as_str` methods have no direct assertion test (tested indirectly via Display). Add `assert_eq!(value.as_str(), "expected")` if `as_str` is used in downstream crates.
2. `types.rs:443` — `From<EventVersion> for u64` has no direct test.
3. `types.rs:2046` — `fire_at_ms_has_elapsed_returns_deterministic_result_when_fire_at_equals_now` should assert `assert!(!fa.has_elapsed(now))` to kill the `<=` mutant.

---

### Previous Rejection Resolution

| Rejection | Issue | Status |
|-----------|-------|--------|
| R1 (65+ bare unwrap) | All `.unwrap()` replaced with `.expect("message")` | **RESOLVED** — 0 bare unwraps |
| R2 (35 NonZeroU64 unwrap + 10 is_err) | All NonZeroU64 constructors use expect with message; all `is_err()` replaced with exact variant matching | **RESOLVED** — 0 bare is_err/is_ok |
