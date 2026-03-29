bead_id: vo-ald
bead_title: vo-types: define WorkflowDefinition and DAG node types
phase: state-1.5-test-planning-retry-1
updated_at: 2026-03-27T12:00:00Z

# Test Plan: vo-ald -- WorkflowDefinition and DAG Node Types

## Summary

- Behaviors identified: 65
- Trophy allocation: 3 static / 43 unit / 19 integration / 4 e2e (overlap)
- Proptest invariants: 9
- Fuzz targets: 2
- Kani harnesses: 3
- Mutation kill target: >= 90%

### Trophy Allocation Rationale

This is a pure-domain crate with zero I/O and zero async. The traditional trophy ratios are adjusted:

| Layer | Count | % | Justification |
|-------|-------|---|---------------|
| Static / Compile-time | 3 | 5% | Compile-time guards (no binary_path, exhaustive enums, no Default, no Hash on f32 types) |
| Unit (Calc layer) | 43 | 66% | Pure constructors (`RetryPolicy::new`, `NonEmptyVec::new`), accessor methods, `IntoIterator`, `next_nodes()`, error display |
| Integration (parse pipeline) | 19 | 29% | `WorkflowDefinition::parse` exercises the full deserialization → validation chain. JSON round-trips. Complex graph scenarios. |
| E2E (acceptance) | 4 | -- | Acceptance tests from the bead spec exercising realistic multi-node workflows end-to-end through `parse` + `next_nodes`. (Overlap with unit/integration.) |

> **Why unit-dominant**: Most behaviors are pure constructors, accessors, and the `next_nodes` function -- zero dependencies, zero I/O. Integration layer covers the complex `parse` pipeline (deserialization + 5-step validation). The 4 E2E tests overlap with unit/integration scenarios but exercise the full `parse` → `next_nodes` chain.

---

## 1. Behavior Inventory

### NonEmptyVec<T>
B-1: NonEmptyVec accepts non-empty vec when constructed via `new()`
B-2: NonEmptyVec rejects empty vec with error when constructed via `new()`
B-3: NonEmptyVec borrows first element when `first()` called
B-4: NonEmptyVec borrows all but first when `rest()` called
B-5: NonEmptyVec borrows full inner slice when `as_slice()` called
B-6: NonEmptyVec consumes into inner vec when `into_vec()` called
B-7: NonEmptyVec returns correct count when `len()` called
B-8: NonEmptyVec `is_empty()` always returns false
B-9: NonEmptyVec constructs without validation when `new_unchecked()` called with non-empty vec
B-10: NonEmptyVec `new_unchecked()` panics when called with empty vec
B-63: NonEmptyVec yields all elements in insertion order when consumed via IntoIterator
B-64: NonEmptyVec yields exactly one element when singleton consumed via IntoIterator

### StepOutcome
B-11: StepOutcome has exactly two variants Success and Failure
B-12: StepOutcome serde round-trips for both variants

### EdgeCondition
B-13: EdgeCondition has exactly three variants Always, OnSuccess, OnFailure
B-14: EdgeCondition serde round-trips for all variants

### RetryPolicy
B-15: RetryPolicy accepts valid parameters when max_attempts >= 1 and backoff_multiplier >= 1.0
B-16: RetryPolicy rejects zero max_attempts with ZeroAttempts error
B-17: RetryPolicy rejects backoff_multiplier < 1.0 with InvalidMultiplier error
B-18: RetryPolicy returns ZeroAttempts when both max_attempts == 0 AND backoff_multiplier < 1.0 (priority)
B-19: RetryPolicy accepts max_attempts == 1 at minimum boundary
B-20: RetryPolicy accepts backoff_multiplier == 1.0 at minimum boundary
B-21: RetryPolicy accepts max_attempts == 255 (u8::MAX) at maximum boundary
B-22: RetryPolicy accepts backoff_ms == 0 (no delay is valid per contract)
B-23: RetryPolicy serde round-trips correctly

### RetryPolicyError
B-24: RetryPolicyError::ZeroAttempts displays "max_attempts must be >= 1, got 0"
B-25: RetryPolicyError::InvalidMultiplier displays the actual value received

### DagNode
B-26: DagNode contains node_name and retry_policy fields only (no binary_path -- compile-time)

### Edge
B-27: Edge holds source_node, target_node, and condition fields
B-28: Edge serde round-trips correctly

### WorkflowDefinition
B-29: parse accepts valid single-node workflow with no edges
B-30: parse accepts valid linear 3-node workflow (a -> b -> c) with Always edges
B-31: parse accepts valid diamond workflow (a -> b, a -> c, b -> d, c -> d)
B-32: parse rejects empty nodes list with EmptyWorkflow error
B-33: parse rejects invalid JSON with DeserializationFailed error
B-34: parse rejects JSON with missing required fields with DeserializationFailed error
B-35: parse rejects node with zero max_attempts with InvalidRetryPolicy error
B-36: parse rejects node with backoff_multiplier < 1.0 with InvalidRetryPolicy error
B-37: parse rejects edge with unknown target node with UnknownNode error
B-38: parse rejects edge with unknown source node with UnknownNode error
B-39: parse rejects cyclic workflow (a -> b -> a) with CycleDetected error
B-40: parse rejects self-loop (a -> a) with CycleDetected error
B-65: parse rejects 3-node cycle (a -> b -> c -> a) with CycleDetected error
B-41: parse error priority: DeserializationFailed before EmptyWorkflow
B-42: parse error priority: EmptyWorkflow before InvalidRetryPolicy
B-43: parse error priority: InvalidRetryPolicy before UnknownNode
B-44: parse error priority: UnknownNode before CycleDetected
B-45: get_node returns Some(&DagNode) when node name exists
B-46: get_node returns None when node name does not exist
B-47: parse is deterministic (same bytes always produce same Ok or Err)
B-48: WorkflowDefinition JSON round-trip: serialize(Ok(parse(json))) == json (structural equality)

### next_nodes()
B-49: next_nodes returns single successor when Always edge from current node
B-50: next_nodes returns successor when OnSuccess edge and outcome is Success
B-51: next_nodes returns empty vec when OnSuccess edge and outcome is Failure
B-52: next_nodes returns successor when OnFailure edge and outcome is Failure
B-53: next_nodes returns empty vec when OnFailure edge and outcome is Success
B-54: next_nodes returns empty vec when node has no outgoing edges (terminal node)
B-55: next_nodes returns multiple successors when multiple edges match (diamond fan-out)
B-56: next_nodes respects mixed edge conditions (Always + OnSuccess both fire on Success)
B-57: next_nodes for linear 3-node workflow returns [b] from node a on any outcome

### WorkflowDefinitionError
B-58: WorkflowDefinitionError::DeserializationFailed displays "workflow definition deserialization failed: ..."
B-59: WorkflowDefinitionError::EmptyWorkflow displays "workflow definition must contain at least one node"
B-60: WorkflowDefinitionError::CycleDetected displays the cycle node names
B-61: WorkflowDefinitionError::UnknownNode displays the edge source and unknown target names
B-62: WorkflowDefinitionError::InvalidRetryPolicy displays the node name and the nested reason

---

## 2. Trophy Allocation

| ID | Behavior | Layer | Justification |
|----|----------|-------|---------------|
| B-1 | NonEmptyVec::new accepts non-empty | unit | Pure constructor, no deps |
| B-2 | NonEmptyVec::new rejects empty | unit | Pure constructor, single branch |
| B-3 | NonEmptyVec::first() borrows first | unit | Accessor, trivial |
| B-4 | NonEmptyVec::rest() borrows tail | unit | Accessor, trivial |
| B-5 | NonEmptyVec::as_slice() borrows all | unit | Accessor, trivial |
| B-6 | NonEmptyVec::into_vec() consumes | unit | Accessor, trivial |
| B-7 | NonEmptyVec::len() returns count | unit | Accessor, trivial |
| B-8 | NonEmptyVec::is_empty() is false | unit | Invariant assertion |
| B-9 | NonEmptyVec::new_unchecked ok | unit | Unsafe-ish constructor |
| B-10 | NonEmptyVec::new_unchecked panics | unit | Guard assertion |
| B-63 | NonEmptyVec::IntoIterator yields all elements | unit | Trait impl, pure iteration |
| B-64 | NonEmptyVec::IntoIterator yields singleton | unit | Trait impl, boundary |
| B-11 | StepOutcome variant exhaustiveness | static | Compile-time match |
| B-12 | StepOutcome serde round-trip | unit | Pure serde, no deps |
| B-13 | EdgeCondition variant exhaustiveness | static | Compile-time match |
| B-14 | EdgeCondition serde round-trip | unit | Pure serde, no deps |
| B-15 | RetryPolicy::new accepts valid | unit | Pure constructor |
| B-16 | RetryPolicy::new rejects zero attempts | unit | Error branch |
| B-17 | RetryPolicy::new rejects low multiplier | unit | Error branch |
| B-18 | RetryPolicy::new priority: zero first | unit | Priority ordering |
| B-19 | RetryPolicy boundary max_attempts=1 | unit | Boundary value |
| B-20 | RetryPolicy boundary multiplier=1.0 | unit | Boundary value |
| B-21 | RetryPolicy boundary max_attempts=255 | unit | Boundary value |
| B-22 | RetryPolicy backoff_ms=0 valid | unit | Boundary value |
| B-23 | RetryPolicy serde round-trip | unit | Pure serde |
| B-24 | RetryPolicyError::ZeroAttempts display | unit | Error display |
| B-25 | RetryPolicyError::InvalidMultiplier display | unit | Error display |
| B-26 | DagNode has no binary_path | static | Compile-time guard |
| B-27 | Edge field access | unit | Struct construction |
| B-28 | Edge serde round-trip | unit | Pure serde |
| B-29 | parse single-node workflow | integration | Full parse pipeline |
| B-30 | parse linear 3-node workflow | integration + e2e | Full parse + acceptance |
| B-31 | parse diamond workflow | integration + e2e | Full parse + acceptance |
| B-32 | parse rejects empty nodes | integration | Full parse pipeline |
| B-33 | parse rejects invalid JSON | integration | Full parse pipeline |
| B-34 | parse rejects missing fields | integration | Full parse pipeline |
| B-35 | parse rejects zero max_attempts | integration | Full parse pipeline |
| B-36 | parse rejects low multiplier | integration | Full parse pipeline |
| B-37 | parse rejects unknown target | integration | Full parse pipeline |
| B-38 | parse rejects unknown source | integration | Full parse pipeline |
| B-39 | parse rejects cycle A->B->A | integration | Full parse pipeline |
| B-40 | parse rejects self-loop A->A | integration | Full parse pipeline |
| B-65 | parse rejects cycle A->B->C->A | integration | Full parse pipeline |
| B-41 | error priority: deser > empty | integration | Ordering invariant |
| B-42 | error priority: empty > retry | integration | Ordering invariant |
| B-43 | error priority: retry > unknown | integration | Ordering invariant |
| B-44 | error priority: unknown > cycle | integration | Ordering invariant |
| B-45 | get_node returns Some | unit | Lookup method |
| B-46 | get_node returns None | unit | Lookup method |
| B-47 | parse is deterministic | integration | Idempotency |
| B-48 | WorkflowDefinition JSON round-trip | integration + e2e | Full pipeline |
| B-49 | next_nodes Always edge | unit | Pure function |
| B-50 | next_nodes OnSuccess + Success | unit | Pure function |
| B-51 | next_nodes OnSuccess + Failure | unit | Pure function |
| B-52 | next_nodes OnFailure + Failure | unit | Pure function |
| B-53 | next_nodes OnFailure + Success | unit | Pure function |
| B-54 | next_nodes terminal node | unit | Pure function |
| B-55 | next_nodes multiple successors | unit | Pure function |
| B-56 | next_nodes mixed conditions | unit | Pure function |
| B-57 | next_nodes linear chain | unit + e2e | Pure function + acceptance |
| B-58 | Error display: DeserializationFailed | unit | Display impl |
| B-59 | Error display: EmptyWorkflow | unit | Display impl |
| B-60 | Error display: CycleDetected | unit | Display impl |
| B-61 | Error display: UnknownNode | unit | Display impl |
| B-62 | Error display: InvalidRetryPolicy | unit | Display impl |

---

## 3. BDD Scenarios

### NonEmptyVec<T>

#### Behavior B-1: NonEmptyVec accepts non-empty vec when constructed
```
Given: a Vec containing [1, 2, 3]
When: NonEmptyVec::new(vec) is called
Then: Ok(NonEmptyVec) is returned with nev.first() == &1 and nev.len() == 3 and nev.as_slice() == &[1, 2, 3]
```
Test: `non_empty_vec_accepts_non_empty_when_constructed()`

#### Behavior B-2: NonEmptyVec rejects empty vec with error when constructed
```
Given: an empty Vec<i32>
When: NonEmptyVec::new(vec) is called
Then: Err("NonEmptyVec must not be empty") is returned
```
Test: `non_empty_vec_rejects_empty_when_constructed()`

#### Behavior B-3: NonEmptyVec borrows first element when first() called
```
Given: NonEmptyVec containing [42]
When: nev.first() is called
Then: &42 is returned
```
Test: `non_empty_vec_returns_first_element_when_first_called()`

#### Behavior B-4: NonEmptyVec borrows all but first when rest() called
```
Given: NonEmptyVec containing [1, 2, 3]
When: nev.rest() is called
Then: &[2, 3] is returned
```
Test: `non_empty_vec_returns_rest_excluding_first_when_rest_called()`

#### Behavior B-5: NonEmptyVec borrows full inner slice when as_slice() called
```
Given: NonEmptyVec containing [10, 20, 30]
When: nev.as_slice() is called
Then: &[10, 20, 30] is returned
```
Test: `non_empty_vec_returns_full_slice_when_as_slice_called()`

#### Behavior B-6: NonEmptyVec consumes into inner vec when into_vec() called
```
Given: NonEmptyVec containing [1, 2]
When: nev.into_vec() is called
Then: vec![1, 2] is returned
```
Test: `non_empty_vec_returns_inner_vec_when_into_vec_called()`

#### Behavior B-7: NonEmptyVec returns correct count when len() called
```
Given: NonEmptyVec containing 5 elements
When: nev.len() is called
Then: 5 is returned
```
Test: `non_empty_vec_returns_element_count_when_len_called()`

#### Behavior B-8: NonEmptyVec is_empty() always returns false
```
Given: any valid NonEmptyVec with exactly 1 element
When: nev.is_empty() is called
Then: false is returned
```
Test: `non_empty_vec_is_empty_always_returns_false_when_called()`

#### Behavior B-9: NonEmptyVec constructs without validation when new_unchecked() with non-empty
```
Given: vec![99]
When: NonEmptyVec::new_unchecked(vec) is called
Then: Ok(NonEmptyVec) is returned with nev.first() == &99 and nev.len() == 1 and nev.as_slice() == &[99]
```
Test: `non_empty_vec_new_unchecked_constructs_when_vec_is_non_empty()`

#### Behavior B-10: NonEmptyVec new_unchecked() panics when called with empty vec
```
Given: an empty Vec
When: NonEmptyVec::new_unchecked(vec) is called
Then: panic occurs (with message containing "NonEmptyVec")
```
Test: `non_empty_vec_new_unchecked_panics_when_vec_is_empty()`

#### Behavior B-63: NonEmptyVec yields all elements in insertion order when consumed via IntoIterator
```
Given: NonEmptyVec containing [10, 20, 30]
When: nev is consumed via for item in nev { collect into Vec }
Then: vec![10, 20, 30] is returned and iteration order matches insertion order
```
Test: `non_empty_vec_yields_all_elements_in_order_when_iterated()`

#### Behavior B-64: NonEmptyVec yields exactly one element when singleton consumed via IntoIterator
```
Given: NonEmptyVec containing [42]
When: iterated via IntoIterator (for item in nev)
Then: exactly one element 42 is yielded
```
Test: `non_empty_vec_yields_single_element_when_singleton_iterated()`

---

### StepOutcome

#### Behavior B-11: StepOutcome has exactly two variants
```
Given: all variants of StepOutcome
When: exhaustive match is performed
Then: exactly Success and Failure exist
```
> This is a compile-time check via `match` exhaustiveness. No runtime test needed -- but a unit test verifying the variant count via `mem::variant_count::<StepOutcome>() == 2` provides runtime confirmation.

Test: `step_outcome_has_exactly_two_variants_when_checked()`

#### Behavior B-12: StepOutcome serde round-trips
```
Given: StepOutcome::Success
When: serialized to JSON then deserialized
Then: StepOutcome::Success is restored
And: same for StepOutcome::Failure
```
Test: `step_outcome_serde_round_trips_for_both_variants()`

---

### EdgeCondition

#### Behavior B-13: EdgeCondition has exactly three variants
```
Given: all variants of EdgeCondition
When: exhaustive match is performed
Then: exactly Always, OnSuccess, OnFailure exist
```
Test: `edge_condition_has_exactly_three_variants_when_checked()`

#### Behavior B-14: EdgeCondition serde round-trips
```
Given: each EdgeCondition variant (Always, OnSuccess, OnFailure)
When: serialized to JSON then deserialized
Then: the same variant is restored (Always -> Always, OnSuccess -> OnSuccess, OnFailure -> OnFailure)
```
Test: `edge_condition_serde_round_trips_for_all_variants()`

---

### RetryPolicy

#### Behavior B-15: RetryPolicy accepts valid parameters
```
Given: max_attempts = 3, backoff_ms = 1000, backoff_multiplier = 2.0
When: RetryPolicy::new(3, 1000, 2.0) is called
Then: Ok(RetryPolicy { max_attempts: 3, backoff_ms: 1000, backoff_multiplier: 2.0 }) is returned
```
Test: `retry_policy_accepts_valid_params_when_all_constraints_satisfied()`

#### Behavior B-16: RetryPolicy rejects zero max_attempts
```
Given: max_attempts = 0, backoff_ms = 100, backoff_multiplier = 1.0
When: RetryPolicy::new(0, 100, 1.0) is called
Then: Err(RetryPolicyError::ZeroAttempts) is returned
```
Test: `retry_policy_rejects_zero_attempts_with_zero_attempts_error_when_max_is_zero()`

#### Behavior B-17: RetryPolicy rejects backoff_multiplier < 1.0
```
Given: max_attempts = 3, backoff_ms = 100, backoff_multiplier = 0.5
When: RetryPolicy::new(3, 100, 0.5) is called
Then: Err(RetryPolicyError::InvalidMultiplier { got: 0.5 }) is returned
```
Test: `retry_policy_rejects_low_multiplier_with_invalid_multiplier_error_when_below_1()`

#### Behavior B-18: RetryPolicy priority: ZeroAttempts wins over InvalidMultiplier
```
Given: max_attempts = 0, backoff_ms = 100, backoff_multiplier = 0.5
When: RetryPolicy::new(0, 100, 0.5) is called
Then: Err(RetryPolicyError::ZeroAttempts) is returned (not InvalidMultiplier)
```
Test: `retry_policy_returns_zero_attempts_when_both_zero_and_low_multiplier()`

#### Behavior B-19: RetryPolicy accepts max_attempts = 1
```
Given: max_attempts = 1, backoff_ms = 100, backoff_multiplier = 1.0
When: RetryPolicy::new(1, 100, 1.0) is called
Then: Ok(RetryPolicy { max_attempts: 1, backoff_ms: 100, backoff_multiplier: 1.0 }) is returned
```
Test: `retry_policy_accepts_max_attempts_1_at_minimum_boundary()`

#### Behavior B-20: RetryPolicy accepts backoff_multiplier = 1.0
```
Given: max_attempts = 1, backoff_ms = 100, backoff_multiplier = 1.0
When: RetryPolicy::new(1, 100, 1.0) is called
Then: Ok(RetryPolicy { max_attempts: 1, backoff_ms: 100, backoff_multiplier: 1.0 }) is returned
```
Test: `retry_policy_accepts_multiplier_1_at_minimum_boundary()`

#### Behavior B-21: RetryPolicy accepts max_attempts = 255
```
Given: max_attempts = 255, backoff_ms = 100, backoff_multiplier = 1.0
When: RetryPolicy::new(255, 100, 1.0) is called
Then: Ok(RetryPolicy { max_attempts: 255, backoff_ms: 100, backoff_multiplier: 1.0 }) is returned
```
Test: `retry_policy_accepts_max_attempts_255_at_maximum_boundary()`

#### Behavior B-22: RetryPolicy accepts backoff_ms = 0
```
Given: max_attempts = 1, backoff_ms = 0, backoff_multiplier = 1.0
When: RetryPolicy::new(1, 0, 1.0) is called
Then: Ok(RetryPolicy { max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0 }) is returned
```
Test: `retry_policy_accepts_backoff_ms_zero_when_no_delay_requested()`

#### Behavior B-23: RetryPolicy serde round-trip
```
Given: a valid RetryPolicy { max_attempts: 5, backoff_ms: 2000, backoff_multiplier: 1.5 }
When: serialized to JSON then deserialized
Then: RetryPolicy { max_attempts: 5, backoff_ms: 2000, backoff_multiplier: 1.5 } is restored
```
Test: `retry_policy_serde_round_trips_for_valid_policy()`

---

### RetryPolicyError

#### Behavior B-24: RetryPolicyError::ZeroAttempts display
```
Given: RetryPolicyError::ZeroAttempts
When: Display::fmt is called
Then: string contains "max_attempts must be >= 1, got 0"
```
Test: `retry_policy_error_zero_attempts_displays_correct_message_when_formatted()`

#### Behavior B-25: RetryPolicyError::InvalidMultiplier display
```
Given: RetryPolicyError::InvalidMultiplier { got: 0.5 }
When: Display::fmt is called
Then: string contains "backoff_multiplier must be >= 1.0, got 0.5"
```
Test: `retry_policy_error_invalid_multiplier_displays_got_value_when_formatted()`

---

### DagNode

#### Behavior B-26: DagNode has no binary_path field (compile-time)
```
Given: the DagNode struct definition
When: a developer attempts to access .binary_path
Then: compilation fails (field does not exist)
```
> This is a compile-time test. Enforced by:
> 1. A `#[cfg(test)]` module containing a function that attempts `let _: () = dagnode.binary_path;` and asserting it does NOT compile (use `compile_fail` or simply document).
> 2. Alternatively: a runtime test that uses `serde_json::to_value(dag_node)` and checks the JSON object does NOT contain a "binary_path" key.
```
Given: a DagNode constructed with node_name = NodeName("a"), retry_policy = RetryPolicy { max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0 }
When: serialized to serde_json::Value
Then: the resulting object does NOT contain key "binary_path"
And: the object contains exactly keys "node_name" and "retry_policy"
And: the "node_name" value is "a"
And: the "retry_policy" value contains keys "max_attempts", "backoff_ms", "backoff_multiplier"
```
Test: `dag_node_has_no_binary_path_field_when_serialized()`

---

### Edge

#### Behavior B-27: Edge holds correct fields
```
Given: source_node = NodeName("a"), target_node = NodeName("b"), condition = EdgeCondition::Always
When: Edge is constructed
Then: edge.source_node == NodeName("a")
And: edge.target_node == NodeName("b")
And: edge.condition == EdgeCondition::Always
```
Test: `edge_holds_source_target_and_condition_when_constructed()`

#### Behavior B-28: Edge serde round-trip
```
Given: an Edge with source_node = NodeName("x"), target_node = NodeName("y"), condition = EdgeCondition::OnSuccess
When: serialized to JSON then deserialized
Then: Edge with source_node = NodeName("x"), target_node = NodeName("y"), condition = EdgeCondition::OnSuccess is restored
```
Test: `edge_serde_round_trips_for_valid_edge()`

---

### WorkflowDefinition::parse

#### Behavior B-29: parse accepts single-node workflow with no edges
```
Given: JSON with workflow_name "solo", nodes = [{node_name: "a", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}], edges = []
When: WorkflowDefinition::parse(json_bytes) is called
Then: Ok(def) is returned
And: def.workflow_name == WorkflowName("solo")
And: def.nodes.len() == 1
And: def.nodes.first().node_name == NodeName("a")
And: def.nodes.first().retry_policy == RetryPolicy { max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0 }
And: def.edges.len() == 0
```
Test: `parse_accepts_single_node_workflow_when_no_edges()`

#### Behavior B-30: parse accepts linear 3-node workflow (acceptance test 1)
```
Given: JSON with workflow_name "linear", nodes = [{node_name: "a", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}, {node_name: "b", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}, {node_name: "c", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}], edges = [{source_node: "a", target_node: "b", condition: "Always"}, {source_node: "b", target_node: "c", condition: "Always"}]
When: WorkflowDefinition::parse(json_bytes) is called
Then: Ok(def) is returned
And: def.workflow_name == WorkflowName("linear")
And: def.nodes.len() == 3
And: def.edges.len() == 2
And: def.edges[0].source_node == NodeName("a") and def.edges[0].target_node == NodeName("b") and def.edges[0].condition == EdgeCondition::Always
And: def.edges[1].source_node == NodeName("b") and def.edges[1].target_node == NodeName("c") and def.edges[1].condition == EdgeCondition::Always
And: next_nodes(&NodeName("a"), StepOutcome::Success, &def) returns a vec containing the DagNode with node_name == "b"
```
Test: `parse_linear_3_node_workflow_a_b_c_succeeds_and_next_nodes_a_returns_b()`

#### Behavior B-31: parse accepts diamond workflow (acceptance test 2)
```
Given: JSON with workflow_name "diamond", nodes = [{node_name: "a", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}, {node_name: "b", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}, {node_name: "c", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}, {node_name: "d", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}], edges = [{source_node: "a", target_node: "b", condition: "Always"}, {source_node: "a", target_node: "c", condition: "Always"}, {source_node: "b", target_node: "d", condition: "OnSuccess"}, {source_node: "c", target_node: "d", condition: "OnSuccess"}]
When: WorkflowDefinition::parse(json_bytes) is called
Then: Ok(def) is returned
And: def.workflow_name == WorkflowName("diamond")
And: def.nodes.len() == 4
And: def.edges.len() == 4
And: next_nodes(&NodeName("a"), StepOutcome::Success, &def) returns a vec containing the DagNodes with node_names "b" and "c"
```
Test: `parse_diamond_workflow_succeeds_and_next_nodes_a_returns_b_and_c()`

#### Behavior B-32: parse rejects empty nodes with EmptyWorkflow (acceptance test 7)
```
Given: JSON with workflow_name "empty", nodes = [], edges = []
When: WorkflowDefinition::parse(json_bytes) is called
Then: Err(WorkflowDefinitionError::EmptyWorkflow) is returned
```
Test: `parse_empty_nodes_returns_empty_workflow()`

#### Behavior B-33: parse rejects invalid JSON with DeserializationFailed
```
Given: byte slice containing "not valid json{{{"
When: WorkflowDefinition::parse(bytes) is called
Then: Err(WorkflowDefinitionError::DeserializationFailed { source: _ }) is returned
```
Test: `parse_rejects_malformed_json_with_deserialization_failed_when_bytes_invalid()`

#### Behavior B-34: parse rejects missing required fields with DeserializationFailed
```
Given: JSON object with only workflow_name "test", missing nodes and edges
When: WorkflowDefinition::parse(json_bytes) is called
Then: Err(WorkflowDefinitionError::DeserializationFailed { source: _ }) is returned
```
Test: `parse_rejects_missing_fields_with_deserialization_failed_when_json_incomplete()`

#### Behavior B-35: parse rejects node with zero max_attempts with InvalidRetryPolicy (acceptance test 8)
```
Given: JSON with workflow_name "bad-retry", nodes = [{node_name: "bad_node", retry_policy: {max_attempts: 0, backoff_ms: 100, backoff_multiplier: 1.0}}], edges = []
When: WorkflowDefinition::parse(json_bytes) is called
Then: Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name: NodeName("bad_node"), reason: RetryPolicyError::ZeroAttempts }) is returned
```
Test: `parse_rejects_zero_max_attempts_with_invalid_retry_policy_when_node_has_zero_attempts()`

#### Behavior B-36: parse rejects node with low backoff_multiplier with InvalidRetryPolicy (acceptance test 9)
```
Given: JSON with workflow_name "bad-retry", nodes = [{node_name: "bad_node", retry_policy: {max_attempts: 3, backoff_ms: 100, backoff_multiplier: 0.5}}], edges = []
When: WorkflowDefinition::parse(json_bytes) is called
Then: Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name: NodeName("bad_node"), reason: RetryPolicyError::InvalidMultiplier { got: 0.5 } }) is returned
```
Test: `parse_rejects_low_multiplier_with_invalid_retry_policy_when_node_has_low_multiplier()`

#### Behavior B-37: parse rejects edge with unknown target node with UnknownNode (acceptance test 6)
```
Given: JSON with workflow_name "test", nodes = [{node_name: "a", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}], edges = [{source_node: "a", target_node: "ghost", condition: "Always"}]
When: WorkflowDefinition::parse(json_bytes) is called
Then: Err(WorkflowDefinitionError::UnknownNode { edge_source: NodeName("a"), unknown_target: NodeName("ghost") }) is returned
```
Test: `parse_rejects_dangling_edge_with_unknown_node_when_target_missing()`

#### Behavior B-38: parse rejects edge with unknown source node with UnknownNode
```
Given: JSON with workflow_name "test", nodes = [{node_name: "b", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}], edges = [{source_node: "phantom", target_node: "b", condition: "Always"}]
When: WorkflowDefinition::parse(json_bytes) is called
Then: Err(WorkflowDefinitionError::UnknownNode { edge_source: NodeName("phantom"), unknown_target: NodeName("phantom") }) is returned
```
Test: `parse_rejects_dangling_edge_with_unknown_node_when_source_missing()`

#### Behavior B-39: parse rejects cyclic workflow with CycleDetected (acceptance test 5)
```
Given: JSON with workflow_name "cycle", nodes = [{node_name: "a", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}, {node_name: "b", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}], edges = [{source_node: "a", target_node: "b", condition: "Always"}, {source_node: "b", target_node: "a", condition: "Always"}]
When: WorkflowDefinition::parse(json_bytes) is called
Then: Err(WorkflowDefinitionError::CycleDetected { cycle_nodes: vec![NodeName("a"), NodeName("b"), NodeName("a")] }) is returned
```
Test: `parse_cyclic_workflow_a_b_a_returns_cycle_detected()`

#### Behavior B-40: parse rejects self-loop with CycleDetected
```
Given: JSON with workflow_name "self-loop", nodes = [{node_name: "a", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}], edges = [{source_node: "a", target_node: "a", condition: "Always"}]
When: WorkflowDefinition::parse(json_bytes) is called
Then: Err(WorkflowDefinitionError::CycleDetected { cycle_nodes: vec![NodeName("a"), NodeName("a")] }) is returned
```
Test: `parse_rejects_self_loop_with_cycle_detected_when_node_edges_to_itself()`

#### Behavior B-65: parse rejects 3-node cycle (A->B->C->A) with CycleDetected
```
Given: JSON with workflow_name "3-cycle", nodes = [{node_name: "a", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}, {node_name: "b", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}, {node_name: "c", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}], edges = [{source_node: "a", target_node: "b", condition: "Always"}, {source_node: "b", target_node: "c", condition: "Always"}, {source_node: "c", target_node: "a", condition: "Always"}]
When: WorkflowDefinition::parse(json_bytes) is called
Then: Err(WorkflowDefinitionError::CycleDetected { cycle_nodes: vec![NodeName("a"), NodeName("b"), NodeName("c"), NodeName("a")] }) is returned
```
Test: `parse_rejects_3_node_cycle_a_b_c_a_with_cycle_detected()`

#### Behavior B-41: error priority -- DeserializationFailed before EmptyWorkflow
```
Given: invalid JSON (not parseable at all): "not valid json{{{"
When: WorkflowDefinition::parse is called
Then: Err(WorkflowDefinitionError::DeserializationFailed { source: _ }) is returned (not EmptyWorkflow)
```
Test: `parse_returns_deserialization_failed_before_empty_workflow_when_json_malformed()`

#### Behavior B-42: error priority -- EmptyWorkflow before InvalidRetryPolicy
```
Given: valid JSON structure with workflow_name "empty", nodes = [], edges = []
When: WorkflowDefinition::parse is called
Then: Err(WorkflowDefinitionError::EmptyWorkflow) is returned (not InvalidRetryPolicy)
```
Test: `parse_returns_empty_workflow_before_invalid_retry_policy_when_nodes_empty()`

#### Behavior B-43: error priority -- InvalidRetryPolicy before UnknownNode
```
Given: JSON with workflow_name "both", nodes = [{node_name: "bad_node", retry_policy: {max_attempts: 0, backoff_ms: 100, backoff_multiplier: 1.0}}], edges = [{source_node: "bad_node", target_node: "ghost", condition: "Always"}]
When: WorkflowDefinition::parse is called
Then: Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name: NodeName("bad_node"), reason: RetryPolicyError::ZeroAttempts }) is returned (not UnknownNode)
```
Test: `parse_returns_invalid_retry_policy_before_unknown_node_when_both_present()`

#### Behavior B-44: error priority -- UnknownNode before CycleDetected
```
Given: JSON with workflow_name "both", nodes = [{node_name: "a", retry_policy: {max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}}], edges = [{source_node: "a", target_node: "ghost", condition: "Always"}, {source_node: "a", target_node: "a", condition: "Always"}]
When: WorkflowDefinition::parse is called
Then: Err(WorkflowDefinitionError::UnknownNode { edge_source: NodeName("a"), unknown_target: NodeName("ghost") }) is returned (not CycleDetected)
```
Test: `parse_returns_unknown_node_before_cycle_detected_when_both_present()`

#### Behavior B-45: get_node returns Some when node exists
```
Given: a parsed WorkflowDefinition from B-29 (workflow_name "solo", node "a")
When: def.get_node(&NodeName("a")) is called
Then: Some(&DagNode { node_name: NodeName("a"), retry_policy: RetryPolicy { max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0 } }) is returned
```
Test: `get_node_returns_some_when_node_name_exists()`

#### Behavior B-46: get_node returns None when node does not exist
```
Given: a parsed WorkflowDefinition from B-29 (workflow_name "solo", node "a")
When: def.get_node(&NodeName("nonexistent")) is called
Then: None is returned
```
Test: `get_node_returns_none_when_node_name_not_found()`

#### Behavior B-47: parse is deterministic
```
Given: the single-node workflow JSON from B-29 (workflow_name "solo", node "a", no edges)
When: WorkflowDefinition::parse(json_bytes) is called twice with the same bytes
Then: both calls return Ok(def) and def.workflow_name == WorkflowName("solo") and def.nodes.len() == 1 and def.edges.len() == 0 and result1 == result2 via PartialEq
```
Test: `parse_returns_identical_result_when_called_twice_with_same_input()`

#### Behavior B-48: WorkflowDefinition JSON round-trip (acceptance test 11)
```
Given: the linear 3-node workflow JSON from B-30 (workflow_name "linear", nodes [a, b, c], edges [a->b Always, b->c Always])
When: parse succeeds, then the WorkflowDefinition is serialized to JSON
Then: the serialized JSON structurally equals the input (same workflow_name "linear", same node names "a", "b", "c", same edge structure with conditions Always)
```
Test: `workflow_definition_json_roundtrip()`

---

### next_nodes()

#### Behavior B-49: next_nodes returns successor when Always edge
```
Given: def with node "a", node "b", edge a->b Always
When: next_nodes(&NodeName("a"), StepOutcome::Success, &def) is called
Then: vec containing the DagNode with node_name "b" is returned
```
Test: `next_nodes_returns_successor_when_always_edge_matches()`

#### Behavior B-50: next_nodes returns successor when OnSuccess edge and Success outcome (acceptance test 3)
```
Given: def with node "a", node "b", edge a->b OnSuccess
When: next_nodes(&NodeName("a"), StepOutcome::Success, &def) is called
Then: vec containing the DagNode with node_name "b" is returned
```
Test: `next_nodes_with_on_success_edge_routes_correctly_on_success_outcome()`

#### Behavior B-51: next_nodes returns empty vec when OnSuccess edge and Failure outcome
```
Given: def with node "a", node "b", edge a->b OnSuccess
When: next_nodes(&NodeName("a"), StepOutcome::Failure, &def) is called
Then: empty Vec is returned
```
Test: `next_nodes_returns_empty_when_on_success_edge_and_failure_outcome()`

#### Behavior B-52: next_nodes returns successor when OnFailure edge and Failure outcome (acceptance test 4)
```
Given: def with node "a", node "b", edge a->b OnFailure
When: next_nodes(&NodeName("a"), StepOutcome::Failure, &def) is called
Then: vec containing the DagNode with node_name "b" is returned
```
Test: `next_nodes_with_on_failure_edge_routes_correctly_on_failure_outcome()`

#### Behavior B-53: next_nodes returns empty vec when OnFailure edge and Success outcome
```
Given: def with node "a", node "b", edge a->b OnFailure
When: next_nodes(&NodeName("a"), StepOutcome::Success, &def) is called
Then: empty Vec is returned
```
Test: `next_nodes_returns_empty_when_on_failure_edge_and_success_outcome()`

#### Behavior B-54: next_nodes returns empty vec for terminal node
```
Given: def with node "z" that has no outgoing edges
When: next_nodes(&NodeName("z"), StepOutcome::Success, &def) is called
Then: empty Vec is returned
And: next_nodes(&NodeName("z"), StepOutcome::Failure, &def) also returns empty Vec
```
Test: `next_nodes_returns_empty_when_node_has_no_outgoing_edges()`

#### Behavior B-55: next_nodes returns multiple successors for diamond fan-out
```
Given: def with nodes [a, b, c], edges a->b Always, a->c Always
When: next_nodes(&NodeName("a"), StepOutcome::Success, &def) is called
Then: vec containing the DagNodes with node_names "b" and "c" is returned (order not guaranteed)
```
Test: `next_nodes_returns_multiple_successors_when_multiple_always_edges()`

#### Behavior B-56: next_nodes respects mixed edge conditions
```
Given: def with nodes [a, b, c], edges a->b Always, a->c OnSuccess
When: next_nodes(&NodeName("a"), StepOutcome::Success, &def) is called
Then: vec containing the DagNodes with node_names "b" and "c" is returned
When: next_nodes(&NodeName("a"), StepOutcome::Failure, &def) is called
Then: vec containing only the DagNode with node_name "b" is returned (only Always edge fires)
```
Test: `next_nodes_returns_correct_nodes_when_mixed_edge_conditions()`

#### Behavior B-57: next_nodes for linear chain returns next hop
```
Given: def with nodes [a, b, c], edges a->b Always, b->c Always
When: next_nodes(&NodeName("a"), StepOutcome::Success, &def) is called
Then: vec containing the DagNode with node_name "b" is returned
When: next_nodes(&NodeName("b"), StepOutcome::Success, &def) is called
Then: vec containing the DagNode with node_name "c" is returned
```
Test: `next_nodes_returns_next_hop_when_linear_chain_traversed()`

---

### WorkflowDefinitionError

#### Behavior B-58: DeserializationFailed display
```
Given: WorkflowDefinitionError::DeserializationFailed { source: serde_json::Error }
When: Display::fmt is called
Then: string contains "workflow definition deserialization failed:"
```
Test: `workflow_definition_error_deserialization_failed_displays_message_when_formatted()`

#### Behavior B-59: EmptyWorkflow display
```
Given: WorkflowDefinitionError::EmptyWorkflow
When: Display::fmt is called
Then: string contains "workflow definition must contain at least one node"
```
Test: `workflow_definition_error_empty_workflow_displays_message_when_formatted()`

#### Behavior B-60: CycleDetected display
```
Given: WorkflowDefinitionError::CycleDetected { cycle_nodes: vec![NodeName("a"), NodeName("b")] }
When: Display::fmt is called
Then: string contains "cycle" and node names "a" and "b"
```
Test: `workflow_definition_error_cycle_detected_displays_node_names_when_formatted()`

#### Behavior B-61: UnknownNode display
```
Given: WorkflowDefinitionError::UnknownNode { edge_source: NodeName("a"), unknown_target: NodeName("ghost") }
When: Display::fmt is called
Then: string contains "a" and "ghost" and "unknown target node"
```
Test: `workflow_definition_error_unknown_node_displays_names_when_formatted()`

#### Behavior B-62: InvalidRetryPolicy display
```
Given: WorkflowDefinitionError::InvalidRetryPolicy { node_name: NodeName("a"), reason: RetryPolicyError::ZeroAttempts }
When: Display::fmt is called
Then: string contains "a" and "max_attempts"
```
Test: `workflow_definition_error_invalid_retry_policy_displays_node_and_reason_when_formatted()`

---

## 4. Proptest Invariants

### Proptest: RetryPolicy::new -- accepted values satisfy invariants
```
Invariant: For all (max_attempts in 1..=255, backoff_ms in 0..=u64::MAX, backoff_multiplier in 1.0..=f32::MAX),
          RetryPolicy::new returns Ok(policy) where policy.max_attempts == max_attempts
          and policy.backoff_multiplier == backoff_multiplier and policy.backoff_ms == backoff_ms

Strategy: (1u8..=255u8, 0u64..=1_000_000u64, 1.0f32..1e10f32)
Anti-invariant: max_attempts = 0 always fails
Anti-invariant: backoff_multiplier < 1.0 always fails
```
Test: `retry_policy_new_proptest_accepted_values_satisfy_invariants()`

### Proptest: RetryPolicy::new -- rejected max_attempts = 0
```
Invariant: RetryPolicy::new(0, any_backoff_ms, any_multiplier) always returns Err(RetryPolicyError::ZeroAttempts)

Strategy: (0u8, 0u64..1_000_000u64, any_valid_f32)
```
Test: `retry_policy_new_proptest_zero_attempts_always_fails()`

### Proptest: RetryPolicy::new -- rejected backoff_multiplier < 1.0
```
Invariant: For max_attempts >= 1 and backoff_multiplier in -1e10..0.9999,
          RetryPolicy::new returns Err(RetryPolicyError::InvalidMultiplier { got })

Strategy: (1u8..=255u8, 0u64, -1e10f32..0.9999f32)
```
Test: `retry_policy_new_proptest_low_multiplier_always_fails()`

### Proptest: NonEmptyVec serde round-trip
```
Invariant: For any NonEmptyVec<Vec<u8>> with 1..=100 elements,
          serialize(NonEmptyVec) -> deserialize -> original == restored

Strategy: proptest::collection::vec(any::<u8>(), 1..=100)
         .prop_map(|v| NonEmptyVec::new_unchecked(v))
```
Test: `non_empty_vec_serde_round_trip_proptest()`

### Proptest: Edge serde round-trip
```
Invariant: For any valid Edge, serialize -> deserialize == original

Strategy: (valid_node_name_strategy(), valid_node_name_strategy(), any::<EdgeCondition>())
```
Test: `edge_serde_round_trip_proptest()`

### Proptest: WorkflowDefinition serde round-trip via parse
```
Invariant: For any valid workflow JSON (1..=10 nodes, 0..=20 edges, no cycles, all edges valid),
          parse(json) succeeds AND serialize(Ok(parse(json))) == parse(serialize(parse(json)))

Strategy: Generate random acyclic graphs with valid node names and retry policies.
          Use a strategy that builds valid JSON strings:
          - 1..=10 nodes with random valid NodeNames and valid RetryPolicies
          - 0..=min(N*(N-1), 20) edges with valid conditions, no cycles (topological order constraint)
```
Test: `workflow_definition_parse_serialize_round_trip_proptest()`

### Proptest: next_nodes always returns nodes from def
```
Invariant: For any valid WorkflowDefinition and any NodeName current that exists in def,
          every &DagNode returned by next_nodes(current, outcome, &def) must be a node
          present in def.nodes (by NodeName equality)

Strategy: (valid_workflow_definition_strategy(), existing_node_index(), any::<StepOutcome>())
```
Test: `next_nodes_always_returns_nodes_from_def_proptest()`

### Proptest: next_nodes result is subset of edge targets
```
Invariant: For any valid WorkflowDefinition, any current node, any outcome,
          the set of NodeNames from next_nodes result equals the set of target_node names
          from edges where source_node == current AND condition matches outcome

Strategy: (valid_workflow_definition_strategy(), existing_node_index(), any::<StepOutcome>())
```
Test: `next_nodes_matches_edge_targets_proptest()`

### Proptest: RetryPolicy serde round-trip
```
Invariant: For any valid RetryPolicy, serialize -> deserialize == original

Strategy: (1u8..=255u8, 0u64..1_000_000u64, 1.0f32..100.0f32)
         .prop_map(|(a, b, m)| RetryPolicy::new(a, b, m).unwrap())
```
Test: `retry_policy_serde_round_trip_proptest()`

---

## 5. Fuzz Targets

### Fuzz Target: WorkflowDefinition::parse
```
Input type: arbitrary &[u8]
Risk class:
  - Panic on malformed JSON (must not panic -- PO-8)
  - Panic on deeply nested or pathological JSON structures
  - Logic error: cycle detection algorithm correctness on complex graphs
  - Logic error: edge referential integrity check missed on edge cases
  - Integer overflow in node count or edge count
Corpus seeds:
  - Valid single-node workflow JSON
  - Valid 3-node linear workflow JSON
  - Valid diamond workflow JSON
  - Empty object "{}"
  - Empty array "[]"
  - Null bytes
  - Very large node count (1000 nodes)
  - Deeply nested JSON (10+ levels)
  - UTF-8 BOM prefix
  - Workflow with many edges (complete DAG)
  - Workflow with duplicate edges (NG-14: allowed)
  - Workflow with very long node names (128 chars)
  - JSON with NaN/Infinity for backoff_multiplier (serde rejects by default -- verify)
```

### Fuzz Target: NonEmptyVec serde deserialization
```
Input type: arbitrary &[u8] (interpreted as JSON)
Risk class:
  - Panic on malformed JSON array
  - Panic on empty array (should fail gracefully via serde, not panic)
Corpus seeds:
  - "[]" (empty array -- must not panic)
  - "[1]" (single element)
  - "[1,2,3]" (multiple elements)
  - "not json"
  - "[null, 1]"
  - Very large array (10000 elements)
```

---

## 6. Kani Harnesses

### Kani Harness: RetryPolicy invariants hold for all u8 inputs
```
Property: For ANY (max_attempts: u8, backoff_ms: u64, backoff_multiplier: f32),
          if RetryPolicy::new returns Ok(p), then p.max_attempts >= 1
          AND p.backoff_multiplier >= 1.0

Bound: Exhaustive for max_attempts (u8 has only 256 values),
       representative samples for backoff_ms and backoff_multiplier

Rationale: These are the core type invariants (I-6, I-7). A bug here would
           allow constructing invalid RetryPolicies that bypass validation.
           Exhaustive proof over u8 is tractable.
```
Harness: `#[kani::proof] fn retry_policy_invariants_hold_for_all_u8()`

### Kani Harness: NonEmptyVec never has length zero after successful construction
```
Property: For any NonEmptyVec<T> returned by NonEmptyVec::new(vec),
          nev.len() >= 1 AND nev.is_empty() == false

Bound: Any Vec<i32> with 0..=256 elements

Rationale: The NonEmptyVec invariant (I-2) is foundational. If broken,
           WorkflowDefinition could have zero nodes, violating the core
           domain constraint.
```
Harness: `#[kani::proof] fn non_empty_vec_len_always_positive_after_new()`

### Kani Harness: Cycle detection completeness
```
Property: For any WorkflowDefinition returned by parse (Ok case),
          the graph has no directed cycle.
          Specifically: no sequence of edges e1, e2, ..., ek exists
          such that e1.source == ek.target.

Bound: Graphs with 1..=8 nodes and 0..=16 edges

Rationale: Cycle detection (I-1) is the most critical validation. A missed
           cycle would allow infinite execution loops at runtime. Formal
           verification is warranted because DFS-based cycle detection has
           subtle edge cases (self-loops, parallel edges, large graphs).
```
Harness: `#[kani::proof] fn parsed_workflow_definition_has_no_cycles()`

---

## 7. Mutation Testing Checkpoints

### Critical mutations that MUST be caught:

| # | Mutation | Location | Caught By |
|---|----------|----------|-----------|
| 1 | `max_attempts == 0` changed to `max_attempts <= 0` | `RetryPolicy::new` | `retry_policy_rejects_zero_attempts_with_zero_attempts_error_when_max_is_zero()` |
| 2 | `backoff_multiplier < 1.0` changed to `backoff_multiplier <= 1.0` | `RetryPolicy::new` | `retry_policy_accepts_multiplier_1_at_minimum_boundary()` |
| 3 | Validation order swapped (multiplier checked before attempts) | `RetryPolicy::new` | `retry_policy_returns_zero_attempts_when_both_zero_and_low_multiplier()` |
| 4 | `nodes.is_empty()` changed to `nodes.len() > 100` | `WorkflowDefinition::parse` | `parse_empty_nodes_returns_empty_workflow()` |
| 5 | Cycle detection disabled (always returns Ok) | `WorkflowDefinition::parse` | `parse_cyclic_workflow_a_b_a_returns_cycle_detected()` |
| 6 | Edge check skipped (unknown target allowed) | `WorkflowDefinition::parse` | `parse_rejects_dangling_edge_with_unknown_node_when_target_missing()` |
| 7 | `EdgeCondition::Always` match changed to `OnSuccess` | `next_nodes` | `next_nodes_returns_successor_when_always_edge_matches()` (with Failure outcome) |
| 8 | `EdgeCondition::OnSuccess` match changed to `Always` | `next_nodes` | `next_nodes_returns_empty_when_on_success_edge_and_failure_outcome()` |
| 9 | `EdgeCondition::OnFailure` match changed to `Always` | `next_nodes` | `next_nodes_returns_empty_when_on_failure_edge_and_success_outcome()` |
| 10 | `get_node` returns first node regardless of name | `WorkflowDefinition::get_node` | `get_node_returns_none_when_node_name_not_found()` |
| 11 | `NonEmptyVec::new` accepts empty vec | `NonEmptyVec::new` | `non_empty_vec_rejects_empty_when_constructed()` |
| 12 | `NonEmptyVec::is_empty()` returns true | `NonEmptyVec::is_empty` | `non_empty_vec_is_empty_always_returns_false_when_called()` |
| 13 | Empty workflow check removed | `WorkflowDefinition::parse` | `parse_empty_nodes_returns_empty_workflow()` |
| 14 | Error priority: cycle check before unknown-node check | `WorkflowDefinition::parse` | `parse_returns_unknown_node_before_cycle_detected_when_both_present()` |
| 15 | InvalidRetryPolicy check after UnknownNode check | `WorkflowDefinition::parse` | `parse_returns_invalid_retry_policy_before_unknown_node_when_both_present()` |
| 16 | `NonEmptyVec::IntoIterator` yields nothing (empty iterator) | `NonEmptyVec` IntoIterator impl | `non_empty_vec_yields_all_elements_in_order_when_iterated()` |
| 17 | `NonEmptyVec::IntoIterator` yields elements in reverse order | `NonEmptyVec` IntoIterator impl | `non_empty_vec_yields_all_elements_in_order_when_iterated()` |
| 18 | `NonEmptyVec::IntoIterator` yields only first element | `NonEmptyVec` IntoIterator impl | `non_empty_vec_yields_all_elements_in_order_when_iterated()` |
| 19 | `workflow_name` field silently corrupted to wrong value in parse result | `WorkflowDefinition::parse` | `parse_accepts_single_node_workflow_when_no_edges()` (asserts def.workflow_name == WorkflowName("solo")) |
| 20 | `workflow_name` field silently corrupted in linear parse result | `WorkflowDefinition::parse` | `parse_linear_3_node_workflow_a_b_c_succeeds_and_next_nodes_a_returns_b()` (asserts def.workflow_name == WorkflowName("linear")) |
| 21 | `node_name` in InvalidRetryPolicy error blames wrong node | `WorkflowDefinition::parse` | `parse_rejects_zero_max_attempts_with_invalid_retry_policy_when_node_has_zero_attempts()` (asserts node_name: NodeName("bad_node")) |
| 22 | `node_name` in InvalidRetryPolicy error blames wrong node (multiplier case) | `WorkflowDefinition::parse` | `parse_rejects_low_multiplier_with_invalid_retry_policy_when_node_has_low_multiplier()` (asserts node_name: NodeName("bad_node")) |
| 23 | `unknown_target` in UnknownNode error reports wrong target | `WorkflowDefinition::parse` | `parse_rejects_dangling_edge_with_unknown_node_when_source_missing()` (asserts unknown_target: NodeName("phantom")) |

### Threshold
**Minimum mutation kill rate: 90%**

All 23 critical mutations above must be caught. Any surviving mutation
indicates a missing test case that must be added before the bead is closed.

---

## 8. Combinatorial Coverage Matrix

### RetryPolicy::new

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path: typical values | max_attempts=3, backoff_ms=1000, multiplier=2.0 | Ok(RetryPolicy{max_attempts: 3, backoff_ms: 1000, backoff_multiplier: 2.0}) | unit |
| boundary: min attempts | max_attempts=1, backoff_ms=100, multiplier=1.0 | Ok(RetryPolicy{max_attempts: 1, backoff_ms: 100, backoff_multiplier: 1.0}) | unit |
| boundary: max attempts | max_attempts=255, backoff_ms=100, multiplier=1.0 | Ok(RetryPolicy{max_attempts: 255, backoff_ms: 100, backoff_multiplier: 1.0}) | unit |
| boundary: min multiplier | max_attempts=1, backoff_ms=100, backoff_multiplier=1.0 | Ok(RetryPolicy{max_attempts: 1, backoff_ms: 100, backoff_multiplier: 1.0}) | unit |
| boundary: zero backoff | max_attempts=1, backoff_ms=0, backoff_multiplier=1.0 | Ok(RetryPolicy{max_attempts: 1, backoff_ms: 0, backoff_multiplier: 1.0}) | unit |
| error: zero attempts | max_attempts=0, backoff_ms=100, backoff_multiplier=1.0 | Err(ZeroAttempts) | unit |
| error: low multiplier | max_attempts=3, backoff_ms=100, backoff_multiplier=0.5 | Err(InvalidMultiplier{got: 0.5}) | unit |
| error: both invalid | max_attempts=0, backoff_ms=100, backoff_multiplier=0.5 | Err(ZeroAttempts) -- priority | unit |
| error: very negative multiplier | max_attempts=1, backoff_ms=0, backoff_multiplier=-100.0 | Err(InvalidMultiplier{got: -100.0}) | unit |
| invariant: any valid | 1<=attempts<=255, multiplier>=1.0 | Ok + all fields preserved | proptest |
| anti-invariant: zero attempts | attempts=0, any multiplier | Err(ZeroAttempts) | proptest |

### NonEmptyVec<T>

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path | vec![1,2,3] | Ok(NonEmptyVec{[1,2,3]}) with first()==&1, len()==3 | unit |
| single element | vec![42] | Ok(NonEmptyVec{[42]}) with first()==&42 | unit |
| empty vec | vec![] | Err("NonEmptyVec must not be empty") | unit |
| first() accessor | vec![10,20] | &10 | unit |
| rest() accessor | vec![10,20,30] | &[20,30] | unit |
| rest() single elem | vec![99] | &[] | unit |
| as_slice() | vec![1,2] | &[1,2] | unit |
| into_vec() | vec![1,2] | vec![1,2] | unit |
| len() | vec![a,b,c] | 3 | unit |
| is_empty() | vec![x] | false | unit |
| new_unchecked panic | vec![] | panic | unit |
| IntoIterator multi | vec![10,20,30] | yields 10, 20, 30 in order | unit |
| IntoIterator singleton | vec![42] | yields exactly 42 | unit |
| serde round-trip | any non-empty vec | identity | proptest |

### WorkflowDefinition::parse

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| single node, no edges | valid JSON, 1 node "solo"/"a" | Ok(def), workflow_name=="solo", nodes.len()==1, edges.len()==0 | integration |
| linear 3-node | valid JSON, "linear", a->b->c | Ok(def), workflow_name=="linear", nodes.len()==3, edges.len()==2 | integration |
| diamond 4-node | valid JSON, "diamond", a->b,c; b,c->d | Ok(def), workflow_name=="diamond", nodes.len()==4, edges.len()==4 | integration |
| empty nodes | valid JSON, nodes=[] | Err(EmptyWorkflow) | integration |
| malformed JSON | "not json" | Err(DeserializationFailed) | integration |
| missing fields | {"workflow_name":"x"} | Err(DeserializationFailed) | integration |
| invalid retry: zero attempts | node "bad_node" with max_attempts=0 | Err(InvalidRetryPolicy{node_name: "bad_node", reason: ZeroAttempts}) | integration |
| invalid retry: low multiplier | node "bad_node" with multiplier=0.5 | Err(InvalidRetryPolicy{node_name: "bad_node", reason: InvalidMultiplier{0.5}}) | integration |
| unknown target | edge from "a" to "ghost" | Err(UnknownNode{edge_source: "a", unknown_target: "ghost"}) | integration |
| unknown source | edge from "phantom" to "b" | Err(UnknownNode{edge_source: "phantom", unknown_target: "phantom"}) | integration |
| cycle: a->b->a | edges form 2-node cycle | Err(CycleDetected{[a,b,a]}) | integration |
| cycle: a->b->c->a | edges form 3-node cycle | Err(CycleDetected{[a,b,c,a]}) | integration |
| self-loop: a->a | edge a->a | Err(CycleDetected{[a,a]}) | integration |
| priority: deser > empty | invalid JSON | Err(DeserializationFailed) | integration |
| priority: empty > retry | empty nodes | Err(EmptyWorkflow) | integration |
| priority: retry > unknown | both present | Err(InvalidRetryPolicy) | integration |
| priority: unknown > cycle | both present | Err(UnknownNode) | integration |
| duplicate edges (NG-14) | two identical edges | Ok(def), both stored | integration |
| determinism | same input twice | result1 == result2 via PartialEq | integration |
| JSON round-trip | parse then serialize | structural equality | integration |

### next_nodes()

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| Always + Success | Always edge a->b | [DagNode "b"] | unit |
| Always + Failure | Always edge a->b | [DagNode "b"] | unit |
| OnSuccess + Success | OnSuccess edge a->b | [DagNode "b"] | unit |
| OnSuccess + Failure | OnSuccess edge a->b | [] | unit |
| OnFailure + Success | OnFailure edge a->b | [] | unit |
| OnFailure + Failure | OnFailure edge a->b | [DagNode "b"] | unit |
| no outgoing edges | terminal node "z" | [] (both outcomes) | unit |
| multiple matches | 2 Always edges a->b, a->c | [DagNode "b", DagNode "c"] | unit |
| mixed conditions | Always a->b + OnSuccess a->c, Failure | [DagNode "b"] (Always only) | unit |
| mixed conditions | Always a->b + OnSuccess a->c, Success | [DagNode "b", DagNode "c"] | unit |
| linear chain | a->b->c, from a | [DagNode "b"] | unit |
| diamond fan-out | a->b,c, from a, Success | [DagNode "b", DagNode "c"] | unit |
| invariant: result subset of def | any valid graph | all returned nodes in def | proptest |

### Error Display (WorkflowDefinitionError)

| Scenario | Variant | Expected Contains | Layer |
|----------|---------|-------------------|-------|
| B-58 | DeserializationFailed | "deserialization failed" | unit |
| B-59 | EmptyWorkflow | "at least one node" | unit |
| B-60 | CycleDetected | "cycle", "a", "b" | unit |
| B-61 | UnknownNode | "a", "ghost", "unknown target node" | unit |
| B-62 | InvalidRetryPolicy | "bad_node", "max_attempts" | unit |

---

## Open Questions

None. The contract is sufficiently specified. All types, invariants, error variants, validation ordering, and non-goals are explicitly documented.

---

## Acceptance Test Cross-Reference

The 11 acceptance tests from the bead spec map to the following BDD scenarios:

| # | Acceptance Test | BDD Scenario IDs |
|---|----------------|------------------|
| 1 | parse_linear_3_node_workflow_a_b_c_succeeds_and_next_nodes_a_returns_b | B-30, B-57 |
| 2 | parse_diamond_workflow_succeeds_and_next_nodes_a_returns_b_and_c | B-31, B-55 |
| 3 | next_nodes_with_on_success_edge_routes_correctly_on_success_outcome | B-50 |
| 4 | next_nodes_with_on_failure_edge_routes_correctly_on_failure_outcome | B-52 |
| 5 | parse_cyclic_workflow_a_b_a_returns_cycle_detected | B-39 |
| 6 | parse_dangling_edge_returns_unknown_node | B-37 |
| 7 | parse_empty_nodes_returns_empty_workflow | B-32 |
| 8 | retry_policy_zero_attempts_returns_zero_attempts | B-16 |
| 9 | retry_policy_backoff_multiplier_below_1_returns_invalid_multiplier | B-17 |
| 10 | dag_node_has_no_binary_path_field | B-26 |
| 11 | workflow_definition_json_roundtrip | B-48 |

All 11 acceptance tests are covered by the plan. Additional scenarios provide exhaustive coverage of error variants, boundary values, edge cases, error priority ordering, IntoIterator behavior, 3-node cycle detection, and invariant preservation.
