# Contract: Tree-structured Workspace Index

## 1. Purpose

Defines the contract for a tree-structured workspace index in the veloxide event-sourced system. This contract establishes types, invariants, and error taxonomy for indexing and navigating hierarchical workspace structures. The workspace index is the authoritative source for workspace membership and tree topology.

## 2. Source ADRs

- `docs/adr/v2/ADR-039-v2-hierarchical-lifecycle-state-machine.md` (hierarchical structures)
- `docs/adr/v2/ADR-016-v2-atomic-storage-snapshots.md` (atomic batch operations)
- `docs/adr/v2/ADR-002-v2-fjall-storage.md` (key encoding and tree structures)

## 3. Workspace Index Types

### 3.1 WorkspaceId

Unique identifier for a workspace node.

```
WorkspaceId {
  ulid: Ulid,
}
```

### 3.2 WorkspacePath

Represents the hierarchical path from root to a workspace node.

```
WorkspacePath {
  segments: NonEmptyVec<WorkspaceName>,
}
```

### 3.3 WorkspaceName

A single segment in a workspace path.

```
WorkspaceName {
  name: String,  // validated: lowercase alphanumeric + hyphens, max 64 chars
}
```

### 3.4 WorkspaceNode

A single node in the workspace tree.

```
WorkspaceNode {
  id: WorkspaceId,
  name: WorkspaceName,
  parent_id: Option<WorkspaceId>,  // None for root
  children: Vec<WorkspaceId>,       // ordered list of child IDs
  metadata: WorkspaceMetadata,
  created_at: TimestampMs,
  updated_at: TimestampMs,
}
```

### 3.5 WorkspaceMetadata

Arbitrary key-value metadata attached to a workspace.

```
WorkspaceMetadata {
  entries: BTreeMap<String, String>,
}
```

### 3.6 WorkspaceIndex

The main index structure holding all workspace nodes.

```
WorkspaceIndex {
  nodes: BTreeMap<WorkspaceId, WorkspaceNode>,
  root_ids: Vec<WorkspaceId>,          // top-level workspaces
  path_index: BTreeMap<WorkspacePath, WorkspaceId>,  // path -> id lookup
}
```

### 3.7 WorkspaceIndexSnapshot

Persistent snapshot of the workspace index.

```
WorkspaceIndexSnapshot {
  version: u64,
  nodes: Vec<WorkspaceNode>,
  root_ids: Vec<WorkspaceId>,
  checksum: u64,
  created_at: TimestampMs,
}
```

## 4. Operations

### 4.1 Insert Workspace

```
insert(parent_id: Option<WorkspaceId>, name: WorkspaceName, metadata: WorkspaceMetadata) -> Result<WorkspaceId, WorkspaceIndexError>
```

Creates a new workspace as a child of the specified parent (or as a root if None).

### 4.2 Delete Workspace

```
delete(id: WorkspaceId) -> Result<(), WorkspaceIndexError>
```

Deletes a workspace and recursively all descendants. Fails if workspace has running instances.

### 4.3 Move Workspace

```
move(id: WorkspaceId, new_parent_id: Option<WorkspaceId>) -> Result<(), WorkspaceIndexError>
```

Reparents a workspace to a new parent. Fails if move would create a cycle.

### 4.4 Update Metadata

```
update_metadata(id: WorkspaceId, metadata: WorkspaceMetadata) -> Result<(), WorkspaceIndexError>
```

Replaces the metadata for a workspace.

### 4.5 Find by Path

```
find_by_path(path: WorkspacePath) -> Result<WorkspaceId, WorkspaceIndexError>
```

Returns the workspace ID for a given path.

### 4.6 Find by ID

```
find_by_id(id: WorkspaceId) -> Result<WorkspaceNode, WorkspaceIndexError>
```

Returns the workspace node for a given ID.

### 4.7 List Children

```
list_children(id: WorkspaceId) -> Result<Vec<WorkspaceId>, WorkspaceIndexError>
```

Returns the ordered list of child workspace IDs.

### 4.8 Get Ancestors

```
get_ancestors(id: WorkspaceId) -> Result<Vec<WorkspaceId>, WorkspaceIndexError>
```

Returns the chain of ancestor IDs from root to immediate parent.

### 4.9 Get Descendants

```
get_descendants(id: WorkspaceId) -> Result<Vec<WorkspaceId>, WorkspaceIndexError>
```

Returns all descendant workspace IDs in depth-first order.

## 5. Invariants (INV-*)

### Structural Invariants

- **INV-001**: `WorkspacePath` always has at least one segment
- **INV-002**: `WorkspacePath` segments are case-insensitive and stored lowercase
- **INV-003**: `WorkspaceId` is unique across the entire index
- **INV-004**: `path_index` is always consistent with `nodes` — every path maps to the correct ID
- **INV-005**: Root workspaces have `parent_id = None`
- **INV-006**: Non-root workspaces have `parent_id = Some(...)` referencing an existing workspace
- **INV-007**: `children` list always matches the reverse of parent relationships
- **INV-008**: A workspace cannot be its own ancestor (no cycles)

### Metadata Invariants

- **INV-009**: `WorkspaceMetadata` keys are unique within a workspace (no duplicate keys)
- **INV-010**: `created_at` is immutable after creation
- **INV-011**: `updated_at` is always >= `created_at`

### Index Integrity Invariants

- **INV-012**: `WorkspaceIndexSnapshot.checksum` validates all bytes in the snapshot
- **INV-013**: Snapshot version increments monotonically on each write
- **INV-014**: Deleted workspace IDs are never reused

### Operational Invariants

- **INV-015**: `delete` removes the workspace and all descendants atomically
- **INV-016**: `move` preserves all descendants' paths recursively
- **INV-017**: `find_by_path` returns the same result as path traversal via parent links
- **INV-018**: Empty `children` list means leaf node (no children)
- **INV-019**: `get_descendants` includes all nested children at any depth

## 6. Error Taxonomy

```rust
enum WorkspaceIndexError {
  // Not found errors
  WorkspaceNotFound(WorkspaceId),
  PathNotFound(WorkspacePath),
  ParentNotFound(WorkspaceId),

  // Constraint violations
  CyclicMoveDetected { workspace_id: WorkspaceId, attempted_parent: WorkspaceId },
  DuplicatePath(WorkspacePath),
  DuplicateName { parent_id: WorkspaceId, name: WorkspaceName },
  CannotDeleteWorkspaceWithInstances { workspace_id: WorkspaceId, instance_count: u32 },
  CannotDeleteWorkspaceWithChildren { workspace_id: WorkspaceId, child_count: u32 },

  // Validation errors
  InvalidWorkspaceName(String),  // name violates format constraints
  EmptyPathSegment,
  PathTooDeep { max_depth: u32, actual_depth: u32 },
  MetadataKeyTooLong { max_length: usize, actual_length: usize },
  MetadataValueTooLong { max_length: usize, actual_length: usize },
  TooManyMetadataEntries { max: usize, actual: usize },

  // State errors
  IndexNotInitialized,
  SnapshotCorrupted { expected_checksum: u64, actual_checksum: u64 },
  VersionMismatch { expected: u64, actual: u64 },

  // Storage errors
  StorageWriteFailed(String),
  StorageReadFailed(String),
}
```

### Error Categories

| Category | Errors | Recovery Strategy |
|----------|--------|------------------|
| NotFound | `WorkspaceNotFound`, `PathNotFound`, `ParentNotFound` | Validate ID/path before operation |
| ConstraintViolation | `CyclicMoveDetected`, `DuplicatePath`, `DuplicateName` | Validate before mutation |
| Precondition | `CannotDeleteWorkspaceWithInstances`, `CannotDeleteWorkspaceWithChildren` | Terminate children first |
| Validation | `InvalidWorkspaceName`, `EmptyPathSegment`, `PathTooDeep` | Validate input |
| StateCorruption | `SnapshotCorrupted`, `VersionMismatch`, `IndexNotInitialized` | Restore from backup |
| Storage | `StorageWriteFailed`, `StorageReadFailed` | Retry or escalate |

## 7. Workspace Index Protocol

### 7.1 Initialize Index

```
1. Check if persisted snapshot exists
2. If yes: Load snapshot, verify checksum, validate invariants
3. If no: Create empty index with version 0
4. Return initialized WorkspaceIndex
```

### 7.2 Insert Workspace

```
1. Validate name format (WorkspaceName rules)
2. If parent_id is Some:
   a. Verify parent exists in nodes
   b. Check no duplicate name under parent
3. Generate new WorkspaceId (ULID)
4. Build WorkspacePath from parent's path + new name
5. Check path_index for duplicate
6. Create WorkspaceNode with empty children
7. Insert into nodes map
8. Insert path into path_index
9. If parent_id is Some: Add to parent's children list
10. If parent_id is None: Add to root_ids
11. Increment version
12. Persist snapshot
```

### 7.3 Delete Workspace

```
1. Verify workspace exists
2. Recursively collect all descendant IDs
3. For each descendant, check no active instances
4. If any have children, fail with CannotDeleteWorkspaceWithChildren
5. Remove all descendant nodes from nodes map
6. Remove all descendant paths from path_index
7. For each descendant, remove from parent's children list
8. Remove from root_ids if present
9. Increment version
10. Persist snapshot
```

### 7.4 Move Workspace

```
1. Verify workspace exists
2. Verify new_parent exists (if Some)
3. Check move does not create cycle:
   a. Get all ancestors of workspace
   b. If new_parent is in ancestors, reject with CyclicMoveDetected
4. Check no duplicate name under new parent
5. Build new path
6. Update path_index for workspace and all descendants
7. Remove from old parent's children list
8. Add to new parent's children list
9. Update parent_id field
10. Increment version
11. Persist snapshot
```

### 7.5 Snapshot and Recovery

```
1. Periodically (or on dirty shutdown):
   a. Serialize entire WorkspaceIndex
   b. Compute checksum
   c. Write WorkspaceIndexSnapshot with version and checksum
2. On recovery:
   a. Load latest snapshot
   b. Verify checksum
   c. Validate all INV-* invariants
   d. If valid: restore index
   e. If invalid: escalate for manual recovery
```

## 8. Constraints

- **Idempotent path lookups**: `find_by_path` must be O(log n)
- **Atomic mutations**: All index operations are atomic — partial failures rollback
- **No soft deletes**: Deleted workspaces are removed immediately
- **Synchronous persistence**: Snapshots are written synchronously to avoid loss
- **Max tree depth**: 16 levels (INV-012)
- **Max metadata entries per workspace**: 64
- **Max metadata key length**: 128 bytes
- **Max metadata value length**: 4096 bytes
- **Max name length**: 64 bytes

## 9. Relevant Files

- `crates/vo-types/src/string_types.rs` (existing ID types like InstanceId)
- `crates/vo-types/src/integer_types.rs` (TimestampMs, SequenceNumber types)
- `crates/vo-types/src/non_empty_vec.rs` (NonEmptyVec used in path segments)
- `crates/vo-storage/src/` (storage backend for snapshots)
- `crates/vo-core/src/` (transaction handling for atomic operations)

## 10. Acceptance Criteria

- [ ] `WorkspaceId` uses ULID for globally unique, time-ordered identifiers
- [ ] `WorkspacePath` supports arbitrary depth up to 16 levels
- [ ] `WorkspaceIndex` provides O(log n) lookup by both ID and path
- [ ] All invariants (INV-001 through INV-019) are formally stated
- [ ] Cycle detection prevents moves that would create invalid tree structures
- [ ] Error taxonomy covers not-found, constraint, validation, state, and storage failures
- [ ] Snapshot protocol ensures no data loss on crash
- [ ] Delete is recursive and atomic across the entire subtree
- [ ] Move preserves all descendant path consistency
- [ ] Contract is self-contained and references only existing crate boundaries
