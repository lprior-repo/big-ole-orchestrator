bead_id: vo-ald
bead_title: vo-types: define WorkflowDefinition and DAG node types
phase: state-4.7-test-suite-review-retry-1
updated_at: 2026-03-27T21:30:00Z

# Test Suite Review: vo-ald -- WorkflowDefinition and DAG Node Types (RE-REVIEW)

## VERDICT: REJECTED

---

## Previous Mandate Verification

| # | Finding | Status | Evidence |
|---|---------|--------|----------|
| LETHAL #1 | `unreachable!()` in Clone impl | **FIXED** | workflow.rs:122-126 — `DeserializationFailed { message: String }` with `#[derive(Clone)]`. No more panicking branch. |
| LETHAL #2 | Missing test: Always edge + Failure outcome | **FIXED** | workflow.rs:1148-1158 — `next_nodes_returns_successor_when_always_edge_and_failure_outcome()` calls `next_nodes` with `StepOutcome::Failure` and asserts `result[0].node_name == "b"`. Kills mutation M1. |
| MAJOR #1 | `#[allow(clippy::expect_used)]` in production code | **FIXED** | non_empty_vec.rs:41-44 — replaced with `unsafe { self.0.get_unchecked(0) }` with SAFETY comment. No `#[allow]` attribute remains. |
| MAJOR #2 | Missing proptest: parse round-trip | **FIXED** | workflow.rs:1473-1553 — `workflow_definition_parse_serialize_round_trip_proptest()` generates random acyclic workflows (1..=5 nodes, topological-order edges) and verifies parse/serialize identity. |
| MAJOR #3 | Missing test: NonEmptyVec empty deserialization | **FIXED** | non_empty_vec.rs:188-195 — `non_empty_vec_deserialization_rejects_empty_json_array_when_input_is_empty()` asserts `result.is_err()` and checks error message. |

All 5 mandate items verified as resolved.

---

### Tier 0 -- Static

**[FAIL] Banned assertions (is_ok/is_err)**
- `non_empty_vec.rs:192` -- `assert!(result.is_err())` inside `non_empty_vec_deserialization_rejects_empty_json_array_when_input_is_empty()`. The test then proceeds to `result.unwrap_err().to_string()` on line 193 and asserts the error message on line 194, so the concrete value IS checked. However, the `is_err()` guard is a banned pattern. Replace with `let err = result.expect_err("should fail");` or `assert!(matches!(result, Err(_)));`. **LETHAL**.

**[PASS] Silent error suppression** -- `non_empty_vec.rs:161` uses `let _ = NonEmptyVec::new_unchecked(...)` inside a `#[should_panic]` test. `new_unchecked` returns `Self` (not `Result`), so this is not error suppression -- it's the expression that triggers the panic. Reclassified: PASS.

**[PASS] Ignored tests** -- none found.

**[PASS] Sleep in tests** -- none found.

**[PASS] Test naming violations** -- no `fn test_`, `fn it_works`, `fn should_pass` found.

**[PASS] Holzmann Rule 2 (loops in test bodies)** -- all `for` loops in test code iterate over bounded collections:
- workflow.rs:430, 465 -- fixed arrays of enum variants (2 and 3 elements)
- workflow.rs:1436 -- `for node in &result` bounded by small fixed graph in proptest
Production code loops (lines 188, 203, 281, 284, 331) are not test code.

**[PASS] Holzmann Rule 7 (shared mutable state)** -- none found.

**[PASS] Mock interrogation** -- no mocks found.

**[PASS] Integration test purity** -- no `/tests/` directory; all tests are inline `#[cfg(test)]` modules.

**[PASS] Error variant completeness** -- all variants tested with exact assertions:

| Variant | Test(s) | Assertion type |
|---------|---------|---------------|
| `RetryPolicyError::ZeroAttempts` | B-16 (line 499), B-18 (line 516) | `assert_eq!(result, Err(RetryPolicyError::ZeroAttempts))` |
| `RetryPolicyError::InvalidMultiplier { got }` | B-17 (line 506) | `assert_eq!(result, Err(RetryPolicyError::InvalidMultiplier { got: 0.5 }))` |
| `WorkflowDefinitionError::DeserializationFailed { .. }` | B-33 (line 790), B-34 (line 800), B-41 (line 978) | `matches!(...)` + negative match |
| `WorkflowDefinitionError::EmptyWorkflow` | B-32 (line 775), B-42 (line 993) | `assert_eq!(result, Err(WorkflowDefinitionError::EmptyWorkflow))` |
| `WorkflowDefinitionError::CycleDetected { cycle_nodes }` | B-39 (line 895), B-40 (line 925), B-65 (line 945) | `assert_eq!` with exact `cycle_nodes` vec |
| `WorkflowDefinitionError::UnknownNode { edge_source, unknown_target }` | B-37 (line 853), B-38 (line 874), B-44 (line 1029) | `assert_eq!` with exact fields |
| `WorkflowDefinitionError::InvalidRetryPolicy { node_name, reason }` | B-35 (line 811), B-36 (line 832), B-43 (line 1008) | `assert_eq!` with exact fields |

**[PASS] Density audit** -- 77 tests / 12 public functions = 6.42x (target >= 5x).

**[PASS] Production code audit** -- no `#[allow]` attributes, no `unwrap()`/`expect()`, no `unreachable!()`/`panic!()`/`todo!()` in non-test code. The `unsafe { self.0.get_unchecked(0) }` at non_empty_vec.rs:44 has a valid SAFETY comment.

---

### Tier 1 -- Execution

**[FAIL] Clippy: 7 warnings in bead code (19 total, 12 pre-existing from vo-acb)**

Bead-specific clippy failures:

| File:Line | Lint | Description |
|-----------|------|-------------|
| non_empty_vec.rs:200 | unused_imports | `use proptest::prelude::*` is unused -- the `proptest!` macro generates its own `use` |
| non_empty_vec.rs:145 | bool_assert_comparison | `assert_eq!(nev.is_empty(), false)` should be `assert!(!nev.is_empty())` |
| workflow.rs:350/1560 | items_after_test_module | `edge_matches_outcome` helper function defined AFTER `mod tests { ... }` -- must be moved before the test module |
| workflow.rs:431 | needless_borrows_for_generic_args | `&variant` in `serde_json::to_value(&variant)` -- Copy type, borrow unnecessary |
| workflow.rs:470 | needless_borrows_for_generic_args | Same pattern |
| workflow.rs:590 | needless_borrows_for_generic_args | Same pattern |
| workflow.rs:1420 | needless_borrows_for_generic_args | Same pattern |

**[PASS] Tests: 560 passed, 0 failed, 0 ignored, 0 flaky** (cargo test -p vo-types --lib)

**[SKIP] Ordering probe** -- nextest not available; single-threaded execution confirmed by standard cargo test.

**[SKIP] Insta** -- insta not in Cargo.toml.

---

### Tier 2 -- Coverage (manual audit, not blocked by Tier 1 failure)

**Coverage per public function (bead-specific):**

| Function | Tests covering it | Status |
|----------|-------------------|--------|
| `NonEmptyVec::new` | B-1, B-2, proptests | PASS |
| `NonEmptyVec::new_unchecked` | B-9, B-10 | PASS |
| `NonEmptyVec::first` | B-1, B-3 | PASS |
| `NonEmptyVec::rest` | B-4, singleton test | PASS |
| `NonEmptyVec::as_slice` | B-1, B-5 | PASS |
| `NonEmptyVec::into_vec` | B-6 | PASS |
| `NonEmptyVec::len` | B-1, B-7 | PASS |
| `NonEmptyVec::is_empty` | B-8 | PASS |
| `NonEmptyVec::IntoIterator` | B-63, B-64 | PASS |
| `RetryPolicy::new` | B-15..B-22, 3 proptests | PASS |
| `WorkflowDefinition::parse` | B-29..B-48, round-trip proptest | PASS |
| `WorkflowDefinition::get_node` | B-45, B-46 | PASS |
| `next_nodes` | B-49..B-57 (including B-49b), 2 proptests | PASS |

**Line coverage estimate: ~98%** (all production code paths exercised by tests; only dead-code branches like `Some(2) => None` in DFS state machine are not directly asserted but are exercised transitively).

---

### Tier 3 -- Mutation Analysis (manual, not blocked by Tier 1 failure)

**All 23 critical mutations from test plan checkpoint table are now killed:**

| # | Mutation | Killed By | Verified |
|---|----------|-----------|----------|
| 1 | `== 0` to `<= 0` | B-16 | Yes |
| 2 | `< 1.0` to `<= 1.0` | B-20 | Yes |
| 3 | Validation order swap | B-18 | Yes |
| 4 | `is_empty()` to `> 100` | B-32 | Yes |
| 5 | Cycle detection disabled | B-39 | Yes |
| 6 | Edge check skipped | B-37 | Yes |
| 7 | `Always => true` to `Always => (outcome == Success)` | **B-49b** (NEW) | Yes |
| 8 | `OnSuccess` to `Always` | B-51 | Yes |
| 9 | `OnFailure` to `Always` | B-53 | Yes |
| 10 | `get_node` returns first | B-46 | Yes |
| 11 | `NonEmptyVec::new` accepts empty | B-2 | Yes |
| 12 | `is_empty()` returns true | B-8 | Yes |
| 13 | Empty workflow check removed | B-32 | Yes |
| 14 | Cycle before unknown-node check | B-44 | Yes |
| 15 | InvalidRetryPolicy after UnknownNode | B-43 | Yes |
| 16 | IntoIterator yields nothing | B-63 | Yes |
| 17 | IntoIterator reversed | B-63 | Yes |
| 18 | IntoIterator yields only first | B-63 | Yes |
| 19-20 | workflow_name corrupted | B-29, B-30 | Yes |
| 21-22 | InvalidRetryPolicy wrong node | B-35, B-36 | Yes |
| 23 | UnknownNode wrong target | B-38 | Yes |

**Additional mutation verified killed:**
- NonEmptyVec deserialization accepts empty array -> KILLED by `non_empty_vec_deserialization_rejects_empty_json_array_when_input_is_empty()` (non_empty_vec.rs:188)

**No surviving mutants identified.** Kill rate estimate: 100% (24/24 critical mutations).

---

### LETHAL FINDINGS (8)

1. **non_empty_vec.rs:192** -- `assert!(result.is_err())` is a banned assertion pattern. The test correctly checks the error message on line 194, but the `is_err()` guard must be replaced. Fix: `let err = result.expect_err("should fail for empty array");` then assert on `err.to_string()`.

2. **non_empty_vec.rs:200** -- Clippy `unused_imports`: `use proptest::prelude::*;` inside the `proptests` submodule is unused because the `proptest!` macro auto-imports. Fix: delete the line.

3. **non_empty_vec.rs:145** -- Clippy `bool_assert_comparison`: `assert_eq!(nev.is_empty(), false)` should be `assert!(!nev.is_empty())`.

4. **workflow.rs:350/1560** -- Clippy `items_after_test_module`: `fn edge_matches_outcome()` at line 1560 is defined after `mod tests { ... }` at line 350. Fix: move `edge_matches_outcome` before the `mod tests` block (or inside it).

5. **workflow.rs:431** -- Clippy `needless_borrows_for_generic_args`: `serde_json::to_value(&variant)` -- `StepOutcome` is `Copy`, borrow is unnecessary. Fix: `serde_json::to_value(variant)`.

6. **workflow.rs:470** -- Clippy `needless_borrows_for_generic_args`: same pattern with `EdgeCondition`. Fix: `serde_json::to_value(variant)`.

7. **workflow.rs:590** -- Clippy `needless_borrows_for_generic_args`: `serde_json::to_value(&policy)` -- `RetryPolicy` is `Copy`. Fix: `serde_json::to_value(policy)`.

8. **workflow.rs:1420** -- Clippy `needless_borrows_for_generic_args`: same pattern in proptest. Fix: `serde_json::to_value(policy)`.

### MAJOR FINDINGS (0)

None.

### MINOR FINDINGS (1/5 threshold)

1. **workflow.rs:993-1005** -- Test `parse_returns_empty_workflow_before_invalid_retry_policy_when_nodes_empty` (B-42) claims to verify error priority (EmptyWorkflow before InvalidRetryPolicy), but the test input has empty nodes with no retry policies to validate. This makes it identical to B-32's test. The priority assertion is vacuously true because there is no InvalidRetryPolicy to race against. The test name is misleading. (Carried forward from previous review MINOR #2.)

---

### MANDATE

Before resubmission, ALL of the following must be resolved:

1. **FIX non_empty_vec.rs:192** -- Replace `assert!(result.is_err())` with `let err = result.expect_err("should fail for empty array");` and use `err` directly on line 193 (remove the `result.unwrap_err()` call).

2. **FIX non_empty_vec.rs:200** -- Delete the unused `use proptest::prelude::*;` line.

3. **FIX non_empty_vec.rs:145** -- Change `assert_eq!(nev.is_empty(), false)` to `assert!(!nev.is_empty())`.

4. **FIX workflow.rs:1560** -- Move `fn edge_matches_outcome()` to BEFORE the `mod tests { ... }` block (before line 349), or move it inside the test module.

5. **FIX workflow.rs:431, 470, 590, 1420** -- Remove unnecessary `&` borrows on Copy types passed to `serde_json::to_value()`.

After all fixes: re-run ALL tiers from Tier 0. Full re-run. Always.
