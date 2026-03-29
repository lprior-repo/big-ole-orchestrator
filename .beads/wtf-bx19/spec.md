# vo-bx19 — dag: Parse graph_raw into DAG node set

```yaml
id: vo-bx19
title: "dag: Parse graph_raw into DAG node set"
status: ready
effort: 2hr
priority: 2
type: feature
crates:
  - vo-actor
  - vo-common
dependencies: []
```

---

## Section 1 — Problem Statement

`WorkflowDefinition.graph_raw` (a `String` field at `crates/vo-common/src/types/workflow.rs:23`) holds the serialized DAG graph, but nothing parses it into `HashMap<NodeId, DagNode>` for `DagActorState::new()`. Currently `initialize_paradigm_state` in `crates/vo-actor/src/instance/state.rs:62` passes an **empty** `HashMap` to `DagActorState::new()`, meaning DAG instances have no nodes and `ready_nodes()` always returns `[]`. This bead creates the parser that bridges `graph_raw` → `DagActorState`.

---

## Section 2 — Scope

**In scope:**
- New `parse` module under `crates/vo-actor/src/dag/parse.rs`
- `parse_dag_graph` function: `&str` → `Result<HashMap<NodeId, DagNode>, DagParseError>`
- Error enum `DagParseError` with structured variants
- Cycle detection (reject cyclic graphs)
- Duplicate node detection
- Missing predecessor detection
- Integration: wire into `initialize_paradigm_state` in `crates/vo-actor/src/instance/state.rs`
- Unit tests covering happy path, error cases, cycle detection

**Out of scope:**
- YAML format support (JSON only for now)
- Schema validation of `activity_type` values against registered types
- DAG visualization or dot-file generation
- Snapshot/recovery path changes

---

## Section 3 — Existing Contracts

### Types consumed (read-only)

```rust
// crates/vo-actor/src/dag/state.rs:10-13
pub struct DagNode {
    pub activity_type: String,
    pub predecessors: Vec<NodeId>,
}

// crates/vo-actor/src/dag/state.rs:17
pub struct NodeId(pub String);

// crates/vo-common/src/types/workflow.rs:18-26
pub struct WorkflowDefinition {
    pub paradigm: WorkflowParadigm,
    pub graph_raw: String,       // <-- input to parser
    pub description: Option<String>,
}
```

### Function consumed (read-only)

```rust
// crates/vo-actor/src/dag/state.rs:56
pub fn DagActorState::new(nodes: HashMap<NodeId, DagNode>) -> Self
```

### Function modified (integration point)

```rust
// crates/vo-actor/src/instance/state.rs:59-68
pub fn initialize_paradigm_state(args: &InstanceArguments) -> ParadigmState {
    match args.paradigm {
        WorkflowParadigm::Dag => ParadigmState::Dag(/* currently empty HashMap */),
        // ...
    }
}
```

---

## Section 4 — Proposed API

### New types

```rust
// crates/vo-actor/src/dag/parse.rs

/// Error returned when parsing a DAG graph definition.
#[derive(Debug, thiserror::Error)]
pub enum DagParseError {
    #[error("invalid JSON in graph_raw: {0}")]
    InvalidJson(String),

    #[error("graph_raw must be a JSON object with a 'nodes' field")]
    MissingNodesField,

    #[error("'nodes' must be an array")]
    NodesNotArray,

    #[error("duplicate node id: {0}")]
    DuplicateNodeId(String),

    #[error("node '{node_id}' has unknown predecessor '{predecessor_id}'")]
    UnknownPredecessor { node_id: String, predecessor_id: String },

    #[error("cycle detected involving nodes: {0}")]
    CycleDetected(String),

    #[error("node at index {index} is missing required field '{field}'")]
    MissingNodeField { index: usize, field: &'static str },
}
```

### New public function

```rust
/// Parse a `graph_raw` JSON string into a `HashMap<NodeId, DagNode>`.
///
/// Expected JSON format:
/// ```json
/// {
///   "nodes": [
///     { "id": "A", "activity_type": "fetch", "predecessors": [] },
///     { "id": "B", "activity_type": "transform", "predecessors": ["A"] }
///   ]
/// }
/// ```
pub fn parse_dag_graph(
    graph_raw: &str,
) -> Result<std::collections::HashMap<NodeId, DagNode>, DagParseError>
```

### Modified function

```rust
// crates/vo-actor/src/instance/state.rs
pub fn initialize_paradigm_state(args: &InstanceArguments) -> ParadigmState {
    match args.paradigm {
        WorkflowParadigm::Dag => {
            let nodes = args
                .workflow_definition
                .as_ref()
                .and_then(|def| crate::dag::parse::parse_dag_graph(&def.graph_raw).ok())
                .unwrap_or_default();
            ParadigmState::Dag(crate::dag::DagActorState::new(nodes))
        }
        // ... unchanged
    }
}
```

---

## Section 5 — graph_raw JSON Schema

The parser accepts a single JSON object with a `"nodes"` array:

```json
{
  "nodes": [
    {
      "id": "fetch_user",
      "activity_type": "http_get",
      "predecessors": []
    },
    {
      "id": "enrich_user",
      "activity_type": "transform",
      "predecessors": ["fetch_user"]
    },
    {
      "id": "notify",
      "activity_type": "send_email",
      "predecessors": ["fetch_user", "enrich_user"]
    }
  ]
}
```

**Field semantics:**
- `id` (string, required): becomes `NodeId(id)`
- `activity_type` (string, required): copied into `DagNode.activity_type`
- `predecessors` (array of strings, required): each element becomes `NodeId` in `DagNode.predecessors`

**Constraints enforced by parser:**
1. Top-level must be `{"nodes": [...]}`  — no extra fields rejected (forward compat)
2. Every node must have `id`, `activity_type`, `predecessors`
3. No duplicate `id` values
4. Every value in `predecessors` must reference an existing node `id`
5. No cycles (topological sort must succeed)

---

## Section 6 — Algorithm

```
parse_dag_graph(graph_raw: &str) -> Result<HashMap<NodeId, DagNode>, DagParseError>

1. Deserialize graph_raw as serde_json::Value
   - Fail: DagParseError::InvalidJson

2. Extract root["nodes"] as array
   - Fail: DagParseError::MissingNodesField / NodesNotArray

3. For each element in nodes array (with index):
   a. Extract "id" as string   -> Fail: MissingNodeField { index, "id" }
   b. Extract "activity_type"  -> Fail: MissingNodeField { index, "activity_type" }
   c. Extract "predecessors" as array of strings
      -> Fail: MissingNodeField { index, "predecessors" }
   d. Insert (NodeId(id), DagNode { activity_type, predecessors }) into HashMap
      -> Fail: DuplicateNodeId(id)

4. Validate all predecessor references exist in node set
   - Fail: UnknownPredecessor { node_id, predecessor_id }

5. Cycle detection via Kahn's algorithm:
   a. Compute in-degree for each node
   b. Queue all zero in-degree nodes
   c. Process queue: decrement successor in-degrees, enqueue newly zero
   d. If processed count < total nodes -> cycle detected
   - Fail: CycleDetected(ordered list of involved node ids)

6. Return Ok(node_map)
```

---

## Section 7 — File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/vo-actor/src/dag/parse.rs` | **CREATE** | `DagParseError` enum, `parse_dag_graph` function |
| `crates/vo-actor/src/dag/mod.rs` | **MODIFY** | Add `pub mod parse;` and `pub use parse::*;` |
| `crates/vo-actor/src/dag/tests.rs` | **MODIFY** | Add parse unit tests (happy path, error variants, cycle) |
| `crates/vo-actor/src/instance/state.rs` | **MODIFY** | Wire `parse_dag_graph` into `initialize_paradigm_state` for `Dag` arm |

---

## Section 8 — Tests

### T1: parse linear dag
```rust
#[test]
fn parse_linear_three_nodes() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"task_a","predecessors":[]},
        {"id":"B","activity_type":"task_b","predecessors":["A"]},
        {"id":"C","activity_type":"task_c","predecessors":["B"]}
    ]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert_eq!(map.len(), 3);
    assert_eq!(map[&NodeId::new("B")].predecessors, vec![NodeId::new("A")]);
}
```

### T2: parse parallel dag
```rust
#[test]
fn parse_parallel_roots() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"t1","predecessors":[]},
        {"id":"B","activity_type":"t2","predecessors":[]},
        {"id":"C","activity_type":"t3","predecessors":["A","B"]}
    ]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert_eq!(map.len(), 3);
}
```

### T3: empty nodes array
```rust
#[test]
fn parse_empty_nodes_yields_empty_map() {
    let json = r#"{"nodes":[]}"#;
    let map = parse_dag_graph(json).expect("parse");
    assert!(map.is_empty());
}
```

### T4: invalid JSON
```rust
#[test]
fn parse_invalid_json() {
    let result = parse_dag_graph("not json");
    assert!(matches!(result, Err(DagParseError::InvalidJson(_))));
}
```

### T5: missing nodes field
```rust
#[test]
fn parse_missing_nodes_field() {
    let result = parse_dag_graph(r#"{"edges":[]}"#);
    assert!(matches!(result, Err(DagParseError::MissingNodesField)));
}
```

### T6: nodes not array
```rust
#[test]
fn parse_nodes_not_array() {
    let result = parse_dag_graph(r#"{"nodes":"oops"}"#);
    assert!(matches!(result, Err(DagParseError::NodesNotArray)));
}
```

### T7: duplicate node id
```rust
#[test]
fn parse_duplicate_node_id() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"t1","predecessors":[]},
        {"id":"A","activity_type":"t2","predecessors":[]}
    ]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(result, Err(DagParseError::DuplicateNodeId(_))));
}
```

### T8: unknown predecessor
```rust
#[test]
fn parse_unknown_predecessor() {
    let json = r#"{"nodes":[
        {"id":"B","activity_type":"t1","predecessors":["NONEXISTENT"]}
    ]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(result, Err(DagParseError::UnknownPredecessor { .. })));
}
```

### T9: cycle detection
```rust
#[test]
fn parse_cycle_detected() {
    let json = r#"{"nodes":[
        {"id":"A","activity_type":"t1","predecessors":["C"]},
        {"id":"B","activity_type":"t2","predecessors":["A"]},
        {"id":"C","activity_type":"t3","predecessors":["B"]}
    ]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(result, Err(DagParseError::CycleDetected(_))));
}
```

### T10: missing required field
```rust
#[test]
fn parse_missing_activity_type_field() {
    let json = r#"{"nodes":[{"id":"A","predecessors":[]}]}"#;
    let result = parse_dag_graph(json);
    assert!(matches!(result, Err(DagParseError::MissingNodeField { field: "activity_type", .. })));
}
```

---

## Section 9 — Invariants

1. **No cycles:** `parse_dag_graph` guarantees the returned graph is acyclic. If it returns `Ok`, topological ordering exists.
2. **Referential integrity:** Every predecessor reference resolves to a node in the returned map.
3. **Unique keys:** `HashMap<NodeId, DagNode>` keys are unique by Rust's type system.
4. **Total nodes match input:** `map.len() == input_nodes_array.len()` on success.
5. **Deterministic:** Same `graph_raw` input always produces the same `HashMap` (no randomness).

---

## Section 10 — Error Taxonomy

| Variant | When | Severity |
|---------|------|----------|
| `InvalidJson` | `graph_raw` is not valid JSON | Fatal (instance cannot start) |
| `MissingNodesField` | Top-level JSON lacks `"nodes"` key | Fatal |
| `NodesNotArray` | `"nodes"` is not a JSON array | Fatal |
| `MissingNodeField` | Node object lacks `id`, `activity_type`, or `predecessors` | Fatal |
| `DuplicateNodeId` | Two nodes share the same `id` | Fatal |
| `UnknownPredecessor` | A predecessor ref doesn't match any node id | Fatal |
| `CycleDetected` | Graph has a cycle | Fatal |

All errors are fatal — `initialize_paradigm_state` falls back to empty map via `.unwrap_or_default()`. A future bead should surface these errors at workflow registration time (API layer validation).

---

## Section 11 — Dependencies

**Workspace crates:**
- `vo-actor` (this is where the code lives)
- `vo-common` (provides `WorkflowDefinition`, but only read for `graph_raw: String`)

**External deps (already in `vo-actor/Cargo.toml`):**
- `serde` + `serde_json` — JSON parsing
- `thiserror` — error derive
- `std::collections::{HashMap, HashSet, VecDeque}` — Kahn's algorithm

**No new crate dependencies required.**

---

## Section 12 — Implementation Notes

- `parse_dag_graph` is a pure function (no I/O, no async). Keep it in `dag/parse.rs` separate from `apply.rs` and `state.rs`.
- Use `serde_json::Value` for manual field extraction rather than a `#[derive(Deserialize)]` struct on the outer wrapper — this gives precise error messages for missing fields.
- Kahn's algorithm needs a successor index (reverse of predecessors). Build it from the parsed nodes: for each node N with predecessor P, add N to `successors[P]`. Use `VecDeque<NodeId>` for the zero-in-degree queue.
- The `DagParseError::CycleDetected` message should include the ids of nodes in the cycle for debugging. Collect them from nodes not processed by Kahn's algorithm.
- Integration in `initialize_paradigm_state`: use `.and_then(...).ok().unwrap_or_default()` to gracefully fall back to empty map if definition is missing or parse fails. This matches the current behavior (empty map) and avoids breaking existing non-DAG tests.
- The `NodeId` wrapper struct has `From<&ActivityId>` but not `From<&str>` — use `NodeId::new(id_str)` which takes `impl Into<String>`.

---

## Section 13 — Checklist

- [ ] Create `crates/vo-actor/src/dag/parse.rs` with `DagParseError` and `parse_dag_graph`
- [ ] Add `pub mod parse;` and `pub use parse::*;` to `crates/vo-actor/src/dag/mod.rs`
- [ ] Wire into `initialize_paradigm_state` in `crates/vo-actor/src/instance/state.rs`
- [ ] Add 10 unit tests to `crates/vo-actor/src/dag/tests.rs`
- [ ] `cargo test -p vo-actor` passes
- [ ] `cargo clippy -p vo-actor -- -D warnings` passes
- [ ] `cargo test --workspace` passes (no regressions)

---

## Section 14 — Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| `graph_raw` format not yet standardized (no existing examples) | Medium | Define JSON schema in Section 5; parser is the canonical definition |
| Empty-map fallback hides parse errors from users | Medium | Acceptable for this bead; follow-up bead adds API-level validation |
| Cycle detection false positive on self-loop | Low | Kahn's algorithm naturally handles self-loops as cycles |
| Breaking existing tests that pass `workflow_definition: None` | Low | `.unwrap_or_default()` preserves current empty-map behavior |

---

## Section 15 — Future Work (Out of Scope)

- **API validation bead:** Reject `graph_raw` at `POST /workflows` registration time, not at instance spawn
- **YAML support:** Add optional YAML parsing alongside JSON
- **DAG linter rules:** Extend `vo-linter` to validate DAG-specific properties (max fan-in, max depth, etc.)
- **Schema validation:** Verify `activity_type` values against registered workflow types

---

## Section 16 — Acceptance Criteria

1. `parse_dag_graph` correctly parses the JSON schema from Section 5 into `HashMap<NodeId, DagNode>`
2. All 7 `DagParseError` variants are reachable with specific test inputs
3. Cycle detection rejects A→B→C→A (and self-loops)
4. `initialize_paradigm_state` for `WorkflowParadigm::Dag` populates `DagActorState.nodes` from `workflow_definition.graph_raw`
5. When `workflow_definition` is `None` or `graph_raw` is invalid, DAG state falls back to empty map (no panic)
6. `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` both pass
7. No new crate dependencies added
