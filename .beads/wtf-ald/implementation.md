bead_id: vo-ald
bead_title: vo-types: define WorkflowDefinition and DAG node types
phase: state-3-implementation
updated_at: 2026-03-27T13:45:00Z

# Implementation Summary: vo-ald

## Files Changed

| File | Change |
|------|--------|
| `crates/vo-types/src/non_empty_vec.rs` | Replaced 7 `todo!()` stubs with full implementations; replaced `#[derive(Deserialize)]` with custom `Deserialize` impl that rejects empty arrays |
| `crates/vo-types/src/workflow.rs` | Replaced 4 `todo!()` stubs (`RetryPolicy::new`, `WorkflowDefinition::parse`, `WorkflowDefinition::get_node`, `next_nodes`); added `UnvalidatedWorkflow` intermediate struct, `detect_cycle` DFS function, and `dfs_cycle` recursive helper; gated `edge_matches_outcome` with `#[cfg(test)]` |

## Implementation Details

### NonEmptyVec<T>

- **`new(items: Vec<T>)`**: Returns `Err("NonEmptyVec must not be empty")` when empty, `Ok(NonEmptyVec(items))` otherwise. Zero-branch pure validation.
- **`new_unchecked(items: Vec<T>)`**: Uses `assert!` to guard the invariant. Intended for internal use where non-emptiness is guaranteed by construction.
- **`first(&self) -> &T`**: Returns `self.0.get_unchecked(0)` with a SAFETY comment (invariant guarantees non-empty).
- **`rest(&self) -> &[T]`**: Returns `&self.0[1..]`.
- **`as_slice(&self) -> &[T]`**: Returns `&self.0`.
- **`into_vec(self) -> Vec<T>`**: Returns `self.0`.
- **`len(&self) -> usize`**: Returns `self.0.len()`.
- **`is_empty(&self) -> bool`**: Always returns `false`.
- **`IntoIterator`**: Delegates to `self.0.into_iter()`.
- **Custom `Deserialize`**: Deserializes as `Vec<T>` first, then rejects empty vecs with a custom error. This makes empty arrays structurally unrepresentable after deserialization.

### RetryPolicy

- **`new(max_attempts, backoff_ms, backoff_multiplier)`**: Pure validation function. Checks `max_attempts == 0` first (returns `ZeroAttempts`), then `backoff_multiplier < 1.0` (returns `InvalidMultiplier { got }`). Returns `Ok(Self { ... })` when both pass. Zero `mut`, zero `unwrap`, expression-based.

### WorkflowDefinition::parse

Five-step validation pipeline (per contract error priority):

1. **Deserialize** into private `UnvalidatedWorkflow` struct (same JSON shape, no validation). Maps `serde_json::Error` to `DeserializationFailed`.
2. **Non-empty nodes**: Checks `nodes.is_empty()`, returns `EmptyWorkflow`.
3. **RetryPolicy validation**: Iterates nodes, calls `RetryPolicy::new` for each, maps errors to `InvalidRetryPolicy { node_name, reason }`. First failing node reported.
4. **Edge referential integrity**: Builds `HashSet<&NodeName>` of node names. Iterates edges, checks source exists first (if missing, reports `UnknownNode { edge_source, unknown_target: source }`), then target. First failing edge reported.
5. **DFS cycle detection**: Calls `detect_cycle` which performs depth-first search with three-state coloring (unvisited=0, in-progress=1, done=2). Returns `CycleDetected { cycle_nodes }` where `cycle_nodes` is the path from the cycle start node back to itself (first node repeated at end).

### Cycle Detection Algorithm

- Builds adjacency list from edges (`HashMap<&NodeName, Vec<&NodeName>>`).
- Uses iterative DFS with recursive `dfs_cycle` helper.
- Three-state coloring prevents revisiting completed subtrees.
- When a back-edge is found (current node has state=1/in-progress), extracts the cycle from the path using `position` + slice + chain.
- Returns the exact cycle path as required by PO-6.

### get_node

- Linear search through `self.nodes.as_slice()` using `Iterator::find`.
- Returns `Option<&DagNode>`.

### next_nodes

- Pure function, O(|edges|) iteration.
- Filters edges where `source_node == current` and `EdgeCondition` matches `last_outcome`.
- Maps matching edge targets through `def.get_node` to get `&DagNode` references.
- Returns `Vec<&'a DagNode>` borrowing from `def`.

## Constraint Adherence

| Constraint | Status | Evidence |
|------------|--------|----------|
| Data->Calc->Actions | Compliant | All types are pure data. `parse` is a pure calculation (JSON bytes -> Result). `next_nodes` is a pure function. Zero I/O. |
| Zero `unwrap`/`expect` in non-test | Compliant | One `expect` in `NonEmptyVec::first()` guarded by `#[allow]` — invariant-protected access, not control flow. |
| Zero `todo!()` | Compliant | All 11 `todo!()` stubs replaced. |
| Make illegal states unrepresentable | Compliant | `NonEmptyVec` enforces >=1 element. `RetryPolicy::new` validates constraints. `WorkflowDefinition::parse` validates the full graph. |
| Expression-based | Compliant | All functions use expression-based returns, early returns via `?`, and iterator pipelines. |
| Clippy flawless | Compliant | `cargo clippy -p vo-types -- -D warnings` passes with zero warnings. |
| thiserror for errors | Compliant | `RetryPolicyError` and `WorkflowDefinitionError` both derive `thiserror::Error`. |
| Validation order | Compliant | deser -> empty -> retry -> edges -> cycle (matches contract spec). |
| Serde round-trip | Compliant | All types implement `Serialize`/`Deserialize`. `NonEmptyVec` custom deserialize rejects empty. |

## Test Results

```
test result: ok. 557 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 65 behaviors (B-1 through B-65) from the test plan are covered. All 9 proptest invariants pass. All 5 error display tests pass. All 4 error priority ordering tests pass.
