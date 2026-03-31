# ADR-027 Test Suite Inquisition Report

**STATUS: REJECTED**

**Date:** 2026-03-31
**Scope:** `crates/vo-types/src/events.rs` — ADR-027 Deterministic Event-Sourced Replay
**Changed variants:** `WorkflowStarted`, `StepScheduled`, `StepCompleted`, `StepFailed`
**Mode:** Suite Inquisition (Mode 2)

---

## VERDICT: REJECTED

3 LETHAL findings stop the audit at Tier 2. 8 MAJOR findings in ADR-027 specific
field coverage. The new required fields (`binary_hash`, `attempt`, `execution_id`)
have NO error-path tests. Deleting their decode lines would NOT be caught by any
existing test. This is not a test suite. It is a collection of happy-path demos.

---

### Tier 0 — Static

| Check | Result |
|-------|--------|
| Banned assertions (`is_ok()`/`is_err()`) | **PASS** — No hits |
| Silent error discard (`let _ =` / `.ok();`) | **PASS** — No hits in scope |
| Ignored tests (`#[ignore]`) | **PASS** — No hits |
| Sleep in tests | **PASS** — No hits |
| Naming violations (`fn test_`) | **PASS** — No hits |
| Holzmann: loops in test bodies | **PASS** — No loops (proptest regression file is seed data, not code) |
| Holzmann: shared mutable state | **PASS** — No hits |
| Mock interrogation | **PASS** — No mocks |
| Integration test purity | **PASS** — No `use crate::` in `tests/` |
| Error variant completeness | **FAIL** — `SerializationError(String)` at events.rs:52 has zero test assertions; dead code variant |
| Density audit | **PASS** — 808 tests / 75 pub fn = 10.8x (target ≥5x) |
| Insta | **N/A** — Not present in Cargo.toml |

---

### Tier 1 — Execution

| Gate | Result |
|------|--------|
| Clippy (`--tests --all-features -D warnings`) | **FAIL** — 5 errors (unused `#[must_use]` return values in `integer_types_tests.rs`) |
| Tests pass | **PASS** — 951 passed, 0 failed, 0 flaky |
| Ordering probe | **PASS** — Consistent results single-threaded and multi-threaded |
| Insta staleness | **N/A** |

**Clippy failures (LETHAL):**
- `crates/vo-types/src/integer_types_tests.rs:69` — `SequenceNumber::new_unchecked(0)` unused must_use
- `crates/vo-types/src/integer_types_tests.rs:142` — `EventVersion::new_unchecked(0)` unused must_use
- `crates/vo-types/src/integer_types_tests.rs:208` — `AttemptNumber::new_unchecked(0)` unused must_use
- `crates/vo-types/src/integer_types_tests.rs:291` — `TimeoutMs::new_unchecked(0)` unused must_use
- `crates/vo-types/src/integer_types_tests.rs:653` — `MaxAttempts::new_unchecked(0)` unused must_use

Note: These are **pre-existing** (not introduced by ADR-027) but still block the gate.

---

### Tier 2 — Coverage

| Metric | Result |
|--------|--------|
| Total line coverage | **FAIL** — 89.90% (1852/2060 lines) — BELOW 90% threshold |
| events.rs line coverage | **FAIL** — 83.06% (461/555 lines) — **WELL BELOW** 90% |
| events.rs region coverage | 80.45% (745/926 regions) |
| events.rs function coverage | 84.51% (60/71 functions — 11 unexecuted) |
| Branch coverage | Not reported (branch instrumentation = 0 across all files) |

**LETHAL:** events.rs line coverage 83.06% is 7 full percentage points below the 90%
threshold. 94 lines are uncovered. The total crate coverage at 89.90% also fails.

---

### Tier 3 — Mutation

`cargo-mutants` not installed. Manual mutation analysis performed.

**Manual mutation analysis on ADR-027 new field decode logic:**

| Mutation | Caught? | Evidence |
|----------|---------|----------|
| Delete `binary_hash` decode line 228 | **NO** | No test sends WorkflowStarted without binary_hash through error path |
| Delete `attempt` decode line 246 (StepScheduled) | **NO** | No test sends StepScheduled without attempt through error path |
| Delete `execution_id` decode line 247 | **NO** | No test sends StepScheduled without execution_id through error path |
| Delete `attempt` decode line 268 (StepFailed) | **NO** | No test sends StepFailed without attempt through error path |
| Delete `dag_topology` default-to-Null lines 224-227 | **NO** | Test provides `{}`, never omits the field |
| Delete `output` default-to-Null lines 258-261 | **NO** | Test provides `null`, never omits the field |
| Replace `as u32` with `0` on line 246 | **NO** | Only test value is 1, no boundary test |
| Replace `as u32` with `0` on line 268 | **NO** | Only test value is 1, no boundary test |

**Estimated kill rate for ADR-027 decode mutations: 0/8 = 0%** — CATASTROPHIC

Every single new field decode path is an uncaught mutant. The tests prove the happy
path works for ONE specific input value. They do not prove the field is required,
that its absence produces the correct error, that the type coercion is correct, or
that optional defaults behave correctly.

---

### LETHAL FINDINGS

1. **events.rs:83.06% line coverage** — 94 uncovered lines in the file under review.
   The Calc layer target is ≥95% for pure functions. This is 12 points below that.

2. **Clippy: 5 warnings promoted to errors** — `integer_types_tests.rs:69,142,208,291,653`.
   Pre-existing, but blocks the gate. `#[must_use]` return values silently discarded
   in test code means the test is not asserting the constructed value.

3. **Total crate line coverage 89.90%** — Below the 90% minimum threshold.

---

### MAJOR FINDINGS (8) — ADR-027 Required Field Error Gaps

1. **Missing error test: `WorkflowStarted.binary_hash`** — The `payload_invalid_fields`
   rstest (lines 979-1036) covers missing/wrong-type for `workflow_id` but NEVER tests
   `binary_hash` missing → `MissingPayloadField("binary_hash")`. If the `require_string`
   call on line 228 is deleted, no test fails.

2. **Missing error test: `StepScheduled.attempt`** — rstest cases 17-20 (lines 994-997)
   cover `workflow_id` and `step_id` errors but stop there. No case tests `attempt`
   missing → `MissingPayloadField("attempt")`. No case tests `attempt: "bad"` →
   `InvalidPayloadField("attempt must be an integer")`.

3. **Missing error test: `StepScheduled.execution_id`** — Same gap. After `step_id`
   the rstest stops. No case tests `execution_id` missing or wrong type.

4. **Missing error test: `StepFailed.attempt`** — rstest cases 31-36 (lines 1010-1015)
   cover through `failure_reason` but never test `attempt` missing/wrong type.

5. **Untested default: `WorkflowStarted.dag_topology`** — Line 535 test provides
   `"dag_topology": {}` explicitly. No test omits the field to verify the
   `unwrap_or(serde_json::Value::Null)` default on line 227.

6. **Untested default: `StepCompleted.output`** — Line 617 test provides
   `"output": null` explicitly. No test omits the field to verify the
   `unwrap_or(serde_json::Value::Null)` default on line 261.

7. **Weak assertion in `decode_event` integration test** — events.rs:768 uses
   `assert!(matches!(payload, EventPayload::WorkflowStarted { .. }))`. The `..`
   pattern accepts ANY field values. This test passes even if `binary_hash` and
   `dag_topology` are completely wrong or missing from the decode result.

8. **No overflow boundary test for `attempt: u32` cast** — Lines 246 and 268 use
   `require_u64(obj, "attempt")? as u32` with `#[allow(clippy::cast_possible_truncation)]`.
   No test sends `attempt: 4294967296` (u32::MAX + 1) or `attempt: u64::MAX` to
   verify behavior. The cast silently truncates.

---

### MINOR FINDINGS (3)

1. **`SerializationError(String)` dead variant** — events.rs:52. Defined in the Error
   enum but never constructed by any code path. No test. Should either be removed
   or have a usage + test.

2. **No structural validation for `dag_topology`** — The field accepts any
   `serde_json::Value`. No test verifies it rejects non-object topology, or that
   the replay system handles malformed topology correctly.

3. **No proptest for `try_from_json`** — This is a deserializer with 12 variant
   branches and complex field extraction. The only parameterized tests are the
   `payload_invalid_fields` rstest (which misses the new fields — see MAJOR 1-4)
   and the `proptest_version_support_is_consistent` which only tests version numbers.
   A proper proptest generating random JSON objects would catch the gaps.

---

### MANDATE

Before resubmission, ALL of the following must exist:

**Required tests (by name, one per surviving mutant):**

1. `payload_try_from_json_returns_missing_payload_field_when_binary_hash_is_absent`
   — Input: `{"type": "WorkflowStarted", "workflow_id": "w1", "version": 1}`
   — Expected: `Err(Error::MissingPayloadField("binary_hash".to_string()))`

2. `payload_try_from_json_returns_missing_payload_field_when_attempt_is_absent_for_step_scheduled`
   — Input: `{"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "execution_id": "e1", "version": 1}`
   — Expected: `Err(Error::MissingPayloadField("attempt".to_string()))`

3. `payload_try_from_json_returns_invalid_payload_field_when_attempt_is_not_integer_for_step_scheduled`
   — Input: `{"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": "bad", "execution_id": "e1", "version": 1}`
   — Expected: `Err(Error::InvalidPayloadField("attempt must be an integer".to_string()))`

4. `payload_try_from_json_returns_missing_payload_field_when_execution_id_is_absent`
   — Input: `{"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": 1, "version": 1}`
   — Expected: `Err(Error::MissingPayloadField("execution_id".to_string()))`

5. `payload_try_from_json_returns_missing_payload_field_when_attempt_is_absent_for_step_failed`
   — Input: `{"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": "err", "version": 1}`
   — Expected: `Err(Error::MissingPayloadField("attempt".to_string()))`

6. `payload_try_from_json_defaults_dag_topology_to_null_when_absent`
   — Input: `{"type": "WorkflowStarted", "workflow_id": "w1", "binary_hash": "abc123", "version": 1}` (no dag_topology)
   — Expected: `Ok(EventPayload::WorkflowStarted { dag_topology: Value::Null, .. })`

7. `payload_try_from_json_defaults_output_to_null_when_absent`
   — Input: `{"type": "StepCompleted", "workflow_id": "w1", "step_id": "s1", "completed_at_ms": 1000, "version": 1}` (no output)
   — Expected: `Ok(EventPayload::StepCompleted { output: Value::Null, .. })`

8. `decode_event_returns_correct_binary_hash_and_dag_topology_in_full_pipeline`
   — Fix line 768: replace `assert!(matches!(..))` with concrete field value assertions

9. `payload_try_from_json_handles_attempt_overflow_gracefully`
   — Input: `attempt: 4294967296` (u32::MAX + 1)
   — Expected: concrete documented behavior (either error or accepted truncated value)

**Required fixes:**

10. Fix 5 clippy errors in `integer_types_tests.rs` — either assert the returned value
    or add `#[allow(unused_must_use)]` with a comment explaining why the test deliberately
    discards the result.

11. Remove `SerializationError(String)` from the Error enum (dead code) or add a code
    path that constructs it + a test asserting the exact variant.

12. Run `cargo llvm-cov` after fixes and verify events.rs ≥ 90% line coverage.

After ALL fixes: re-run ALL tiers from Tier 0. Full re-run. Always.
