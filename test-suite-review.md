# Test Suite Review: Health Check Probe Framework

**Module**: `crates/vo-actor/src/probe.rs`
**Reviewer**: veloxide/polecats/chrome (test-reviewer agent)
**Date**: 2026-04-12
**Scope**: Inline tests in probe.rs (96 test functions)

---

## VERDICT: REJECTED

---

### Tier 0 — Static Analysis

[PASS] Banned pattern scan — no `#\[ignore\]`, no sleep, no mocks, no shared mutable state
[FAIL] Banned assertion scan — 5 `is_ok()`/`is_err()` without exact variant checks
[FAIL] Tautological assertions — 5 assertions that are always true
[FAIL] Error variant completeness — 0/5 ProbeError variants asserted with exact match
[FAIL] Density audit — 96 tests / 28 pub fns = 3.43x (target ≥5x)
[FAIL] Holzmann loop scan — 12 bounded loops in test bodies (all bounded, no unbounded)
[INFO] Integration test purity — no `use crate::` in tests/ directory
[INFO] Insta — not present

### Tier 1 — Execution

[FAIL] Clippy: 3 errors in probe.rs, 19 errors in other crate files
[PASS] nextest: 409 passed, 0 failed, 0 flaky
[PASS] Ordering probe: consistent across single/multi-thread

### Tier 2 — Coverage

[SKIP] Clippy failure blocks Tier 2

### Tier 3 — Mutation

[SKIP] Clippy failure blocks Tier 3

---

## LETHAL FINDINGS

### L1. Tautological assertions — tests that always pass regardless of code correctness

| Line | Assertion | Why it's tautological |
|------|-----------|----------------------|
| 1530 | `assert!(result.is_err() \|\| result.is_ok())` | Every `Result` is either Err or Ok — always true |
| 1543 | `assert!(tcp_result.is_ok() \|\| tcp_result.is_err())` | Same — always true |
| 1544 | `assert!(exec_result.is_ok() \|\| exec_result.is_err())` | Same — always true |
| 1545 | `assert!(http_result.is_ok() \|\| http_result.is_err())` | Same — always true |
| 1812 | `assert!(r.latency_ms > 0 \|\| r.latency_ms == 0)` | Every u64 is either >0 or ==0 — always true |

**Impact**: `test_probe_trait_object_can_be_stored_and_called` and `test_multiple_probe_types_via_trait_object` are effectively untested. Delete the Probe impl and these tests still pass. `qa_smoke_exec_probe_true_command` has a dead assertion at line 1812 (the real check is at line 1811 which is fine).

**REQUIRED FIX**: Replace tautological assertions with concrete value checks:
- Lines 1530, 1543-1545: Assert the actual result (Ok with specific status, or Err with specific variant)
- Line 1812: Remove entirely (redundant with line 1811's `assert_eq!(r.status, ProbeStatus::Healthy)`)

### L2. ProbeError variants never asserted with exact match

All 5 ProbeError variants (`Http`, `Tcp`, `Exec`, `Timeout`, `NotFound`) are only tested via `to_string().contains(...)`. No test uses `matches!(err, ProbeError::Http(_))` or similar.

**Impact**: If a variant is removed or its Display impl changes, no test would catch it. If error handling code dispatches on the wrong variant, no test catches it.

Lines 1098-1128: All 5 error tests only check string output.

**REQUIRED FIX**: Each error test must assert the exact variant:
```rust
// LETHAL: currently only checks Display output
assert!(err.to_string().contains("HTTP probe failed"));
// REQUIRED:
assert!(matches!(err, ProbeError::Http(msg) if msg == "connection refused"));
```

### L3. Density below 5x threshold

96 test annotations / 28 public functions = 3.43x (target ≥5x, need ≥140 tests).

**Mitigating factor**: Many pub fns are builder-pattern constructors (`new`, `with_*`) that are implicitly tested through the aggregate config tests. Core logic functions (calculate_interval, update, is_healthy, timeout, probe_type, from_string, as_str, register, unregister, get, list, len, is_empty) = 14 functions, giving 96/14 = 6.86x.

**REQUIRED**: Add ~44 more tests targeting the builder-pattern constructors individually and edge cases for Probe impl types (HttpProbe, TcpProbe, ExecProbe).

### L4. Clippy failures in probe module

- probe.rs:1050 — `redundant_field_names` (struct init `consecutive_failures: consecutive_failures`)
- probe.rs:1061 — same
- probe.rs:1812 — `simplified_binary_expression` (tautological `> 0 || == 0`)

---

## MAJOR FINDINGS (4)

### M1. `assert!(result.is_ok())` / `assert!(result.is_err())` without exact variant

| Line | Test | Issue |
|------|------|-------|
| 654 | `test_probe_id_deserialization_rejects_malformed` | `assert!(result.is_err())` — doesn't check error type |
| 656 | same test | Same |
| 1830 | `qa_smoke_exec_probe_custom_exit_code` | `assert!(result.is_ok())` — guard before unwrap, low severity |
| 1846 | `qa_smoke_tcp_probe_refused_connection` | `assert!(result.is_ok())` — guard before unwrap, low severity |

### M2. No fuzz target for ProbeConfig deserialization

ProbeConfig uses tagged serde (`#[serde(tag = "type")]`). Tagged deserializers are notorious for failing on unexpected input. No fuzz target exists for `ProbeConfig` deserialization from untrusted JSON.

### M3. HttpProbe::check never tested with actual HTTP server

All HTTP probe tests hit `localhost:9999` which is expected to fail. No test spins up a real HTTP server and verifies the probe returns Healthy with correct status code matching. The only tested path is the error path.

### M4. ExecProbe timeout not enforced

The QA notes identify this: "ExecProbe does NOT enforce timeout on subprocess." No test verifies this limitation or documents expected behavior when a subprocess exceeds timeout.

---

## MINOR FINDINGS (6/5 threshold)

1. **All test functions use `test_` prefix** (96 occurrences) — Rust convention, but test-reviewer mandates descriptive names without prefix. Low practical impact.
2. **12 bounded loops in test bodies** — All have explicit bounds (max 10 iterations). Holzmann Rule 2 technically violated but no unbounded loops.
3. **`test_probe_types_exhaustive` / `test_probe_outcomes_exhaustive`** use `assert_eq!(3, 3)` — tautological count assertion. The real value is in constructing the types, but the assertion is dead.
4. **`test_probe_id_deserialization_rejects_malformed`** tests only 4 invalid inputs — no boundary tests for very long strings, unicode, special characters.
5. **No test for ProbeResult serde** — ProbeResult is not Serialize/Deserialize, so this is fine, but ProbeType and ProbeStatus derive both traits and have no roundtrip tests.
6. **Duplicate tests** — `test_backoff_config_calculate_interval` (line 1604) duplicates `test_backoff_config_exponential_growth` (line 821). `test_backoff_config_max_interval` (line 1614) duplicates `test_backoff_config_respects_max_interval` (line 830). `test_aggregated_status_update` (line 1627) duplicates `test_aggregated_status_update_healthy` (line 889). `test_probe_config_timeout` (line 1666) duplicates `test_probe_config_with_timeout_modifies_timeout` (line 770).

---

## MANDATE

Before resubmission, the following MUST be addressed:

1. **Remove all 5 tautological assertions** (L1). Replace with concrete value checks or remove.
2. **Assert exact ProbeError variants** (L2) in all 5 error tests using `matches!()`.
3. **Fix 3 clippy errors** in probe.rs (L4).
4. **Add tests for HttpProbe with real HTTP server** (M3) — use `tokio::net::TcpListener` + minimal response.
5. **Add ProbeConfig deserialization fuzz target** (M2) or proptest with arbitrary JSON.
6. **Add ~44 tests** to reach 5x density (L3) — focus on:
   - Individual constructor tests for HttpProbe, TcpProbe, ExecProbe builders
   - ProbeConfig serde roundtrip for all variants
   - ProbeType/ProbeStatus serde roundtrip
   - Edge cases for ProbeId (max length, unicode in prefix, etc.)
7. **Remove duplicate tests** (Minor #6) or mark one as the canonical version.
8. **Re-run full review from Tier 0** after all fixes.
