# Test Plan: Tree-structured Workspace Index

**Contract**: `docs/contracts/tree-structured-workspace-index.md`
**Issue**: ve-h9ot
**Target crate**: `crates/vo-types/src/` (types + unit tests)

## Scope

This plan covers exhaustive testing for the `WorkspaceIndex`, its operations, all 19 invariants (INV-001 through INV-019), the full error taxonomy, edge cases, and property-based invariants. Tests are organized by the Testing Trophy: unit tests (majority), targeted property tests via proptest, and targeted integration tests for multi-operation sequences.

---

## 1. Type Construction & Validation Tests

### 1.1 WorkspaceName

| ID | Test | Category | Expected |
|----|------|----------|----------|
| TN-001 | Valid lowercase alphanumeric name `"workspace"` | Happy path | Ok |
| TN-002 | Valid hyphenated name `"my-workspace-42"` | Happy path | Ok |
| TN-003 | Reject uppercase `"Workspace"` | Validation | Err(InvalidWorkspaceName) |
| TN-004 | Reject empty string `""` | Validation | Err(InvalidWorkspaceName) |
| TN-005 | Reject spaces `"my workspace"` | Validation | Err(InvalidWorkspaceName) |
| TN-006 | Reject special chars `"my@workspace"` | Validation | Err(InvalidWorkspaceName) |
| TN-007 | Reject name exceeding 64 bytes | Validation | Err(InvalidWorkspaceName) |
| TN-008 | Accept name at exactly 64 bytes | Boundary | Ok |
| TN-009 | Reject name starting with hyphen `"-leading"` | Validation | Err(InvalidWorkspaceName) |
| TN-010 | Reject name ending with hyphen `"trailing-"` | Validation | Err(InvalidWorkspaceName) |
| TN-011 | Reject consecutive hyphens `"double--hyphen"` | Validation | Err(InvalidWorkspaceName) |

### 1.2 WorkspacePath

| ID | Test | Category | Expected |
|----|------|----------|----------|
| TP-001 | Single-segment path `["root"]` | Happy path | Ok |
| TP-002 | Multi-segment path `["a", "b", "c"]` | Happy path | Ok |
| TP-003 | Reject empty segments list | Validation (INV-001) | Err |
| TP-004 | Reject segment containing empty string `["a", "", "c"]` | Validation | Err(EmptyPathSegment) |
| TP-005 | Path segments stored lowercase (case normalization) | INV-002 | Segments lowercased |
| TP-006 | Max depth 16 accepted | Boundary | Ok |
| TP-007 | Depth 17 rejected | Boundary | Err(PathTooDeep {max:16, actual:17}) |
| TP-008 | Equality is case-insensitive `["A"] == ["a"]` | INV-002 | true |
| TP-009 | Hash is case-insensitive | INV-002 | Same hash for "A" and "a" |

### 1.3 WorkspaceMetadata

| ID | Test | Category | Expected |
|----|------|----------|----------|
| TM-001 | Empty metadata | Happy path | Ok |
| TM-002 | Single entry | Happy path | Ok |
| TM-003 | Duplicate keys (BTreeMap deduplicates) | INV-009 | Last write wins |
| TM-004 | Key at 128 bytes accepted | Boundary | Ok |
| TM-005 | Key at 129 bytes rejected | Boundary | Err(MetadataKeyTooLong) |
| TM-006 | Value at 4096 bytes accepted | Boundary | Ok |
| TM-007 | Value at 4097 bytes rejected | Boundary | Err(MetadataValueTooLong) |
| TM-008 | 64 entries accepted | Boundary | Ok |
| TM-009 | 65 entries rejected | Boundary | Err(TooManyMetadataEntries) |

### 1.4 WorkspaceId

| ID | Test | Category | Expected |
|----|------|----------|----------|
| TI-001 | Generate unique IDs in sequence | Happy path | All distinct |
| TI-002 | IDs are time-ordered (ULID property) | Property | Monotonically sortable |
| TI-003 | Serde round-trip preserves value | Correctness | Eq |
| TI-004 | Display format is valid | Correctness | Parseable string |

---

## 2. WorkspaceIndex Lifecycle Tests

### 2.1 Initialization

| ID | Test | Category | Expected |
|----|------|----------|----------|
| IL-001 | New index has version 0 | Happy path | version == 0 |
| IL-002 | New index has empty nodes | Happy path | nodes.is_empty() |
| IL-003 | New index has empty root_ids | Happy path | root_ids.is_empty() |
| IL-004 | New index has empty path_index | Happy path | path_index.is_empty() |
| IL-005 | Operations on uninitialized index fail | State | Err(IndexNotInitialized) |

### 2.2 Insert

| ID | Test | Category | Expected |
|----|------|----------|----------|
| IN-001 | Insert root workspace (parent_id: None) | Happy path | Ok, returns WorkspaceId |
| IN-002 | Insert child under existing root | Happy path | Ok, child in parent's children list |
| IN-003 | Insert nested 3-level deep | Happy path | Correct path_index entry |
| IN-004 | Insert at max depth (16) | Boundary | Ok |
| IN-005 | Insert at depth 17 fails | Boundary | Err(PathTooDeep) |
| IN-006 | Insert with duplicate name under same parent | INV-007 | Err(DuplicateName) |
| IN-007 | Insert with same name under different parents | Happy path | Ok (names scoped to parent) |
| IN-008 | Insert generates path correctly from parent path | INV-004 | path_index[full_path] == id |
| IN-009 | Insert increments version | INV-013 | version += 1 |
| IN-010 | Insert with non-existent parent_id | NotFound | Err(ParentNotFound) |
| IN-011 | Insert sets created_at = updated_at | INV-010, INV-011 | Equal timestamps |
| IN-012 | Insert adds to root_ids when parent is None | INV-005 | root_ids contains new id |
| IN-013 | Insert does NOT add to root_ids when parent is Some | INV-005 | root_ids unchanged |
| IN-014 | Insert produces unique WorkspaceId | INV-003 | Distinct from all existing |
| IN-015 | Multiple root workspaces coexist | Happy path | All in root_ids |

### 2.3 Delete

| ID | Test | Category | Expected |
|----|------|----------|----------|
| DE-001 | Delete leaf node | Happy path | Ok, node removed |
| DE-002 | Delete node with single child | INV-015 | Both node and child removed |
| DE-003 | Delete node with deeply nested descendants | INV-015 | All descendants removed |
| DE-004 | Delete root workspace removes from root_ids | INV-005 | root_ids updated |
| DE-005 | Delete removes node from parent's children list | INV-007 | parent.children updated |
| DE-006 | Delete removes all descendant paths from path_index | INV-004 | path_index consistent |
| DE-007 | Delete non-existent workspace | NotFound | Err(WorkspaceNotFound) |
| DE-008 | Delete increments version | INV-013 | version += 1 |
| DE-009 | Delete root with 3-level subtree | INV-015, INV-019 | All 4 nodes removed |
| DE-010 | Delete child leaves sibling intact | INV-015 | Sibling unchanged |
| DE-011 | Deleted ID never reused by subsequent insert | INV-014 | New insert gets different ID |

### 2.4 Move

| ID | Test | Category | Expected |
|----|------|----------|----------|
| MO-001 | Move leaf to new parent | Happy path | parent_id updated, children lists updated |
| MO-002 | Move subtree (node + children) to new parent | INV-016 | All descendant paths updated |
| MO-003 | Move root to become child | Happy path | root_ids updated, parent_id set |
| MO-004 | Move child to become root (parent_id: None) | Happy path | root_ids updated, parent_id cleared |
| MO-005 | Move to self is rejected | INV-008 | Err(CyclicMoveDetected) |
| MO-006 | Move to own descendant is rejected | INV-008 | Err(CyclicMoveDetected) |
| MO-007 | Move to own grandchild is rejected | INV-008 | Err(CyclicMoveDetected) |
| MO-008 | Move non-existent workspace | NotFound | Err(WorkspaceNotFound) |
| MO-009 | Move to non-existent parent | NotFound | Err(ParentNotFound) |
| MO-010 | Move to parent that already has child with same name | Constraint | Err(DuplicateName) |
| MO-011 | Move preserves all metadata | Happy path | metadata unchanged |
| MO-012 | Move increments version | INV-013 | version += 1 |
| MO-013 | Move 3-level subtree updates all paths | INV-016 | path_index consistent for all descendants |
| MO-014 | Move to same parent (no-op move) | Edge case | Ok, no structural change |
| MO-015 | Move updates created_at/updated_at correctly | INV-010, INV-011 | created_at unchanged, updated_at >= created_at |

### 2.5 Update Metadata

| ID | Test | Category | Expected |
|----|------|----------|----------|
| UM-001 | Replace metadata entirely | Happy path | Old entries removed, new entries present |
| UM-002 | Set metadata to empty BTreeMap | Happy path | metadata.entries.is_empty() |
| UM-003 | Update non-existent workspace | NotFound | Err(WorkspaceNotFound) |
| UM-004 | Metadata key too long rejected | Validation | Err(MetadataKeyTooLong) |
| UM-005 | Metadata value too long rejected | Validation | Err(MetadataValueTooLong) |
| UM-006 | Too many metadata entries rejected | Validation | Err(TooManyMetadataEntries) |
| UM-007 | Update increments version | INV-013 | version += 1 |
| UM-008 | Update sets updated_at > created_at | INV-011 | updated_at > created_at |
| UM-009 | created_at unchanged after metadata update | INV-010 | Same as original |

---

## 3. Query Operation Tests

### 3.1 find_by_path

| ID | Test | Category | Expected |
|----|------|----------|----------|
| FP-001 | Find root by single-segment path | Happy path | Ok(id) |
| FP-002 | Find deeply nested node by full path | Happy path | Ok(id) |
| FP-003 | Find non-existent path | NotFound | Err(PathNotFound) |
| FP-004 | Case-insensitive lookup `"A/B"` == `"a/b"` | INV-002 | Same result |
| FP-005 | Path lookup is consistent with parent-chain traversal | INV-017 | Same as manual walk |

### 3.2 find_by_id

| ID | Test | Category | Expected |
|----|------|----------|----------|
| FI-001 | Find existing workspace by ID | Happy path | Ok(WorkspaceNode) |
| FI-002 | Find non-existent ID | NotFound | Err(WorkspaceNotFound) |
| FI-003 | Returned node has correct fields | Correctness | All fields match inserted data |

### 3.3 list_children

| ID | Test | Category | Expected |
|----|------|----------|----------|
| LC-001 | List children of leaf returns empty | INV-018 | Ok(vec![]) |
| LC-002 | List children of node with 3 children | Happy path | Ok(vec![id1, id2, id3]) |
| LC-003 | Children order matches insertion order | Correctness | Ordered as inserted |
| LC-004 | List children of non-existent node | NotFound | Err(WorkspaceNotFound) |
| LC-005 | After delete, children list updated | INV-007 | Deleted child removed |

### 3.4 get_ancestors

| ID | Test | Category | Expected |
|----|------|----------|----------|
| GA-001 | Ancestors of root is empty | Happy path | Ok(vec![]) |
| GA-002 | Ancestors of child returns [root_id] | Happy path | Ok(vec![root_id]) |
| GA-003 | Ancestors of 3-deep node returns [root, mid] | Happy path | Correct chain |
| GA-004 | Ancestors of non-existent node | NotFound | Err(WorkspaceNotFound) |
| GA-005 | After move, ancestors chain updated | Correctness | New parent in chain |

### 3.5 get_descendants

| ID | Test | Category | Expected |
|----|------|----------|----------|
| GD-001 | Descendants of leaf is empty | INV-019 | Ok(vec![]) |
| GD-002 | Descendants of parent includes direct children | INV-019 | Ok(vec![child_id]) |
| GD-003 | Descendants of root includes entire subtree | INV-019 | All nodes below root |
| GD-004 | Descendants order is depth-first | INV-019 | Parent before children |
| GD-005 | Descendants of non-existent node | NotFound | Err(WorkspaceNotFound) |
| GD-006 | After delete, descendants updated | INV-015 | Deleted subtree gone |

---

## 4. Invariant Verification Tests

These tests explicitly verify each invariant holds after specific operations.

### 4.1 Structural Invariants (INV-001 to INV-008)

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-001 | INV-001 | Construct WorkspacePath from NonEmptyVec; verify at least 1 segment |
| IV-002 | INV-002 | Insert with mixed-case path segments; verify stored lowercase; verify case-insensitive lookup |
| IV-003 | INV-003 | Insert 100 workspaces; verify all IDs unique via HashSet |
| IV-004 | INV-004 | After each mutation (insert/delete/move), rebuild path_index from nodes and compare |
| IV-005 | INV-005 | Insert root; verify parent_id is None; verify in root_ids |
| IV-006 | INV-006 | Insert child; verify parent_id is Some(existing_id) |
| IV-007 | INV-007 | For every node with parent_id Some(p), verify p.children contains node.id |
| IV-008 | INV-008 | Attempt move creating cycle; verify CyclicMoveDetected; verify ancestors() never contains self |

### 4.2 Metadata Invariants (INV-009 to INV-011)

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-009 | INV-009 | BTreeMap enforces uniqueness; verify with 2 inserts of same key |
| IV-010 | INV-010 | Insert node; capture created_at; update_metadata; verify created_at unchanged |
| IV-011 | INV-011 | Insert node; wait (mock clock); update; verify updated_at >= created_at |

### 4.3 Index Integrity Invariants (INV-012 to INV-014)

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-012 | INV-012 | Create snapshot; compute checksum; corrupt 1 byte; verify SnapshotCorrupted |
| IV-013 | INV-013 | Perform 50 mutations; verify version increments by exactly 1 each time |
| IV-014 | INV-014 | Insert A, delete A, insert B; verify A.id != B.id |

### 4.4 Operational Invariants (INV-015 to INV-019)

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-015 | INV-015 | Build 3-level tree; delete root; verify 0 nodes remain |
| IV-016 | INV-016 | Build subtree A/B/C; move A under new parent X; verify paths are X/A, X/A/B, X/A/C |
| IV-017 | INV-017 | For every node, compare find_by_path(path) with find_by_id(id) traversal |
| IV-018 | INV-018 | Verify children.is_empty() iff no node has this as parent_id |
| IV-019 | INV-019 | Build 4-level tree; get_descendants(root); verify count matches total - 1 |

---

## 5. Error Taxonomy Tests

Each error variant must be produced by at least one test.

| ID | Error Variant | Trigger |
|----|---------------|---------|
| ET-001 | WorkspaceNotFound | find_by_id with non-existent ID |
| ET-002 | PathNotFound | find_by_path with non-existent path |
| ET-003 | ParentNotFound | insert with non-existent parent_id |
| ET-004 | CyclicMoveDetected | move workspace to its own descendant |
| ET-005 | DuplicatePath | Insert path that already exists |
| ET-006 | DuplicateName | Insert child with same name as existing sibling |
| ET-007 | CannotDeleteWorkspaceWithInstances | Delete workspace with active instances (mock) |
| ET-008 | CannotDeleteWorkspaceWithChildren | Delete workspace that has children (if non-recursive) |
| ET-009 | InvalidWorkspaceName | Construct WorkspaceName with uppercase |
| ET-010 | EmptyPathSegment | Construct WorkspacePath with "" segment |
| ET-011 | PathTooDeep | Insert at depth > 16 |
| ET-012 | MetadataKeyTooLong | Key > 128 bytes |
| ET-013 | MetadataValueTooLong | Value > 4096 bytes |
| ET-014 | TooManyMetadataEntries | > 64 entries |
| ET-015 | IndexNotInitialized | Operate on index before init |
| ET-016 | SnapshotCorrupted | Load snapshot with wrong checksum |
| ET-017 | VersionMismatch | Load snapshot with version != expected |
| ET-018 | StorageWriteFailed | Mock storage failure on write |
| ET-019 | StorageReadFailed | Mock storage failure on read |

---

## 6. Property-Based Tests (proptest)

| ID | Property | Strategy |
|----|----------|----------|
| PP-001 | **Insert preserves all invariants** | Generate arbitrary insert sequences; after each, verify INV-001 through INV-019 |
| PP-002 | **Delete preserves all invariants** | Build tree; generate arbitrary delete sequence; verify invariants after each |
| PP-003 | **Move preserves all invariants** | Build tree; generate arbitrary valid move sequence; verify invariants |
| PP-004 | **Path-index consistency** | Arbitrary operation sequence; verify path_index matches node tree |
| PP-005 | **Monotonic versioning** | Arbitrary operation sequence; verify version strictly increases |
| PP-006 | **Ancestor chain is acyclic** | Build arbitrary tree; for each node, ancestors() never contains self |
| PP-007 | **Descendants are exhaustive** | For each node, descendants count = total subtree nodes - 1 |
| PP-008 | **find_by_path == find_by_id** | For every node, both lookups agree |
| PP-009 | **WorkspaceName validation** | Arbitrary string; if valid, Ok; if invalid, Err |
| PP-010 | **Metadata constraint boundaries** | Arbitrary key/value lengths; boundary behavior correct |

---

## 7. Snapshot & Recovery Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SR-001 | Create snapshot from empty index | Happy path | version=0, empty nodes |
| SR-002 | Create snapshot after 10 inserts | Happy path | All 10 nodes present, checksum valid |
| SR-003 | Load snapshot and verify invariants | Happy path | All INV-* hold |
| SR-004 | Load corrupted snapshot (bad checksum) | Recovery | Err(SnapshotCorrupted) |
| SR-005 | Load snapshot with version mismatch | Recovery | Err(VersionMismatch) |
| SR-006 | Snapshot checksum is deterministic | Correctness | Same index -> same checksum |
| SR-007 | Snapshot after delete includes only live nodes | INV-015 | Deleted nodes absent |
| SR-008 | Snapshot after move reflects new paths | INV-016 | Paths updated |

---

## 8. Multi-Operation Sequence Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SO-001 | Insert A, Insert B under A, Delete A | Sequence | Both A and B removed |
| SO-002 | Insert A, Insert B under A, Move B to root | Sequence | B is root, A has no children |
| SO-003 | Insert A, Insert B under A, Insert C under B, Move A under C | Sequence | Err(CyclicMoveDetected) |
| SO-004 | Build 16-level tree, delete level-8 node | Sequence | Levels 8-16 removed |
| SO-005 | Insert 100 roots, delete every other one | Sequence | 50 roots remain, invariants hold |
| SO-006 | Insert tree, move subtree 5 times | Sequence | Final tree state correct, all paths valid |
| SO-007 | Insert, update metadata, move, delete parent | Sequence | Metadata lost with node (no orphan data) |
| SO-008 | Concurrent-like interleaving: alternate insert and delete | Sequence | Index always consistent after each op |

---

## 9. Serde & Interop Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SE-001 | WorkspaceNode JSON round-trip | Serde | Eq after deserialize |
| SE-002 | WorkspaceIndexSnapshot JSON round-trip | Serde | Eq after deserialize |
| SE-003 | WorkspacePath JSON round-trip | Serde | Eq, case preserved |
| SE-004 | WorkspaceIndexError JSON round-trip | Serde | Eq after deserialize |
| SE-005 | Snapshot serialization is deterministic | Correctness | Same bytes for same state |

---

## 10. Edge Case Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| EC-001 | Insert into freshly created empty index | Edge | Works, version=1 |
| EC-002 | Delete the only root | Edge | Index empty, version incremented |
| EC-003 | Move to same parent (idempotent) | Edge | Ok, no change |
| EC-004 | Insert with max-length name (64 bytes) | Boundary | Ok |
| EC-005 | Find by path with single segment (root lookup) | Edge | Correct root returned |
| EC-006 | Insert child then immediately delete parent | Edge | Both removed atomically |
| EC-007 | Insert, move, then delete from new location | Edge | Correct cleanup |
| EC-008 | Metadata with exactly 64 entries, all at boundary sizes | Boundary | Ok |
| EC-009 | Tree with only roots (no nesting) | Edge | All invariants hold |
| EC-010 | Insert with metadata then update_metadata to empty | Edge | Metadata cleared |
| EC-011 | ULID monotonicity under rapid generation | Edge | IDs strictly ordered |
| EC-012 | Path with max-64-char segments at all 16 levels | Boundary | Ok |

---

## Test File Organization

```
crates/vo-types/src/
  workspace/
    mod.rs                          # Module root
    workspace_name.rs               # WorkspaceName type + tests (TN-*)
    workspace_path.rs               # WorkspacePath type + tests (TP-*)
    workspace_metadata.rs           # WorkspaceMetadata type + tests (TM-*)
    workspace_id.rs                 # WorkspaceId type + tests (TI-*)
    workspace_node.rs               # WorkspaceNode type
    workspace_index.rs              # WorkspaceIndex impl + tests (IN-*, DE-*, MO-*, UM-*)
    workspace_index_queries.rs      # Query tests (FP-*, FI-*, LC-*, GA-*, GD-*)
    workspace_index_invariants.rs   # Invariant verification tests (IV-*)
    workspace_index_errors.rs       # Error taxonomy tests (ET-*)
    workspace_index_snapshot.rs     # Snapshot/recovery tests (SR-*)
    workspace_index_proptest.rs     # Property-based tests (PP-*)
    workspace_index_sequences.rs    # Multi-operation sequence tests (SO-*)
    workspace_serde.rs              # Serde tests (SE-*)
    workspace_edge_cases.rs         # Edge case tests (EC-*)
```

## Test Count Summary

| Category | Count |
|----------|-------|
| Type construction & validation | 34 |
| Index lifecycle (insert/delete/move/update) | 46 |
| Query operations | 19 |
| Invariant verification | 19 |
| Error taxonomy | 19 |
| Property-based tests | 10 |
| Snapshot & recovery | 8 |
| Multi-operation sequences | 8 |
| Serde & interop | 5 |
| Edge cases | 12 |
| **Total** | **180** |
