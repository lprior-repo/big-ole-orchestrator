# Bead tw-c253 Findings: Add workflow definition schema versioning

## Summary
Implemented schema versioning for `WorkflowDefinition` in vo-types crate to support migration from v1 (no schema_version) to v2 (schema_version = 2).

## Changes Made

### 1. Updated MAX_SUPPORTED_SCHEMA_VERSION (types.rs:74)
- Changed from `1` to `2` to support schema version 2
- This affects State, WorkflowSpec, Snapshot, and now WorkflowDefinition

### 2. Added schema_version field to WorkflowDefinition (workflow/mod.rs:83-88)
```rust
pub struct WorkflowDefinition {
    pub workflow_name: WorkflowName,
    pub schema_version: u16,  // NEW FIELD
    pub nodes: NonEmptyVec<DagNode>,
    pub edges: Vec<Edge>,
}
```

### 3. Added new error variants to WorkflowDefinitionError (workflow/mod.rs:67-73)
```rust
/// Schema version is unsupported.
#[error("unsupported workflow schema version: {version}")]
UnsupportedSchemaVersion { version: u16 },

/// Schema version is missing (v1 format without version field).
#[error("workflow schema version missing; migration from v1 to v2 required")]
MissingSchemaVersion,
```

### 4. Updated UnvalidatedWorkflow for schema_version (workflow/mod.rs:300-307)
- Added `schema_version: Option<u16>` with `#[serde(default)]` for optional deserialization
- v1 workflows (no schema_version field) will deserialize to None

### 5. Implemented migration logic in validate_unvalidated (workflow/mod.rs:167-180)
```rust
let schema_version = match unvalidated.schema_version {
    Some(v) => {
        if v > MAX_SUPPORTED_SCHEMA_VERSION {
            return Err(WorkflowDefinitionError::UnsupportedSchemaVersion { version: v });
        }
        v
    }
    None => MAX_SUPPORTED_SCHEMA_VERSION,  // Auto-upgrade v1 to v2
};
```

### 6. Added Serialize impl (workflow/mod.rs:247-260)
- Serializes schema_version as explicit field in JSON output

### 7. Added Deserialize impl (workflow/mod.rs:262-276)
- Deserializes via UnvalidatedWorkflow and validates via validate_unvalidated

### 8. Updated test helpers across 11 files
- Added `schema_version: 2` to all direct WorkflowDefinition constructions
- Files updated:
  - workflow_tests.rs
  - tests_bdd_dag_cycle_validation.rs
  - tests_bdd_dag_connectivity.rs
  - tests_bdd_dag_merge_point.rs
  - proptest_domain_types.rs
  - proptest_dag_correctness.rs
  - proptest_dag_correctness_2.rs
  - next_step_selection.rs
  - dependency_graph_resolver_tests.rs
  - red_queen_tests/next_nodes.rs
  - red_queen_tests/boundary_values.rs
  - red_queen_tests/helpers.rs

## Migration Behavior

| Input | Behavior |
|-------|----------|
| v1 (no schema_version field) | Auto-upgraded to schema_version = 2 |
| v2 (schema_version = 2) | Accepted as-is |
| v0 or v1 with explicit version | Accepted if <= MAX_SUPPORTED_SCHEMA_VERSION |
| v3+ (schema_version > 2) | Rejected with UnsupportedSchemaVersion error |

## JSON Format Example

v2 workflow definition:
```json
{
  "schema_version": 2,
  "workflow_name": "my-workflow",
  "nodes": [...],
  "edges": [...]
}
```

## Validation
- Schema version validated on load
- Versions > MAX_SUPPORTED_SCHEMA_VERSION rejected with clear error
- Missing schema_version triggers auto-upgrade (not error)

## Notes
- The `MissingSchemaVersion` error variant is retained for potential future use but is not triggered in current implementation since missing schema_version auto-upgrades to MAX_SUPPORTED_SCHEMA_VERSION
- All direct WorkflowDefinition constructions in tests now use schema_version = 2