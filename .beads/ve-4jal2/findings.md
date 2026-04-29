# Findings: ve-4jal2 — dag-engine: Invalid DAG structure is rejected

## Summary

The DAG validation code in `vo-types/src/workflow/mod.rs` correctly implements:
- Empty workflow detection
- RetryPolicy validation per node
- Edge referential integrity (source and target nodes must exist)
- DFS-based cycle detection

**Bug Fixed**: The `UnknownNode` error variant had misleading semantics. When the source node was unknown, the error message said "references unknown target node" but the `unknown_target` field was set to the source node name.

## Changes Made

### 1. `crates/vo-types/src/workflow/mod.rs`

**Error variant** (line 36-41):
- Renamed field `unknown_target` → `unknown_node`
- Updated error message from "references unknown target node" → "references unknown node"

**Validation code** (lines 142-159):
- Updated error construction to use `unknown_node` instead of `unknown_target`

### 2. `crates/vo-types/src/tests_bdd_dag_cycle_validation.rs`

Updated tests in `edge_integrity` module (lines 784-786, 803-806):
- Changed `unknown_target:` → `unknown_node:`

### 3. `crates/vo-types/src/workflow_tests.rs`

Updated tests (lines 758-761, 779-782, 936-939, 1217-1224):
- Changed `unknown_target:` → `unknown_node:`
- Updated assertion from "unknown target node" → "unknown node"

### 4. `crates/vo-types/src/red_queen_tests.rs`

Updated tests RQ-07 and RQ-08 (lines 157-215):
- Changed `unknown_target:` → `unknown_node:`
- Updated comments to reflect that the error semantics are now correct

## Verification

- Library build: `cargo build -p vo-types` ✓ succeeds
- Pre-existing test compilation errors in `plugin/state_tests.rs` are unrelated to this change

## Semantic Improvement

Before:
```
edge from 'phantom' references unknown target node 'phantom'
```
(misleading - phantom is the source, not target)

After:
```
edge from 'phantom' references unknown node 'phantom'
```
(correct - identifies the unknown node regardless of its role)
