bead_id: vo-ald
bead_title: vo-types: define WorkflowDefinition and DAG node types
phase: state-1.7-test-plan-review-retry-1
updated_at: 2026-03-27T15:30:00Z

# Test Plan Review: vo-ald (Retry 1)

## VERDICT: APPROVED

---

## MANDATE Verification (Previous Rejection Items)

All 6 MANDATE items from the previous review have been verified:

| # | MANDATE Item | Status | Evidence |
|---|-------------|--------|----------|
| 1 | NonEmptyVec::IntoIterator BDD scenarios | **FIXED** | B-63 (line 45), B-64 (line 46) in behavior inventory. Full BDD scenarios at lines 277-291 with exact assertions. Trophy allocation at lines 135-136. |
| 2 | ALL Then clauses use exact values | **FIXED** | B-19 through B-22 now assert all three RetryPolicy fields (e.g., B-19 line 376: `Ok(RetryPolicy { max_attempts: 1, backoff_ms: 100, backoff_multiplier: 1.0 })`). B-29/B-30/B-31 now assert exact workflow_name, node_names, edge structure. B-35/B-36 now specify exact `node_name: NodeName("bad_node")`. B-38 now specifies exact `unknown_target: NodeName("phantom")`. B-45 now asserts full DagNode including retry_policy. B-47 now asserts exact values instead of "identical". B-9 now asserts exact values. No `...`, `where`, or `identical` patterns remain. |
| 3 | 8 new mutation survivability entries | **FIXED** | Mutation table (Section 7, lines 979-1003) now has 23 entries (was 15). New entries 16-18 cover IntoIterator mutations. Entries 19-20 cover workflow_name corruption. Entries 21-23 cover error field corruption. |
| 4 | All 7 test names fixed (no `fn test_` prefix) | **FIXED** | All 65 test function names audited. Zero instances of `test_` prefix found. Previously-flagged names corrected: `parse_linear_3_node_workflow_...` (was `test_parse_...`), `parse_diamond_workflow_...`, `parse_empty_nodes_...`, `parse_cyclic_workflow_...`, `next_nodes_with_on_success_...`, `next_nodes_with_on_failure_...`, `workflow_definition_json_roundtrip()` (was `test_workflow_definition_json_roundtrip()`). |
| 5 | Summary count matches actual behavior count | **FIXED** | Summary (line 10) now says "Behaviors identified: 65". Actual count: B-1 through B-65 = 65. Trophy allocation (line 11): 3 + 43 + 19 = 65. All match. |
| 6 | 3-node cycle BDD added | **FIXED** | B-65 (line 91) in behavior inventory. Full BDD scenario at lines 592-598 asserting exact `CycleDetected { cycle_nodes: vec![NodeName("a"), NodeName("b"), NodeName("c"), NodeName("a")] }`. Trophy allocation at line 167. Combinatorial table at line 1065. |

---

### Axis 1 — Contract Parity

**PASS.**

Public functions/methods from `contract.md` signatures section (13 total):

| # | Function | BDD Coverage |
|---|----------|-------------|
| 1 | `NonEmptyVec::new` | B-1, B-2 |
| 2 | `NonEmptyVec::new_unchecked` | B-9, B-10 |
| 3 | `NonEmptyVec::first` | B-3 |
| 4 | `NonEmptyVec::rest` | B-4 |
| 5 | `NonEmptyVec::as_slice` | B-5 |
| 6 | `NonEmptyVec::into_vec` | B-6 |
| 7 | `NonEmptyVec::len` | B-7 |
| 8 | `NonEmptyVec::is_empty` | B-8 |
| 9 | `NonEmptyVec: IntoIterator` | B-63, B-64 |
| 10 | `RetryPolicy::new` | B-15..B-23 |
| 11 | `WorkflowDefinition::parse` | B-29..B-48 |
| 12 | `WorkflowDefinition::get_node` | B-45, B-46 |
| 13 | `next_nodes` (free fn) | B-49..B-57 |

All 13 public functions have BDD coverage.

Error variant coverage (7 total):

| Variant | BDD Scenarios Asserting Exact Variant |
|---------|--------------------------------------|
| `WorkflowDefinitionError::DeserializationFailed` | B-33, B-34, B-41, B-58 |
| `WorkflowDefinitionError::EmptyWorkflow` | B-32, B-42, B-59 |
| `WorkflowDefinitionError::CycleDetected` | B-39, B-40, B-65, B-60 |
| `WorkflowDefinitionError::UnknownNode` | B-37, B-38, B-61 |
| `WorkflowDefinitionError::InvalidRetryPolicy` | B-35, B-36, B-62 |
| `RetryPolicyError::ZeroAttempts` | B-16, B-18, B-24 |
| `RetryPolicyError::InvalidMultiplier` | B-17, B-25 |

All 7 error variants have scenarios asserting the exact variant (not `is_err()`). Note: B-33, B-34, B-41 use `{ source: _ }` for the `serde_json::Error` field — this is structurally necessary because `serde_json::Error` does not implement `PartialEq`. The variant-level match is exact.

---

### Axis 2 — Assertion Sharpness

**PASS.**

Every `Then:` clause across all 65 BDD scenarios was audited for banned patterns:

| Banned Pattern | Instances Found |
|---------------|-----------------|
| `is_ok()` | 0 |
| `is_err()` | 0 |
| `...` (struct elision) | 0 |
| `where` (vague qualification) | 0 |
| `identical` (without concrete value) | 0 |
| `Some(_)` (unspecified inner) | 0 |
| `> 0` / boolean without concrete value | 0 |

All Then clauses assert concrete values. Notable fixes verified:
- B-19/B-20/B-21/B-22: All three RetryPolicy fields asserted explicitly (lines 376, 385, 393, 401).
- B-29: workflow_name, node_name, retry_policy, edges.len() all asserted (lines 486-491).
- B-30: Exact edge[0] and edge[1] with source/target/condition asserted (lines 502-503).
- B-31: Exact workflow_name, nodes.len(), edges.len(), next_nodes result asserted (lines 513-516).
- B-35/B-36: Exact `node_name: NodeName("bad_node")` asserted (lines 548, 556).
- B-38: Exact `unknown_target: NodeName("phantom")` asserted (line 572).
- B-45: Full DagNode with retry_policy asserted (line 636).
- B-47: Exact values asserted instead of "identical" (line 652).

---

### Axis 3 — Trophy Allocation

**PASS.**

Public function count: 13 (all functions from Axis 1).

BDD scenario count: 65 (B-1 through B-65).

Ratio: 65 / 13 = **5.0x**. Meets the 5x threshold.

| Category | Count | Verification |
|----------|-------|-------------|
| Static (compile-time) | 3 | B-11, B-13, B-26 |
| Unit (Calc layer) | 43 | Verified by trophy table: B-1..B-10(10) + B-12(1) + B-14(1) + B-15..B-25(11) + B-27(1) + B-28(1) + B-45(1) + B-46(1) + B-49..B-57(9) + B-58..B-62(5) + B-63(1) + B-64(1) = 43 |
| Integration (parse pipeline) | 19 | B-29..B-44(16) + B-47(1) + B-48(1) + B-65(1) = 19 |
| E2E (overlap) | 4 | B-30, B-31, B-48, B-57 |

Proptest invariants: 9 — covers RetryPolicy::new (3), NonEmptyVec serde (1), Edge serde (1), WorkflowDefinition parse round-trip (1), next_nodes (2), RetryPolicy serde (1). All pure functions with non-trivial input space have proptest coverage.

Fuzz targets: 2 — `WorkflowDefinition::parse` (line 886) and `NonEmptyVec` serde deserialization (line 911). Both parsers/deserializers have fuzz targets.

Kani harnesses: 3 — RetryPolicy invariants (line 930), NonEmptyVec length (line 945), cycle detection (line 958).

---

### Axis 4 — Boundary Completeness

**PASS.**

**RetryPolicy::new** (all boundaries covered):
| Boundary | Covered By |
|----------|-----------|
| min valid: max_attempts=1 | B-19 |
| max valid: max_attempts=255 | B-21 |
| below min: max_attempts=0 | B-16 |
| min valid: backoff_multiplier=1.0 | B-20 |
| below min: backoff_multiplier<1.0 | B-17 |
| valid: backoff_ms=0 | B-22 |
| negative multiplier | Combinatorial table line 1027 |
| both invalid (priority) | B-18 |

**NonEmptyVec::new** (all boundaries covered):
| Boundary | Covered By |
|----------|-----------|
| min valid: single element | B-3 |
| below min: empty vec | B-2 |
| typical: multiple elements | B-1 |

**NonEmptyVec::IntoIterator** (covered):
| Boundary | Covered By |
|----------|-----------|
| multi-element iteration | B-63 |
| singleton iteration | B-64 |

**WorkflowDefinition::parse** (all boundaries covered):
| Boundary | Covered By |
|----------|-----------|
| min valid: single node | B-29 |
| below min: empty nodes | B-32 |
| malformed JSON | B-33 |
| missing fields | B-34 |
| self-loop cycle | B-40 |
| 2-node cycle | B-39 |
| 3-node cycle | B-65 |
| unknown target | B-37 |
| unknown source | B-38 |

**next_nodes** (all boundaries covered):
| Boundary | Covered By |
|----------|-----------|
| terminal node (no edges) | B-54 |
| multiple successors | B-55 |
| all 6 condition/outcome combos | B-49..B-56 |
| mixed conditions (Always + OnSuccess) | B-56 |
| linear chain | B-57 |

0 functions with >=3 missing boundaries.

---

### Axis 5 — Mutation Survivability

**PASS.**

All 23 critical mutations from Section 7 (lines 979-1003) verified against named catching tests:

| # | Mutation | Caught By | Verified |
|---|----------|-----------|----------|
| 1 | `== 0` changed to `<= 0` in RetryPolicy::new | B-16 | Yes — asserts exact `Err(ZeroAttempts)` |
| 2 | `< 1.0` changed to `<= 1.0` in RetryPolicy::new | B-20 | Yes — asserts `Ok` at exactly 1.0 |
| 3 | Validation order swapped | B-18 | Yes — asserts `ZeroAttempts` when both invalid |
| 4 | `nodes.is_empty()` changed to `nodes.len() > 100` | B-32 | Yes — asserts `EmptyWorkflow` for empty nodes |
| 5 | Cycle detection disabled | B-39 | Yes — asserts `CycleDetected` for a->b->a |
| 6 | Edge check skipped | B-37 | Yes — asserts `UnknownNode` for ghost target |
| 7 | Always match changed to OnSuccess | B-49 (with Failure outcome) | Yes — Always must fire on Failure |
| 8 | OnSuccess match changed to Always | B-51 | Yes — asserts empty vec on Failure |
| 9 | OnFailure match changed to Always | B-53 | Yes — asserts empty vec on Success |
| 10 | get_node returns first regardless | B-46 | Yes — asserts `None` for nonexistent |
| 11 | NonEmptyVec::new accepts empty | B-2 | Yes — asserts `Err` for empty vec |
| 12 | NonEmptyVec::is_empty returns true | B-8 | Yes — asserts `false` |
| 13 | Empty workflow check removed | B-32 | Yes — asserts `EmptyWorkflow` |
| 14 | Cycle check before unknown-node check | B-44 | Yes — asserts `UnknownNode` when both present |
| 15 | InvalidRetryPolicy after UnknownNode | B-43 | Yes — asserts `InvalidRetryPolicy` when both present |
| 16 | IntoIterator yields nothing | B-63 | Yes — asserts `vec![10,20,30]` exact |
| 17 | IntoIterator yields reverse | B-63 | Yes — asserts insertion order |
| 18 | IntoIterator yields only first | B-63 | Yes — asserts 3 elements |
| 19 | workflow_name corrupted (single-node) | B-29 | Yes — asserts `WorkflowName("solo")` |
| 20 | workflow_name corrupted (linear) | B-30 | Yes — asserts `WorkflowName("linear")` |
| 21 | node_name wrong in InvalidRetryPolicy | B-35 | Yes — asserts `NodeName("bad_node")` |
| 22 | node_name wrong (multiplier case) | B-36 | Yes — asserts `NodeName("bad_node")` |
| 23 | unknown_target reports wrong target | B-38 | Yes — asserts `NodeName("phantom")` |

Additional thought-experiment mutations checked:
- `NonEmptyVec::rest()` returns full slice instead of tail → B-4 asserts `&[2, 3]` for `[1,2,3]`. Caught.
- `NonEmptyVec::first()` returns wrong element → B-1 asserts `first() == &1` for `[1,2,3]`. Caught.
- `NonEmptyVec::into_vec()` returns reversed → B-6 asserts `vec![1, 2]`. Caught.
- Cycle detection reports wrong cycle_nodes → B-39/B-40/B-65 assert exact `cycle_nodes` vectors. Caught.
- `next_nodes` returns wrong node for Always edge → B-49 asserts exact node_name "b". Caught.
- Error priority: deser before empty → B-41 asserts exact `DeserializationFailed` variant. Caught.

No uncaught mutations identified.

---

### Axis 6 — Holzmann Plan Audit

**PASS.**

| Rule | Status | Evidence |
|------|--------|----------|
| Rule 1 (Linear) | PASS | All BDD scenarios follow Given → When → Then flow. No nested conditionals in test descriptions. |
| Rule 2 (Bound Every Loop) | PASS | Zero loops proposed in any test body. All iteration delegated to rstest cartesian products or proptest strategies. |
| Rule 4 (One Function, One Job) | PASS | Each BDD scenario tests one logical behavior. Tests with two When/Then blocks (B-54, B-56, B-57) test one behavior with two outcome paths — justified as a single logical assertion. |
| Rule 5 (State Assumptions) | PASS | Every BDD scenario has an explicit `Given:` block stating preconditions with concrete values. |
| Rule 7 (Narrow State) | PASS | No shared mutable state proposed. Each test creates its own fixtures from scratch. |
| Rule 8 (Surface Side Effects) | PASS | Pure domain types — zero I/O, zero filesystem, zero network. All test helpers are pure builders. |

Test naming: All 65 test function names audited. Zero instances of `fn test_`, `fn it_works`, or `fn should_pass` prefix.

---

## Severity Summary

| Severity | Count | Threshold | Result |
|----------|-------|-----------|--------|
| LETHAL | 0 | Any = REJECTED | **PASS** |
| MAJOR | 0 | >=3 = REJECTED | **PASS** |
| MINOR | 1 | >=5 = REJECTED | **PASS** |

---

## LETHAL FINDINGS

None.

---

## MAJOR FINDINGS (0)

None.

---

## MINOR FINDINGS (1)

1. **test-plan.md:265 — B-9 Then clause says `Ok(NonEmptyVec)` for `new_unchecked()` which returns `Self`, not `Result<Self, _>`.** The contract signature (`contract.md:212`) specifies `pub fn new_unchecked(items: Vec<T>) -> Self`. The BDD says `Then: Ok(NonEmptyVec) is returned`. While the subsequent assertions (`nev.first() == &99`, `nev.len() == 1`) make the intent clear, the `Ok()` wrapping is factually incorrect for the return type. The test implementation would need to assert on the returned `NonEmptyVec` directly, not unwrap a `Result`. This is a documentation inconsistency that will be resolved during implementation (compile error if implemented literally) — not a coverage gap.

---

## Notes (non-blocking observations)

1. **Duplicate edges (NG-14)**: Listed in the combinatorial coverage matrix (line 1071) as `Ok(def), both stored` but has no dedicated B- scenario. The combinatorial table serves as an adequate specification for this edge case. NG-14 is a non-goal documenting the absence of deduplication — testing the absence of a feature is not required for approval.

2. **DagNode explicit serde round-trip**: No standalone `serialize(DagNode) -> deserialize -> assert_eq` test. However, DagNode deserialization is exercised in every parse test (B-29 through B-48), DagNode serialization is tested in B-26 (serialize to JSON, verify exact fields), and both component types (NodeName, RetryPolicy) have explicit round-trip tests. Coverage is implicit but adequate.

3. **`get_node` multi-node fixture**: B-45 and B-46 both use the B-29 single-node fixture ("solo" with node "a"). A mutation that always returns the first matching node would be caught by B-46 (asserts `None` for nonexistent name). The function is a trivial `iter().find()` — risk is minimal.

---

## MANDATE

None. The plan meets all requirements for approval.

Previous rejection items:
- [x] NonEmptyVec::IntoIterator BDD scenarios (B-63, B-64)
- [x] All Then clauses use exact values
- [x] 8 new mutation survivability entries (now 23 total)
- [x] All test names free of `fn test_` prefix
- [x] Summary count matches actual behavior count (65)
- [x] 3-node cycle BDD (B-65)
