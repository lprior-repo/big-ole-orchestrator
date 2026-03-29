bead_id: vo-ald
bead_title: vo-types: define WorkflowDefinition and DAG node types
phase: state-1-contract-synthesis
updated_at: 2026-03-27T08:45:00Z

# Contract Specification: vo-ald -- vo-types: define WorkflowDefinition and DAG node types

## Context

- **Feature**: Pure domain types for a workflow DAG definition -- `WorkflowDefinition`, `DagNode`, `Edge`, `EdgeCondition`, `RetryPolicy` -- plus a pure function `next_nodes` for graph traversal. The `parse` constructor on `WorkflowDefinition` validates acyclicity, edge referential integrity, and retry policy constraints. No runtime deps (no fjall/tokio/axum).
- **Domain terms**:
  - `WorkflowDefinition`: The complete, validated graph of a workflow. Contains a name, at least one node, and zero or more directed edges. Guaranteed acyclic after construction.
  - `DagNode`: A single step in the workflow graph. Carries a name and a retry policy. Per ADR-009, the binary path is NOT stored here -- it belongs to `WorkflowRegistration` (a separate concern in the engine layer).
  - `Edge`: A directed connection from one node to another, annotated with an `EdgeCondition` that determines when traversal occurs.
  - `EdgeCondition`: Enum discriminating conditional traversal: `Always` (unconditional), `OnSuccess` (only if the source node succeeded), `OnFailure` (only if the source node failed).
  - `RetryPolicy`: Per-node retry configuration with exponential backoff. `max_attempts >= 1`, `backoff_multiplier >= 1.0`.
  - `StepOutcome`: Result discriminant from `vo-icg` (success/failure). Used by `next_nodes` to filter edges. NOT defined in this crate -- either a re-export stub or a `cfg`-gated local definition is needed.
  - `NonEmptyVec`: A `Vec<T>` guaranteed to contain at least one element. Enforces the "at least one node" invariant structurally.
- **Existing types in vo-types** (from vo-acb, already implemented):
  - `WorkflowName` -- `pub(crate) String`, validated: non-empty, `[a-zA-Z0-9_-]`, max 128 chars, no leading/trailing `-`/`_`. Implements `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash`, `Display`.
  - `NodeName` -- same validation as `WorkflowName`. Same trait impls.
  - `ParseError` -- existing error enum with variants `Empty`, `InvalidCharacters`, `InvalidFormat`, `ExceedsMaxLength`, `BoundaryViolation`, `NotAnInteger`, `ZeroValue`, `OutOfRange`.
  - `MaxAttempts` -- `NonZeroU64` newtype. This is NOT the same as `RetryPolicy.max_attempts` (which is `u8`). `MaxAttempts` is used elsewhere for engine-level attempt tracking. `RetryPolicy` has its own field.
- **Assumptions**:
  - `WorkflowName` and `NodeName` are reused from `string_types.rs` -- NOT redefined.
  - `StepOutcome` is defined in `vo-icg` (not yet available). For this bead, a minimal local enum or trait object is used to decouple. The contract specifies the shape: `StepOutcome { Success, Failure }`.
  - `NonEmptyVec<T>` does not exist in the crate and must be defined here.
  - `serde_json` must be promoted from `[dev-dependencies]` to `[dependencies]` in `vo-types/Cargo.toml` since `WorkflowDefinition::parse` deserializes JSON in non-test code.
  - `petgraph` is available as a workspace dependency and may be used for cycle detection, but the public API does NOT expose petgraph types.
  - The cycle detection algorithm (DFS per ADR-022) reports the exact node names forming the cycle in the error variant.
- **Open questions**:
  - OQ-1: Should `StepOutcome` be defined locally in `vo-types` or imported from `vo-icg`? Recommendation: define a minimal `StepOutcome` enum in `vo-types` to avoid a circular dependency. `vo-icg` can `From`-convert its own outcome type.
  - OQ-2: Should `NonEmptyVec` be a full newtype with accessor methods, or just a type alias with validation? Recommendation: full newtype with `first()`, `rest()`, `as_slice()`, `into_vec()`.
  - OQ-3: Should `Edge` use `NodeName` references or owned values? Recommendation: owned `NodeName` (consistent with the rest of the crate's ownership model).

## Preconditions

### For `WorkflowDefinition::parse(json_bytes: &[u8])`:
- P-1: `json_bytes` is valid UTF-8 JSON.
- P-2: The JSON structure matches the expected schema (object with `workflow_name`, `nodes`, `edges` fields).
- P-3: No external state (DB, network, filesystem) is required.
- P-4: Parsing is deterministic: the same input always produces the same `Ok` or `Err` result.

### For `next_nodes(current, last_outcome, def)`:
- P-5: `current` is a `NodeName` that exists in `def.nodes`.
- P-6: `def` is a valid `WorkflowDefinition` (acyclic, all edges reference valid nodes).
- P-7: `last_outcome` is `StepOutcome::Success` or `StepOutcome::Failure`.

### For `RetryPolicy::new(max_attempts, backoff_ms, backoff_multiplier)`:
- P-8: `max_attempts` is a `u8`.
- P-9: `backoff_ms` is a `u64`.
- P-10: `backoff_multiplier` is a `f32`.

## Postconditions

### For `WorkflowDefinition::parse(json_bytes: &[u8])`:
- PO-1: On `Ok(def)`, `def` is guaranteed acyclic (invariant I-1 holds).
- PO-2: On `Ok(def)`, `def.nodes` contains at least one `DagNode` (invariant I-2 holds).
- PO-3: On `Ok(def)`, every `Edge.source_node` and `Edge.target_node` references a `NodeName` present in `def.nodes` (invariant I-3 holds).
- PO-4: On `Ok(def)`, every `DagNode.retry_policy` satisfies `max_attempts >= 1` and `backoff_multiplier >= 1.0` (invariant I-4 holds).
- PO-5: On `Err(WorkflowDefinitionError)`, no `WorkflowDefinition` is constructed.
- PO-6: On `Err(CycleDetected { cycle_nodes })`, `cycle_nodes` contains the exact `NodeName` values forming the cycle path.
- PO-7: On `Err(UnknownNode { edge_source, unknown_target })`, the edge from `edge_source` to `unknown_target` is identified.
- PO-8: `parse()` never panics. All validation failures return `Err`.

### For `next_nodes(current, last_outcome, def)`:
- PO-9: Returns a `Vec<&DagNode>` containing only nodes that are direct successors of `current` via edges whose `EdgeCondition` matches `last_outcome`.
- PO-10: Returns an empty `Vec` if no edges from `current` match the condition (e.g., terminal node).
- PO-11: Complexity is O(|edges|) -- iterates edges once.
- PO-12: The returned `&DagNode` references borrow from `def` and live as long as the borrow.
- PO-13: `next_nodes` is a pure function with no side effects.

### For `RetryPolicy::new(max_attempts, backoff_ms, backoff_multiplier)`:
- PO-14: On `Ok(policy)`, `policy.max_attempts >= 1` and `policy.backoff_multiplier >= 1.0`.
- PO-15: On `Err(RetryPolicyError::ZeroAttempts)`, `max_attempts` was 0.
- PO-16: On `Err(RetryPolicyError::InvalidMultiplier { got })`, `backoff_multiplier` was < 1.0.
- PO-17: `new()` never panics. All validation failures return `Err`.

### For all types:
- PO-18: All types implement `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`.
- PO-19: Serde round-trip is identity: `deserialize(serialize(t)) == t` for all valid values.

## Invariants

### WorkflowDefinition invariants:
- I-1: **Acyclic** -- The directed graph formed by `edges` over `nodes` contains no cycles. This is checked at construction time via DFS (per ADR-022).
- I-2: **Non-empty nodes** -- `nodes` is a `NonEmptyVec<DagNode>`, so it always contains at least one element. Zero nodes is structurally unrepresentable.
- I-3: **Edge referential integrity** -- For every `Edge { source_node, target_node, .. }` in `edges`, both `source_node` and `target_node` appear in `def.nodes` (matched by `NodeName` equality).

### DagNode invariants:
- I-4: **No binary path** -- `DagNode` does NOT contain a `binary_path` field. Per ADR-009, binary path belongs to `WorkflowRegistration`, not the DAG definition.
- I-5: **Valid retry policy** -- Every `DagNode.retry_policy` satisfies the `RetryPolicy` invariants.

### RetryPolicy invariants:
- I-6: `max_attempts >= 1`. Zero is structurally prevented by the constructor.
- I-7: `backoff_multiplier >= 1.0`. Values < 1.0 are structurally prevented by the constructor.
- I-8: `backoff_ms` is a plain `u64` with no range constraint (0 is valid for "no delay").

### EdgeCondition invariants:
- I-9: Exhaustive enum with exactly three variants: `Always`, `OnSuccess`, `OnFailure`. No other variants exist.

### Edge invariants:
- I-10: `source_node` and `target_node` are owned `NodeName` values that satisfy `NodeName`'s existing invariants (non-empty, valid chars, max 128, no boundary violations).

### StepOutcome invariants (if defined locally):
- I-11: Exhaustive enum with exactly two variants: `Success`, `Failure`. No other variants exist.

## Error Taxonomy

### `WorkflowDefinitionError` -- returned by `WorkflowDefinition::parse`

```rust
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WorkflowDefinitionError {
    /// JSON could not be deserialized into the intermediate unvalidated struct.
    #[error("workflow definition deserialization failed: {source}")]
    DeserializationFailed {
        source: serde_json::Error,
    },

    /// The nodes list is empty.
    #[error("workflow definition must contain at least one node")]
    EmptyWorkflow,

    /// The graph contains a cycle. The cycle_nodes field contains the
    /// exact node names forming the cycle path (first node repeated at end).
    #[error("workflow contains a cycle: {cycle_nodes:?}")]
    CycleDetected {
        cycle_nodes: Vec<NodeName>,
    },

    /// An edge references a node name that does not exist in the nodes list.
    #[error("edge from '{edge_source}' references unknown target node '{unknown_target}'")]
    UnknownNode {
        edge_source: NodeName,
        unknown_target: NodeName,
    },

    /// A DagNode contains an invalid RetryPolicy.
    #[error("node '{node_name}' has invalid retry policy: {reason}")]
    InvalidRetryPolicy {
        node_name: NodeName,
        reason: RetryPolicyError,
    },
}
```

### `RetryPolicyError` -- returned by `RetryPolicy::new`

```rust
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RetryPolicyError {
    /// max_attempts was zero.
    #[error("max_attempts must be >= 1, got 0")]
    ZeroAttempts,

    /// backoff_multiplier was less than 1.0.
    #[error("backoff_multiplier must be >= 1.0, got {got}")]
    InvalidMultiplier {
        got: f32,
    },
}
```

### Error variant selection priority for `WorkflowDefinition::parse`:
1. `DeserializationFailed` -- JSON parse/structure errors. Checked first.
2. `EmptyWorkflow` -- after deserialization, if `nodes` is empty.
3. `InvalidRetryPolicy` -- per-node retry policy validation.
4. `UnknownNode` -- edge referential integrity (both source and target checked).
5. `CycleDetected` -- DFS cycle check. Performed last since it is the most expensive and requires a valid graph structure.

### Error variant selection priority for `RetryPolicy::new`:
1. `ZeroAttempts` -- check `max_attempts == 0` first.
2. `InvalidMultiplier` -- check `backoff_multiplier < 1.0` second.

## Contract Signatures

### Module structure

```rust
// crates/vo-types/src/lib.rs (additions only, existing modules untouched)
mod workflow;
mod non_empty_vec;

pub use errors::ParseError;
pub use workflow::{
    DagNode, Edge, EdgeCondition, RetryPolicy, RetryPolicyError,
    StepOutcome, WorkflowDefinition, WorkflowDefinitionError,
};
pub use non_empty_vec::NonEmptyVec;
pub use types::{
    AttemptNumber, BinaryHash, DurationMs, EventVersion, FireAtMs,
    IdempotencyKey, InstanceId, MaxAttempts, NodeName, SequenceNumber,
    TimeoutMs, TimestampMs, TimerId, WorkflowName,
};
```

### `NonEmptyVec<T>`

```rust
/// A `Vec<T>` guaranteed to contain at least one element.
/// Construction always validates non-emptiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<T> NonEmptyVec<T> {
    /// Construct from a Vec. Returns Err if the vec is empty.
    pub fn new(items: Vec<T>) -> Result<Self, &'static str>;

    /// Construct from a Vec. Panics if empty.
    /// Intended for internal use where the caller guarantees non-emptiness.
    pub fn new_unchecked(items: Vec<T>) -> Self;

    /// Borrow the first element.
    pub fn first(&self) -> &T;

    /// Borrow all elements except the first.
    pub fn rest(&self) -> &[T];

    /// Borrow the full inner slice.
    pub fn as_slice(&self) -> &[T];

    /// Consume and return the inner Vec.
    pub fn into_vec(self) -> Vec<T>;

    /// Number of elements.
    pub fn len(&self) -> usize;

    /// Always true (by invariant).
    pub fn is_empty(&self) -> bool;  // returns false
}

impl<T> IntoIterator for NonEmptyVec<T> { /* delegates to inner Vec */ }
```

### `StepOutcome`

```rust
/// Outcome of executing a single DAG node.
/// Defined locally in vo-types to avoid circular deps with vo-icg.
/// vo-icg should From-convert its own outcome type to this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepOutcome {
    Success,
    Failure,
}
```

### `EdgeCondition`

```rust
/// Condition on which an edge is traversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeCondition {
    /// Always traverse this edge, regardless of step outcome.
    Always,
    /// Traverse only if the source node succeeded.
    OnSuccess,
    /// Traverse only if the source node failed.
    OnFailure,
}
```

### `RetryPolicy`

```rust
/// Per-node retry configuration with exponential backoff.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of execution attempts (minimum 1).
    pub max_attempts: u8,
    /// Initial backoff delay in milliseconds.
    pub backoff_ms: u64,
    /// Multiplier applied to backoff after each retry (minimum 1.0).
    pub backoff_multiplier: f32,
}

impl RetryPolicy {
    /// Construct a new RetryPolicy with validation.
    /// Returns Err(RetryPolicyError) if max_attempts == 0 or backoff_multiplier < 1.0.
    pub fn new(
        max_attempts: u8,
        backoff_ms: u64,
        backoff_multiplier: f32,
    ) -> Result<Self, RetryPolicyError>;
}
```

### `DagNode`

```rust
/// A single step in the workflow DAG.
/// Per ADR-009: binary_path is NOT stored here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DagNode {
    pub node_name: NodeName,
    pub retry_policy: RetryPolicy,
}
```

### `Edge`

```rust
/// A directed edge from one node to another with a traversal condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub source_node: NodeName,
    pub target_node: NodeName,
    pub condition: EdgeCondition,
}
```

### `WorkflowDefinition`

```rust
/// The complete, validated workflow DAG.
/// Guaranteed acyclic after construction.
/// No Default impl -- construction always requires explicit validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub workflow_name: WorkflowName,
    pub nodes: NonEmptyVec<DagNode>,
    pub edges: Vec<Edge>,
}

impl WorkflowDefinition {
    /// Parse a JSON byte slice into a validated WorkflowDefinition.
    ///
    /// Validation order:
    /// 1. JSON deserialization into intermediate struct
    /// 2. Non-empty nodes check
    /// 3. RetryPolicy validation per node
    /// 4. Edge referential integrity (source and target node names must exist)
    /// 5. DFS cycle detection
    ///
    /// Returns Err(WorkflowDefinitionError) for any violation.
    pub fn parse(json_bytes: &[u8]) -> Result<Self, WorkflowDefinitionError>;

    /// Look up a DagNode by NodeName. Returns None if not found.
    pub fn get_node(&self, name: &NodeName) -> Option<&DagNode>;
}

/// Pure function: find the successor nodes for a given current node and outcome.
///
/// Iterates all edges from `current`, filters by EdgeCondition matching `last_outcome`,
/// and returns the DagNodes whose NodeName appears as the target of matching edges.
///
/// Complexity: O(|edges|)
/// Returns: Vec of &DagNode references borrowing from `def`.
pub fn next_nodes(
    current: &NodeName,
    last_outcome: StepOutcome,
    def: &WorkflowDefinition,
) -> Vec<&DagNode>;
```

## Non-goals

- NG-1: No `Default` impl on `WorkflowDefinition`, `DagNode`, `Edge`, or `RetryPolicy`. Construction always requires explicit validation.
- NG-2: No `binary_path` field on `DagNode` -- per ADR-009, binary path belongs to `WorkflowRegistration`.
- NG-3: No runtime dependencies (no fjall, tokio, axum, ractor). This is pure domain logic.
- NG-4: No topological sort API. Topological ordering is an engine concern, not a definition concern.
- NG-5: No mutation methods on `WorkflowDefinition` after construction. The type is immutable.
- NG-6: No `petgraph` types in the public API. petgraph (if used) is an implementation detail for cycle detection only.
- NG-7: No `Hash` derive on `WorkflowDefinition` or `DagNode` (they contain `f32` via `RetryPolicy`, which is not `Hash`).
- NG-8: No `PartialOrd`/`Ord` on any of the new types.
- NG-9: No `From<serde_json::Error>` impl on `WorkflowDefinitionError` -- the `DeserializationFailed` variant wraps it explicitly.
- NG-10: No `NaN` or `Infinity` check on `backoff_multiplier` beyond the `< 1.0` check. Normal `f32` semantics apply; JSON deserialization of `NaN`/`Infinity` is rejected by serde by default.
- NG-11: No `max_attempts` upper bound beyond `u8::MAX` (255). Practical limits are a caller concern.
- NG-12: No `backoff_ms` validation (0 is valid for "no initial delay").
- NG-13: No `Eq` derive on `RetryPolicy` or `WorkflowDefinition` (both contain `f32`).
- NG-14: No edge deduplication. If two edges with the same `(source, target, condition)` exist, both are stored and both contribute to `next_nodes` results.
- NG-15: No self-loop detection as a separate error. A self-loop (A -> A) is caught by the general cycle detection.
- NG-16: No `NodeName` uniqueness constraint within `def.nodes`. Duplicate node names are allowed at the definition level (engine may impose uniqueness at registration time).
