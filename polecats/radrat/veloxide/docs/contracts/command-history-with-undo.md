## Contract: Command History with Undo

### 1. Purpose

Defines the contract for the command history subsystem that tracks workflow graph modifications and provides undo/redo capabilities to operators. This contract establishes the types, invariants, and error taxonomy for the command history system that backs the Dioxus UI's extend-flow workflow editor.

### 2. Source ADRs

- `docs/adr/v2/ADR-031-v2-canonical-workflow-spec-sdk-ui.md` (canonical WorkflowSpec)
- `docs/adr/v2/ADR-007-v2-dioxus-observability-ui.md` (Dioxus UI conventions)
- `docs/adr/v2/ADR-036-v2-command-identity-correlation-and-causation.md` (command identity)

### 3. Command Types

#### 3.1 CommandKind

The classification of graph-modifying operations.

```
enum CommandKind {
    ExtensionApply,      // Bulk or individual extension application
    ExtensionRevert,    // Undo of a prior extension apply
    ExtensionRedo,      // Redo of a previously undone extension
    NodeCreate,         // Direct node creation via UI
    NodeDelete,         // Direct node deletion via UI
    EdgeCreate,         // Edge creation via UI
    EdgeDelete,         // Edge deletion via UI
    ConfigUpdate,        // Node or edge configuration change
}
```

#### 3.2 CommandEnvelope (extends ADR-036)

Carries identity metadata for a history entry.

```
CommandEnvelope {
    command_id: CommandId,           // Stable identity for this command
    correlation_id: CorrelationId,    // Groups related commands
    causation_id: CausationId,       // Immediate parent command/event
    issued_at: TimestampMs,
    issuer: Issuer,
}
```

#### 3.3 CommandId

Unique identifier for a command in the history.

```
CommandId(String) // ULID-based, non-empty
```

#### 3.4 ExtensionApplyMode

How extensions were applied.

```
enum ExtensionApplyMode {
    Single,     // One extension at a time
    Bulk,       // Multiple extensions applied together
}
```

### 4. Snapshot Types

#### 4.1 WorkflowSnapshot

Captures the complete workflow graph state at a point in time.

```
WorkflowSnapshot {
    snapshot_id: SnapshotId,
    captured_at: TimestampMs,
    workflow_name: String,
    nodes: Vec<SketchNode>,        // Current node state
    edges: Vec<Edge>,              // Current edge state
    checksum: u32,                 // CRC32 of normalized graph
}
```

#### 4.2 SnapshotId

Unique identifier for a snapshot.

```
SnapshotId(String) // ULID-based, non-empty
```

#### 4.3 ExtensionBatchMetadata

Metadata about a batch of extensions applied.

```
ExtensionBatchMetadata {
    batch_id: BatchId,
    snapshot_id: SnapshotId,
    mode: ExtensionApplyMode,
    applied_keys: Vec<String>,      // Extension keys that were applied
    created_nodes: Vec<NodeId>,     // Nodes created by this batch
    parent_command_id: CommandId,   // Command that created this batch
}
```

### 5. History Entry

#### 5.1 HistoryEntry

A single entry in the command history stack.

```
HistoryEntry {
    envelope: CommandEnvelope,
    kind: CommandKind,
    snapshot_before: Option<WorkflowSnapshot>,
    snapshot_after: Option<WorkflowSnapshot>,
    batch_metadata: Option<ExtensionBatchMetadata>,
    status: HistoryEntryStatus,
}
```

#### 5.2 HistoryEntryStatus

Outcome of the command.

```
enum HistoryEntryStatus {
    Committed,    // Command succeeded, entry is final
    Undone,       // Command was reverted via undo
    Redone,       // Command was restored via redo
    Failed,       // Command failed during execution
}
```

### 6. Command History State

#### 6.1 CommandHistory

The full undo/redo stack.

```
CommandHistory {
    entries: Vec<HistoryEntry>,     // All history entries
    undo_stack: Vec<CommandId>,    // Commands available to undo (by ID)
    redo_stack: Vec<CommandId>,    // Commands available to redo (by ID)
    capacity: usize,                // Maximum history depth
}
```

#### 6.2 HistoryConstraints

```
const MAX_HISTORY_DEPTH: usize = 100;
const MAX_UNDO_STACK_DEPTH: usize = 50;
const MAX_REDO_STACK_DEPTH: usize = 50;
```

### 7. Invariants (INV-*)

- **INV-001**: `undo_stack.len() == redo_stack.len()` only when history is in equilibrium (no pending undos/redos)
- **INV-002**: `undo_stack` is always a prefix of `entries` in reverse chronological order
- **INV-003**: `redo_stack` contains only entries with `status == Undone`
- **INV-004**: After `undo()`, the undone entry's status transitions to `Undone` and its command moves from `undo_stack` to `redo_stack`
- **INV-005**: After `redo()`, the redone entry's status transitions to `Redone` and its command moves from `redo_stack` to `undo_stack`
- **INV-006**: `can_undo()` returns `true` iff `undo_stack` is non-empty
- **INV-007**: `can_redo()` returns `true` iff `redo_stack` is non-empty
- **INV-008**: `save_undo_point()` creates a new `HistoryEntry` with `status == Committed` and pushes to `undo_stack`
- **INV-009**: `save_undo_point()` clears `redo_stack` (new commands invalidate redo history)
- **INV-010**: `entries.len() <= MAX_HISTORY_DEPTH`; oldest entries beyond capacity are dropped
- **INV-011**: `snapshot_before` is `Some` for all graph-modifying commands
- **INV-012**: `snapshot_after` is `Some` for all commands with `status == Committed`
- **INV-013**: Checksum validation passes when restoring from snapshot

### 8. Error Taxonomy

```rust
enum CommandHistoryError {
    UndoStackEmpty,
    RedoStackEmpty,
    SnapshotNotFound {
        snapshot_id: SnapshotId,
    },
    EntryNotFound {
        command_id: CommandId,
    },
    ChecksumMismatch {
        expected: u32,
        actual: u32,
    },
    HistoryCapacityExceeded {
        capacity: usize,
    },
    SnapshotSerializationError {
        reason: String,
    },
    InvalidHistoryTransition {
        current_status: HistoryEntryStatus,
        attempted_action: String,
    },
}
```

### 9. Operations Protocol

#### 9.1 save_undo_point() -> Result<CommandId, CommandHistoryError>

1. Validate current history state
2. Create new `CommandEnvelope` with unique `command_id`
3. Capture `snapshot_before` of current workflow state
4. Create `HistoryEntry` with `status == Committed`
5. Push to `undo_stack`
6. Clear `redo_stack`
7. Return `CommandId`

#### 9.2 undo() -> Result<bool, CommandHistoryError>

1. Check `can_undo()`; return `Ok(false)` if empty
2. Pop `CommandId` from `undo_stack`
3. Find corresponding `HistoryEntry`
4. Validate `snapshot_before` exists and checksum
5. Restore workflow state from `snapshot_before`
6. Update entry status to `Undone`
7. Push `CommandId` to `redo_stack`
8. Return `Ok(true)`

#### 9.3 redo() -> Result<bool, CommandHistoryError>

1. Check `can_redo()`; return `Ok(false)` if empty
2. Pop `CommandId` from `redo_stack`
3. Find corresponding `HistoryEntry`
4. Validate `snapshot_after` exists and checksum
5. Restore workflow state from `snapshot_after`
6. Update entry status to `Redone`
7. Push `CommandId` to `undo_stack`
8. Return `Ok(true)`

#### 9.4 apply_command() -> Result<CommandId, CommandHistoryError>

1. Call `save_undo_point()`
2. Execute the graph-modifying operation
3. Capture `snapshot_after`
4. Update entry with batch metadata
5. Return `CommandId`

### 10. Constraints

- History is ephemeral during a session; it does not persist across server restarts
- Snapshots are stored in memory with bounded capacity
- Undo/redo operates only on the in-memory workflow state
- Checksums use CRC32 for fast comparison
- All operations return `Result`; no panics in the history subsystem

### 11. Relevant Files

- `crates/vo-frontend/src/ui/selected_node_panel/extend_flow.rs` (current undo UI)
- `crates/vo-frontend/src/ui/selected_node_panel/types.rs` (ExtensionBatchSnapshot, ExtensionTimelineEvent)
- `crates/vo-types/src/command_envelope.rs` (ADR-036 CommandEnvelope)
- `crates/vo-types/src/workflow/types.rs` (DagNode, Edge)

### 12. Acceptance Criteria

- Command history tracks all graph-modifying operations with unique command IDs
- Undo correctly restores workflow state to the snapshot taken before the command
- Redo correctly restores workflow state to the snapshot taken after the command
- New commands clear the redo stack (standard undo/redo invariant)
- `can_undo()` and `can_redo()` accurately reflect stack state
- History entries capture both before and after snapshots for committed commands
- Error taxonomy covers all failure modes (empty stacks, missing snapshots, checksum mismatch)
- Contract references only existing files and ADRs