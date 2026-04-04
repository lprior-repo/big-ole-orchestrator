# Test Plan: vel-bxpg — DAG Cycle Detection with `--graph` Integration

## Summary
- **Bead**: vo-sdk: Integrate cycle detection with --graph (ADR-022)
- **Behaviors identified**: 14
- **Trophy allocation**: 4 unit / 7 integration / 2 e2e / 1 static
- **Proptest invariants**: 3
- **Fuzz targets**: 2
- **Kani harnesses**: 2
- **Mutation threshold**: ≥90% kill rate

---

## 1. Behavior Inventory

### Dag::build() — Core DAG Construction

1. **Dag builds successfully when graph is acyclic** — Given a DAG with nodes and edges containing no cycles, `Dag::build()` returns `Ok(WorkflowDefinition)` with correct structure.

2. **Dag rejects when graph contains a cycle** — Given a DAG where a node can reach itself via directed edges, `Dag::build()` returns `Err(WorkflowDefinitionError::CycleDetected { cycle_nodes })`.

3. **Dag rejects when graph contains a self-loop** — Given a DAG where node A has an edge to itself, `Dag::build()` returns `Err(WorkflowDefinitionError::CycleDetected { cycle_nodes: [A] })`.

4. **Dag rejects when graph has empty nodes** — Given a DAG with no nodes added, `Dag::build()` returns `Err(WorkflowDefinitionError::EmptyWorkflow)`.

5. **Dag rejects when edge references unknown node** — Given an edge whose `source_node` or `target_node` does not exist in nodes list, `Dag::build()` returns `Err(WorkflowDefinitionError::UnknownNode { edge_source, unknown_target })`.

6. **Dag rejects when node has invalid retry policy** — Given a DagNode containing an invalid RetryPolicy, `Dag::build()` returns `Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name, reason })`.

7. **Dag rejects when JSON deserialization fails** — Given intermediate struct that fails deserialization validation, `Dag::build()` returns `Err(WorkflowDefinitionError::DeserializationFailed { message })`.

### output_graph() — CLI Serialization

8. **output_graph writes valid JSON to stdout when given valid WorkflowDefinition** — Given a valid `WorkflowDefinition`, `output_graph()` writes JSON containing `workflow_name`, `nodes`, and `edges` to stdout.

9. **output_graph returns SerializationFailed when JSON serialization fails** — Given a valid `WorkflowDefinition` that cannot be serialized, `output_graph()` returns `Err(GraphOutputError::SerializationFailed)`.

10. **output_graph returns StdoutUnavailable when stdout is not writable** — Given stdout is closed or unavailable, `output_graph()` returns `Err(GraphOutputError::StdoutUnavailable)`.

### Cycle Detection Invariants

11. **Cycle detection is deterministic — same graph produces same cycle_nodes ordering** — Given a cyclic graph, multiple calls to `detect_cycle` return `cycle_nodes` in the same order.

12. **Cycle detection handles disconnected components with cycles** — Given a graph with multiple disconnected components where one contains a cycle, `detect_cycle` returns `Some(cycle_nodes)` for the cyclic component.

### Integration: CLI --graph flag

13. **--graph flag with valid DAG outputs JSON and exits 0** — Given `--graph` flag and valid DAG, process outputs JSON to stdout and exits with code 0.

14. **--graph flag with cyclic DAG outputs error to stderr and exits 1** — Given `--graph` flag and cyclic DAG, process outputs error message to stderr with exact node names and exits with code 1.

---

## 2. Trophy Allocation

| Behavior | Layer | Rationale |
|----------|-------|-----------|
| Dag::build() acyclic success | Integration | Real DAG construction with actual node/edge types |
| Dag::build() EmptyWorkflow error | Unit | Pure error path, no I/O |
| Dag::build() CycleDetected error | Integration | Real cycle detection via DFS |
| Dag::build() self-loop error | Unit | Specific edge case of cycle detection |
| Dag::build() UnknownNode error | Unit | Validation logic, no I/O |
| Dag::build() InvalidRetryPolicy error | Unit | Error construction, no I/O |
| Dag::build() DeserializationFailed error | Integration | Real serde validation |
| output_graph() success | Integration | Real JSON serialization to stdout |
| output_graph() SerializationFailed | Unit | Can use capture to simulate failure |
| output_graph() StdoutUnavailable | Unit | Can use file wrapper to simulate |
| Cycle detection determinism | Unit (proptest) | Pure function, invariant testing |
| Cycle detection disconnected cycles | Unit | Pure logic path |
| --graph CLI integration (happy) | E2E | Full CLI invocation |
| --graph CLI integration (error) | E2E | Full CLI with error path |

**Target**: ~60% integration (8), ~30% unit (4), ~5% e2e (2), ~5% static (1)

**Static Analysis**: clippy + cargo-deny on the `vel-bxpg` crate catches:
- Unused `Result` handling
- `unwrap()` in graph output path
- Type inference issues

---

## 3. BDD Scenarios

### Behavior: Dag builds successfully when graph is acyclic
```
Given: A Dag with at least one node added via add_node() and edges connecting them
When: Dag::build() is called
Then: Returns Ok(WorkflowDefinition) where workflow_name matches the Dag's name
And: nodes array is non-empty
And: edges array contains all connect() edges with correct source_node, target_node, condition
And: Resulting WorkflowDefinition is itself acyclic (re-running detect_cycle returns None)
```

### Behavior: Dag rejects when graph contains a cycle
```
Given: A Dag where nodes form a cycle A -> B -> C -> A
When: Dag::build() is called
Then: Returns Err(WorkflowDefinitionError::CycleDetected { cycle_nodes })
And: cycle_nodes contains exactly ["A", "B", "C"] in path order (deterministic)
```

### Behavior: Dag rejects when graph contains a self-loop
```
Given: A Dag where node A has a connect() edge to itself
When: Dag::build() is called
Then: Returns Err(WorkflowDefinitionError::CycleDetected { cycle_nodes })
And: cycle_nodes contains exactly ["A"]
```

### Behavior: Dag rejects when graph has empty nodes
```
Given: A Dag with no nodes added (build called on empty)
When: Dag::build() is called
Then: Returns Err(WorkflowDefinitionError::EmptyWorkflow)
```

### Behavior: Dag rejects when edge references unknown node
```
Given: A Dag with node A added, then connect(A, "nonexistent") called
When: Dag::build() is called
Then: Returns Err(WorkflowDefinitionError::UnknownNode { edge_source: "A", unknown_target: "nonexistent" })
```

### Behavior: Dag rejects when node has invalid retry policy
```
Given: A Dag with a node containing RetryPolicy { max_retries: u32::MAX, backoff: NegativeInterval }
When: Dag::build() is called
Then: Returns Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name: "NodeName", reason: RetryPolicyError::NegativeBackoff })
```

### Behavior: Dag rejects when JSON deserialization fails
```
Given: A workflow JSON that fails validation (e.g., condition expression syntax error)
When: The intermediate struct deserialization encounters the error
Then: Returns Err(WorkflowDefinitionError::DeserializationFailed { message: "..." })
```

### Behavior: output_graph writes valid JSON to stdout when given valid WorkflowDefinition
```
Given: A valid WorkflowDefinition with workflow_name "test-workflow", nodes [NodeA, NodeB], edges [EdgeAB]
When: output_graph(&workflow_definition) is called
Then: stdout contains valid JSON with workflow_name == "test-workflow"
And: JSON nodes array has length 2
And: JSON edges array has length 1 with source_node, target_node, condition fields present
And: The JSON is parseable by serde_json::from_str::<WorkflowDefinition>
```

### Behavior: output_graph returns SerializationFailed when JSON serialization fails
```
Given: A WorkflowDefinition containing a type that cannot serialize to JSON (e.g., NaN f64)
When: output_graph() is called
Then: Returns Err(GraphOutputError::SerializationFailed)
```

### Behavior: output_graph returns StdoutUnavailable when stdout is not writable
```
Given: stdout is redirected to a file that is no longer writable (e.g., disk full or closed fd)
When: output_graph() is called
Then: Returns Err(GraphOutputError::StdoutUnavailable)
```

### Behavior: Cycle detection is deterministic — same graph produces same cycle_nodes ordering
```
Given: A cyclic graph G
When: detect_cycle is called on G multiple times
Then: Each call returns Some(cycle_nodes) with identical ordering
And: The ordering is consistent across runs (deterministic DFS)
```

### Behavior: Cycle detection handles disconnected components with cycles
```
Given: A graph with two disconnected components: one acyclic (A->B), one cyclic (C->D->C)
When: detect_cycle is called
Then: Returns Some(cycle_nodes) for the cyclic component C->D->C
And: Does not report the acyclic component
```

### Behavior: --graph flag with valid DAG outputs JSON and exits 0
```
Given: CLI args include --graph and point to a valid workflow file
When: The process runs to completion
Then: stdout contains valid WorkflowDefinition JSON
And: process.exit_code() == 0
```

### Behavior: --graph flag with cyclic DAG outputs error to stderr and exits 1
```
Given: CLI args include --graph and point to a workflow with a cycle
When: The process runs to completion
Then: stderr contains error message with exact node names forming the cycle
And: process.exit_code() == 1
```

---

## 4. Proptest Invariants

### Proptest: detect_cycle — Idempotence
```
Invariant: Running detect_cycle on a WorkflowDefinition produced by Dag::build()
          (which succeeded) always returns None (the result is acyclic).
Strategy: Construct random valid DAGs (nodes: 1-20, edges: 0 to nodes*(nodes-1)/2, no duplicates),
          build() must succeed, then detect_cycle on the result must return None.
Anti-invariant: Graph with edges that form cycles should return Some(cycle_nodes).
```

### Proptest: detect_cycle — Cycle Detection Completeness
```
Invariant: Any graph containing a cycle (according to transitive closure) must be detected.
          Formally: if exists path from A to B and path from B to A, detect_cycle returns Some.
Strategy: Generate graphs with known cycles (self-loops, 2-node mutual edges, N-node loops),
          verify detect_cycle returns Some(cycle_nodes).
Anti-invariant: Acyclic graphs (trees, DAGs) return None.
```

### Proptest: output_graph — Round-trip Serialization
```
Invariant: A WorkflowDefinition that serializes successfully can be deserialized back
          to an equivalent WorkflowDefinition (modulo serialization-specific details like ordering).
Strategy: Generate valid WorkflowDefinition instances, serialize via output_graph logic,
          deserialize via serde_json, verify nodes and edges counts match.
Anti-invariant: WorkflowDefinition with NaN/Infinite f64 values (cannot serialize to JSON).
```

---

## 5. Fuzz Targets

### Fuzz Target: WorkflowDefinition JSON Deserialization
```
Input type: Arbitrary bytes (JSON string)
Risk: Panic (unwrap on parse error), OOM (deeply nested JSON), Logic error (malformed but parses)
Corpus seeds:
  - Valid minimal WorkflowDefinition: {"workflow_name": "x", "nodes": [], "edges": []}
  - Single node, no edges
  - Nodes with various RetryPolicy configurations
  - Edge with various condition values (null, string, object)
  - Self-loop: {"workflow_name": "x", "nodes": [{"name": "A", ...}], "edges": [{"source_node": "A", "target_node": "A", ...}]}
  - Deeply nested JSON (stack overflow risk)
  - Invalid UTF-8 bytes
```

### Fuzz Target: Graph Output JSON Serialization
```
Input type: Arbitrary WorkflowDefinition structs (via custom struct mutator)
Risk: Panic in serde_json serialization, Logic error in JSON structure
Corpus seeds:
  - Minimal valid WorkflowDefinition
  - WorkflowDefinition with maximum node count
  - WorkflowDefinition with special characters in node names
  - WorkflowDefinition with empty strings in required fields
  - WorkflowDefinition with extremely long workflow_name (DoS via memory)
```

---

## 6. Kani Harnesses

### Kani Harness: Dag::build() — Cycle Detection Exhaustiveness
```
Property: For any finite graph with N nodes and E edges, detect_cycle returns:
          - Some(cycle_nodes) if and only if the graph contains a cycle
          - None if and only if the graph is acyclic
Bound: N <= 10 nodes, E <= 20 edges (state space: 2^(N*(N-1)/2) possible edge sets)
Rationale: Cycle detection is critical infrastructure. A false negative (missing a cycle)
           would allow invalid WorkflowDefinitions to be registered, violating ADR-022's
           guarantee that cycles are caught at discovery time. Kani provides formal proof
           that no input within the bound escapes detection.
```

### Kani Harness: output_graph — No Panic on Valid Input
```
Property: For any valid WorkflowDefinition (according to its invariants), output_graph
          returns Ok(()) and does not panic.
Bound: WorkflowDefinition with nodes.len() <= 100, edges.len() <= 500
Rationale: The serialization path must be provably panic-free. A panic in output_graph
           would crash the CLI process, which is unacceptable for user-facing tooling.
           Kani proves no unwrap/expect in the serialization path can fire on valid input.
```

---

## 7. Mutation Testing Checkpoints

### Critical Mutations to Survive

| Function | Mutation | Must Be Caught By |
|----------|----------|-------------------|
| `Dag::build()` | Cycle detection bypassed (return Ok even if cycle exists) | `dag_rejects_when_graph_contains_a_cycle` |
| `Dag::build()` | EmptyWorkflow check removed | `dag_rejects_when_graph_has_empty_nodes` |
| `Dag::build()` | UnknownNode validation removed | `dag_rejects_when_edge_references_unknown_node` |
| `detect_cycle` | Self-loop not detected | `dag_rejects_when_graph_contains_a_self_loop` |
| `detect_cycle` | Returns wrong node in cycle_nodes | `cycle_detection_returns_deterministic_ordering` |
| `output_graph` | Serialization error swallowed (returns Ok) | `output_graph_returns_serialization_failed_when_json_fails` |
| `output_graph` | stdout error swallowed | `output_graph_returns_stdout_unavailable_when_not_writable` |
| CLI integration | Exit code 0 on cycle (instead of 1) | `--graph_flag_with_cyclic_dag_exits_1` |
| CLI integration | Error message missing node names | `--graph_flag_with_cyclic_dag_outputs_node_names` |

**Threshold**: ≥90% mutation kill rate via `cargo mutants`.

---

## 8. Combinatorial Coverage Matrix

### Dag::build() — Result<WorkflowDefinition, WorkflowDefinitionError>

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| Happy path | Valid DAG (1-20 nodes, valid edges) | Ok(WorkflowDefinition) with correct nodes+edges | Integration |
| Error: EmptyWorkflow | No nodes added | Err(WorkflowDefinitionError::EmptyWorkflow) | Unit |
| Error: CycleDetected | Simple cycle A→B→A | Err(WorkflowDefinitionError::CycleDetected { cycle_nodes: ["A","B"] }) | Integration |
| Error: CycleDetected | Self-loop | Err(WorkflowDefinitionError::CycleDetected { cycle_nodes: ["A"] }) | Unit |
| Error: CycleDetected | 3-node cycle A→B→C→A | Err(WorkflowDefinitionError::CycleDetected { cycle_nodes: ["A","B","C"] }) | Integration |
| Error: CycleDetected | Disconnected component cycle | Err with cycle_nodes from cyclic component only | Unit |
| Error: UnknownNode | Edge to non-existent node | Err(WorkflowDefinitionError::UnknownNode { edge_source: "A", unknown_target: "X" }) | Unit |
| Error: InvalidRetryPolicy | Negative backoff interval | Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name: "A", reason: NegativeBackoff }) | Unit |
| Error: DeserializationFailed | JSON failing validation | Err(WorkflowDefinitionError::DeserializationFailed { message: "..." }) | Integration |
| Invariant: Result is acyclic | Any Ok result | detect_cycle on result returns None | Unit (proptest) |

### output_graph() — Result<(), GraphOutputError>

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| Happy path | Valid WorkflowDefinition | Ok(()) with JSON on stdout | Integration |
| Error: SerializationFailed | WorkflowDefinition with f64::NAN | Err(GraphOutputError::SerializationFailed) | Unit |
| Error: StdoutUnavailable | stdout closed | Err(GraphOutputError::StdoutUnavailable) | Unit |
| Invariant: JSON validity | Any Ok result | stdout parses as valid WorkflowDefinition | Integration |

### detect_cycle() — Option<Vec<NodeName>>

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| No cycle | Tree/DAG | None | Unit |
| Self-loop | A→A | Some(["A"]) | Unit |
| 2-node cycle | A→B→A | Some(["A","B"]) | Unit |
| N-node cycle | A→B→C→...→A | Some([...]) with deterministic order | Unit |
| Disconnected + cyclic | A→B (component 1), C→D→C (component 2) | Some(["C","D"]) from cyclic component | Unit |
| Invariant: Determinism | Same graph multiple times | Same cycle_nodes order every time | Unit (proptest) |

---

## Open Questions

1. **Determinism of cycle_nodes ordering**: The contract states "deterministic" but doesn't specify the algorithm (DFS vs Kahn's). Tests will validate consistency but the exact ordering algorithm should be documented. **Resolution**: Document that ordering follows DFS discovery order (first-discovered cycle node first).

2. **Maximum graph size**: No upper bound specified for nodes/edges. For Kani harnesses, we assume N≤10. Should there be a compile-time or runtime limit? **Resolution**: Test with N=1000 to identify any practical limits, add runtime bounds if needed.

3. **JSON serialization of RetryPolicy**: Not specified if RetryPolicy serializes to a human-readable or machine-readable format. Tests assume standard serde serialization. **Resolution**: Tests will use `#[derive(Serialize, Deserialize)]` default behavior.

4. **Stderr format for cycle error**: Contract specifies "error message is printed to stderr containing the exact node names". Not specified if this is structured (JSON) or plain text. **Resolution**: Assume plain text formatted as: `Error: Cycle detected in workflow. Cycle: A -> B -> C. Process exiting.` Tests will match on presence of node names.

---

## Exit Criteria Verification

- [x] Every public API behavior has at least one BDD scenario (14 behaviors → 14 scenarios)
- [x] Every pure function with multiple inputs has at least one proptest invariant (3 invariants)
- [x] Every parsing/deserialization boundary has a fuzz target (2 targets)
- [x] Every error variant in the Error enum has an explicit test scenario:
  - `WorkflowDefinitionError`: 6 variants → 6 scenarios
  - `GraphOutputError`: 3 variants → 3 scenarios
- [x] Mutation threshold target (≥90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
