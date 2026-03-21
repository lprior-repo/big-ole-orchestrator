# Contract Specification: wtf-frontend graph core (wtf-d7kw)

## Context
- **Feature**: Adapt Oya graph module for FSM/DAG/Procedural node types
- **Domain terms**: FSM (Finite State Machine), DAG (Directed Acyclic Graph), Procedural workflow
- **Assumptions**: 
  - The existing graph module structure (Workflow, Node, Connection) is sound and should be preserved
  - The execution_state module with time-travel support is already implemented and working
  - New node types should integrate with existing Node, Connection, and Workflow types
- **Open questions**:
  - Should existing Restate-based node types be removed or maintained alongside new types?

## Preconditions

1. **NodeType enum variants are exhaustive**: Any match on NodeType must handle all 9 variants
2. **Valid state transitions**: FSM nodes can only transition between valid states (Entry→State→Transition→Final)
3. **DAG acyclicity**: DAG nodes (Task, Split, Join) must not create cycles
4. **Unique node IDs**: Each node in a workflow must have a unique NodeId
5. **Valid connections**: Connections must reference existing nodes (source and target must exist in workflow)
6. **Non-empty node names**: Node names must be non-empty strings
7. **Port name validity**: Port names on connections must match source/target node port definitions

## Postconditions

1. **Node creation preserves identity**: A created node appears in workflow.nodes with correct id
2. **Connection graph integrity**: Adding a connection maintains the connections list
3. **State transitions are recorded**: FSM state changes are recorded in execution_state history
4. **Node type is queryable**: node.node_type() returns correct NodeType variant
5. **Time-travel cursor is valid**: execution_state cursor position is within bounds of history
6. **Workflow serialization is lossless**: Serializing and deserializing a workflow preserves all data

## Invariants

1. **Workflow graph is never empty after initialization**: A workflow always has at least one entry node
2. **All connections reference valid nodes**: No orphan connections (node ID not in workflow.nodes)
3. **FSM entry nodes have no incoming connections**: Entry nodes are graph roots
4. **FSM final nodes have no outgoing connections**: Final nodes are terminal
5. **DAG split nodes have multiple outgoing, single incoming**: Fan-out pattern
6. **DAG join nodes have single outgoing, multiple incoming**: Fan-in pattern

## Error Taxonomy

- `Error::InvalidNodeType` - when node type variant is not valid for the operation
- `Error::InvalidStateTransition` - when FSM transition is not allowed (e.g., Final→Entry)
- `Error::CycleDetected` - when DAG would create a cycle
- `Error::NodeNotFound` - when connection references non-existent node
- `Error::DuplicateNodeId` - when adding node with existing ID
- `Error::EmptyNodeName` - when node name is empty
- `Error::InvalidPortName` - when port name doesn't exist on node
- `Error::TimeTravelBounds` - when cursor position is outside history bounds
- `Error::DisconnectedGraph` - when workflow graph is not fully connected

## Contract Signatures

```rust
// NodeType enum - the core type being introduced
pub enum NodeType {
    FsmEntry,
    FsmTransition,
    FsmState,
    FsmFinal,
    DagTask,
    DagSplit,
    DagJoin,
    ProceduralStep,
    ProceduralScript,
}

// Workflow operations
fn add_node(workflow: &mut Workflow, node: Node) -> Result<(), Error>
fn add_connection(workflow: &mut Workflow, conn: Connection) -> Result<(), Error>
fn remove_node(workflow: &mut Workflow, node_id: NodeId) -> Result<(), Error>
fn get_node(workflow: &Workflow, node_id: NodeId) -> Result<Node, Error>

// FSM operations
fn fsm_transition(state: &ExecutionState, from: FsmState, to: FsmState) -> Result<ExecutionState, Error>
fn validate_fsm_chain(nodes: &[Node]) -> Result<(), Error>

// DAG operations
fn validate_dag_acyclicity(nodes: &[Node], connections: &[Connection]) -> Result<(), Error>
fn dag_fan_out(workflow: &Workflow, split_node_id: NodeId) -> Result<Vec<NodeId>, Error>
fn dag_fan_in(workflow: &Workflow, join_node_id: NodeId) -> Result<NodeId, Error>

// Time-travel operations
fn jump_to_cursor(state: &ExecutionState, cursor: usize) -> Result<ExecutionState, Error>
fn get_history_snapshot(state: &ExecutionState, index: usize) -> Result<ExecutionRecord, Error>
```

## Type Encoding

| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| NodeType variants exhaustive | Compile-time | `enum NodeType { ... }` with sealed trait |
| Valid FSM transitions | Runtime-checked | `Result<FsmTransition, Error::InvalidStateTransition>` |
| DAG acyclicity | Runtime-checked | `Result<(), Error::CycleDetected>` |
| Non-empty node name | Compile-time | `NonEmptyString::new() -> Result<NodeName, Error>` |
| NodeId uniqueness | Runtime-checked | `HashMap<NodeId, Node>` prevents duplicates |
| Port name validity | Runtime-checked | `Result<(), Error::InvalidPortName>` |

## Violation Examples

- VIOLATES P2: `fsm_transition(Final, Final→Entry)` — should produce `Err(Error::InvalidStateTransition)`
- VIOLATES P3: `add_connection(Join→Task→Split→Join creates cycle)` — should produce `Err(Error::CycleDetected)`
- VIOLATES P5: `add_node(workflow, Node { name: "" })` — should produce `Err(Error::EmptyNodeName)`
- VIOLATES P6: `add_connection(source_port: "nonexistent")` — should produce `Err(Error::InvalidPortName)`

## Ownership Contracts

- `add_node(workflow, node)` — ownership of `node` transfers into `workflow.nodes` vec
- `get_node(workflow, id)` — returns reference, no ownership transfer
- `fsm_transition(state, ...)` — `state` is borrowed, returns new `ExecutionState` (no mutation)
- `jump_to_cursor(state, pos)` — `state` borrowed, returns new state at cursor position

## Non-goals

- Implementing actual FSM/DAG/Procedural execution logic (only type definitions and validation)
- Removing existing Restate-based node types (backwards compatibility)
- Network serialization of workflows (local only)
