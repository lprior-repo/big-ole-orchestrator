# BLACKHAT Audit Findings: vo-sdk/dag.rs — Connect Same Pair Twice

## Issue
`ve-b4bz0` · BLACKHAT: vo-sdk — dag.rs — Connect same pair twice

## Audit Target
`crates/vo-sdk/src/dag.rs` — `Dag::connect()` method

## Finding: Duplicate Edges Allowed (Design Smell)

### Location
`crates/vo-sdk/src/dag.rs:96-105`

```rust
pub fn connect<T>(
    &mut self,
    from: &NodeHandle<impl Any, T>,
    to: &NodeHandle<T, impl Any>,
) -> Result<(), DagError> {
    let from_idx = self.find_index(from.name())?;
    let to_idx = self.find_index(to.name())?;
    self.edges.push((from_idx, to_idx));  // No duplicate check!
    Ok(())
}
```

### Evidence
Test `connect_same_node_twice_creates_two_edges` (dag_tests.rs:259-269) asserts that calling `connect` with the same node pair twice creates 2 edges, with comment "duplicate edges should be allowed".

### Analysis

**Current behavior:** `connect()` appends `(from_idx, to_idx)` to `self.edges` without checking if that pair already exists.

**Problem:** A DAG semantically represents unique relationships between nodes. Allowing duplicate edges between the same pair is likely a bug:

1. **Semantic ambiguity** — What does "connect A→B twice" mean? It's unclear if this is intentional (multi-edge graph) or accidental (programming error).

2. **Silent data corruption** — If a caller accidentally calls `connect()` twice (e.g., retry logic, buggy code), they get duplicate edges with no warning.

3. **Downstream assumption violation** — Downstream code consuming `WorkflowSpec` may assume edge uniqueness.

4. **Cycle detection impact** — While Kahn's algorithm still works with duplicate edges, they could cause confusion in cycle path reporting.

**Not a security vulnerability** — This is a correctness/design issue.

### Recommendation

Two valid approaches:

1. **Add duplicate-edge error** — Return `DagError::DuplicateEdge { from, to }` if the edge already exists:
   ```rust
   if self.edges.contains(&(from_idx, to_idx)) {
       return Err(DagError::DuplicateEdge { ... });
   }
   ```

2. **Add idempotent mode** — A separate `connect_unique()` that is idempotent (no error on duplicate, just no-op).

The existing test would need updating to reflect the new behavior.

### Verdict
**Issue confirmed as design smell.** No code changes made as this is a blackhat audit bead (findings only).
