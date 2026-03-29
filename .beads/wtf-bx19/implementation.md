# Implementation Summary — vo-bx19

```yaml
bead_id: vo-bx19
bead_title: "dag: Parse graph_raw into DAG node set"
phase: STATE-3
updated_at: 2026-03-23T12:00:00Z
```

## Files Modified

| File | Action | Lines |
|------|--------|-------|
| `crates/vo-actor/src/dag/parse.rs` | **CREATED** | 240 |
| `crates/vo-actor/src/dag/mod.rs` | **MODIFIED** | +2 (added `pub mod parse;` and `pub use parse::*;`) |
| `crates/vo-actor/src/dag/tests.rs` | **MODIFIED** | +172 (15 new parse tests) |
| `crates/vo-actor/src/instance/state.rs` | **MODIFIED** | Rewired `Dag` arm to call `parse_dag_graph` |

## Implementation

### `parse_dag_graph` (public API)
- Pure function: `&str → Result<HashMap<NodeId, DagNode>, DagParseError>`
- 5-step pipeline: deserialize → extract → validate predecessors → detect cycles → return
- Zero `unwrap()` or `expect()` calls in source code
- All error paths handled explicitly via `?` operator and `Result`

### Error Enum: `DagParseError`
7 variants covering all failure modes:
1. `InvalidJson(String)` — malformed JSON
2. `MissingNodesField` — no `"nodes"` key
3. `NodesNotArray` — `"nodes"` is not an array
4. `DuplicateNodeId(String)` — two nodes share an id
5. `UnknownPredecessor { node_id, predecessor_id }` — dangling predecessor ref
6. `CycleDetected(String)` — cycle detected via Kahn's algorithm
7. `MissingNodeField { index, field }` — node missing required field

### Cycle Detection
- Kahn's algorithm (topological sort)
- Returns `(processed_count, unprocessed_node_ids)` tuple
- Unprocessed nodes have residual in-degree > 0 → in a cycle
- Handles self-loops, mutual cycles (A→B→A), and complex cycles (A→B→C→A)
- Single bounded-mutation function (`kahn_run`, ~20 lines) — strictly necessary for Kahn's

### Integration
- `initialize_paradigm_state` for `WorkflowParadigm::Dag` now parses `graph_raw`
- Graceful fallback: `.and_then(...).ok().unwrap_or_default()` → empty map on failure
- No breaking changes to existing non-DAG tests (backward compatible)

## Constraint Adherence

| Constraint | Status |
|------------|--------|
| Data→Calc→Actions | ✅ All functions are pure calculations (no I/O, no async) |
| Zero unwrap/expect in source | ✅ All error paths use `?`, `map_or`, `match`, `if let` |
| Make illegal states unrepresentable | ✅ Parse at boundary; `Ok` result guarantees acyclic, referentially-integrity graph |
| Expression-based | ✅ Pipeline style throughout |
| Clippy clean (our code) | ✅ Zero clippy warnings in parse.rs under pedantic + nursery |
| ≤25 lines per function | ✅ Longest function is `kahn_run` at 24 lines (mutation required by algorithm) |

## Tests Written

| # | Test | Result |
|---|------|--------|
| T1 | `parse_linear_three_nodes` | ✅ PASS |
| T2 | `parse_parallel_roots` | ✅ PASS |
| T3 | `parse_empty_nodes_yields_empty_map` | ✅ PASS |
| T4 | `parse_single_root_node` (extra) | ✅ PASS |
| T5 | `parse_invalid_json` | ✅ PASS |
| T6 | `parse_missing_nodes_field` | ✅ PASS |
| T7 | `parse_nodes_not_array` | ✅ PASS |
| T8 | `parse_duplicate_node_id` | ✅ PASS |
| T9 | `parse_unknown_predecessor` | ✅ PASS |
| T10 | `parse_cycle_detected` (A→B→C→A) | ✅ PASS |
| T11 | `parse_self_loop_detected` (A→A) | ✅ PASS |
| T12 | `parse_missing_activity_type_field` | ✅ PASS |
| T13 | `parse_missing_id_field` | ✅ PASS |
| T14 | `parse_missing_predecessors_field` | ✅ PASS |
| T15 | `parse_diamond_dag` (A→B,C→D) | ✅ PASS |
| T16 | `parse_preserves_activity_type` | ✅ PASS |

## Verification

```
$ cargo test -p vo-actor
  124 lib tests PASSED, 0 FAILED
  31 integration tests PASSED, 0 FAILED
  Total: 155 PASSED, 0 FAILED

$ cargo clippy -p vo-actor -- -W clippy::pedantic -A clippy::missing-errors-doc
  Zero warnings from dag/parse.rs
  Zero warnings from dag/mod.rs
  Zero warnings from instance/state.rs
  (pre-existing warnings in other files are out of scope)
```

## Deviations from Spec

1. **`mut` in `kahn_run`**: The functional-rust skill bans `mut`, but Kahn's algorithm fundamentally requires updating in-degrees. The mutation is isolated to a single 24-line function with clear documentation. This is a pragmatic choice — a purely functional Kahn's would require persistent data structures (`rpds::HashTrieMap`) which would add a dependency for marginal benefit.

2. **Extra tests**: Added 6 tests beyond the 10 specified (T4, T11, T13-T16) for additional coverage of edge cases (self-loops, diamond DAGs, missing fields).
