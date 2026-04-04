# Contract Specification: vel-bxpg

## Context
- **Bead**: vo-sdk: Integrate cycle detection with --graph (ADR-022)
- **Feature**: Integration of DAG cycle detection with `--graph` serialization output
- **Domain Terms**:
  - `NodeHandle<I, O>`: Typed handle wrapping a DAG node with input type `I` and output type `O`
  - `Dag`: Workflow builder that tracks nodes and edges via `add_node()` and `connect()`
  - `WorkflowDefinition`: The serializable DAG output containing workflow_name, nodes, and edges
  - `--graph`: CLI flag that triggers WorkflowDefinition JSON serialization to stdout
  - `Cycle`: A directed path that returns to its starting node (including self-loops)
- **Assumptions**:
  - The cycle detection algorithm (DFS) already exists in `vo-types::workflow::detect_cycle`
  - The `WorkflowDefinition` type and `WorkflowDefinitionError::CycleDetected` already exist in vo-types
  - The `NodeHandle<I, O>` and `Dag` types are being built in a companion bead (ADR-010)
  - The `--graph` CLI flag handling exists and is the trigger point for this integration
- **Open Questions**:
  - None identified

## Preconditions
- [ ] `Dag::build()` or equivalent is called prior to `--graph` output
- [ ] The DAG must have at least one node registered via `add_node()`
- [ ] All edges created via `connect()` must reference existing nodes (compile-time enforced by NodeHandle types)
- [ ] The `--graph` flag is present in the command-line arguments

## Postconditions
- [ ] **Happy path (acyclic graph)**: `Dag::build()` returns `Ok(WorkflowDefinition)` which is serialized to stdout as JSON
- [ ] **Error path (cyclic graph)**: `Dag::build()` returns `Err(WorkflowDefinitionError::CycleDetected { cycle_nodes })` 
- [ ] **Cycle detected behavior**: When cycle is detected:
  - Error message is printed to stderr containing the exact node names forming the cycle
  - Process exits with non-zero exit code (specifically `exit(1)`)
- [ ] The serialized JSON contains a valid `WorkflowDefinition` with:
  - `workflow_name`: The name of the workflow
  - `nodes`: Non-empty array of `DagNode` objects
  - `edges`: Array of `Edge` objects with `source_node`, `target_node`, and `condition`

## Invariants
- [ ] **Graph is a DAG (acyclic)**: After `Dag::build()` succeeds, the resulting `WorkflowDefinition` is guaranteed acyclic
- [ ] **Cycle detection is deterministic**: Same graph always produces same cycle_nodes ordering
- [ ] **All nodes are referenced**: Every edge's `source_node` and `target_node` exist in the nodes list
- [ ] **Write-once is not violated**: `--graph` output does not use FD3/FD4 (separate concerns from task I/O)

## Error Taxonomy
```rust
// Errors from vo-types::WorkflowDefinitionError
pub enum WorkflowDefinitionError {
    /// JSON could not be deserialized into the intermediate unvalidated struct.
    DeserializationFailed { message: String },
    /// The nodes list is empty.
    EmptyWorkflow,
    /// The graph contains a cycle.
    CycleDetected { cycle_nodes: Vec<NodeName> },
    /// An edge references a node name that does not exist in the nodes list.
    UnknownNode { edge_source: NodeName, unknown_target: NodeName },
    /// A DagNode contains an invalid RetryPolicy.
    InvalidRetryPolicy { node_name: NodeName, reason: RetryPolicyError },
}

// Additional SDK-specific errors for --graph integration
pub enum GraphOutputError {
    /// Cycle was detected in the DAG before serialization.
    CycleDetected { cycle_nodes: Vec<NodeName> },
    /// Failed to serialize WorkflowDefinition to JSON.
    SerializationFailed,
    /// stdout is not available for writing.
    StdoutUnavailable,
}
```

## Contract Signatures

### Core DAG Build
```rust
impl Dag {
    /// Build and validate the DAG, running cycle detection.
    /// Returns Ok(WorkflowDefinition) if acyclic, Err(WorkflowDefinitionError) otherwise.
    /// 
    /// # Errors
    /// Returns `WorkflowDefinitionError::EmptyWorkflow` if no nodes added.
    /// Returns `WorkflowDefinitionError::CycleDetected { cycle_nodes }` if cycle found.
    pub fn build(self) -> Result<WorkflowDefinition, WorkflowDefinitionError>;
}
```

### Graph Serialization (CLI integration)
```rust
/// Output the DAG as JSON to stdout if --graph flag is present.
/// This function MUST be called after `Dag::build()` succeeds.
/// 
/// # Errors
/// Returns `GraphOutputError::SerializationFailed` if JSON serialization fails.
/// Returns `GraphOutputError::StdoutUnavailable` if stdout cannot be written to.
/// 
/// # Panics
/// Never. All fallible operations return Result.
pub fn output_graph(workflow: &WorkflowDefinition) -> Result<(), GraphOutputError>;
```

### Cycle Detection (delegated to vo-types)
```rust
// Imported from vo-types::workflow::detect_cycle
// fn detect_cycle(nodes: &[DagNode], edges: &[Edge]) -> Option<Vec<NodeName>>
// Returns Some(cycle_nodes) if cycle found, None otherwise
```

## Non-goals
- [ ] Runtime cycle detection (this is compile-time/discovery-time only per ADR-022)
- [ ] Modifying the existing FD3/FD4 read/write scaffold
- [ ] Adding retry policy validation (handled by vo-types)
- [ ] Handling fan-in scenarios (per ADR-010, these use runtime serde validation)

## ADR-022 Specific Requirements
1. **Compile-time/Discovery Validation**: Cycle detection MUST run before `--graph` serialization
2. **Error Format**: stderr message must specify the exact node names forming the cycle
3. **Exit Code**: Non-zero exit code when cycle detected (engine refuses to register)
4. **No Runtime Failure**: Cycles are caught at discovery time, never at runtime when webhook fires
5. **Algorithm**: DFS or Kahn's algorithm (DFS already exists in vo-types)

## Validation Checklist
- [ ] Every error variant in `WorkflowDefinitionError` has a corresponding test
- [ ] `CycleDetected` error includes the exact node names in the error message
- [ ] Cycle detection handles self-loops (A->A)
- [ ] Cycle detection handles disconnected components with cycles
- [ ] `--graph` with valid DAG outputs correct JSON to stdout
- [ ] `--graph` with cyclic DAG outputs error to stderr and exits non-zero
