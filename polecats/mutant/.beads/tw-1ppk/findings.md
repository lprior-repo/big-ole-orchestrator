# Findings: Implement workflow definition hot reload (tw-1ppk)

## Implementation Summary

Created a new module `vo-core/src/workflow_definition_hot_reload/` that implements hot reloading of workflow definitions without server restart.

## Architecture

The hot reload system consists of three main components:

### 1. WorkflowDefinitionRegistry (`registry.rs`)
- Thread-safe in-memory registry for validated `WorkflowDefinition` objects
- Uses `Arc<RwLock>` for concurrent access
- Maintains bidirectional mapping between workflow names and binary paths
- Public API:
  - `register(workflow_name, definition, binary_path)` - Register a new or updated definition
  - `get(&WorkflowName)` - Get definition by name
  - `get_binary_path(&WorkflowName)` - Get binary path by name
  - `get_by_binary_path(&PathBuf)` - Get workflow name by binary path
  - `remove(&WorkflowName)` - Remove a workflow definition
  - `list_workflows()` - List all registered workflows
  - `len()` / `is_empty()` - Registry statistics

### 2. WorkflowDefinitionLoader (`loader.rs`)
- Loads workflow definitions by executing binaries with `--graph` flag
- Parses JSON output using `WorkflowDefinition::from_deserializer`
- Validates definitions before registration
- Error handling: timeouts (10s), stderr capture, validation failures
- Methods:
  - `load_from_binary(path)` - Initial load from binary
  - `reload_from_binary(path)` - Hot reload when binary changes
  - `parse_workflow_definition(json_bytes)` - Validates JSON into WorkflowDefinition

### 3. WorkflowDefinitionWatcher (`watcher.rs`)
- File watcher using `notify` crate for filesystem events
- Debounced (300ms) to coalesce rapid changes
- Filters events to only workflow binaries (wasm/elf/exe/bin extensions)
- Handles Modify events by triggering reload
- Returns results via channel for async consumption

## Key Design Decisions

### Why watch workflow binaries?
Workflow binaries in `./data/workflows/` are the canonical source of truth. When a binary changes:
1. Run it with `--graph` to get the new definition
2. Validate the JSON output
3. If valid: update registry and log success
4. If invalid: keep old definition and log error

### Why not use existing config_hot_reload infrastructure?
The existing `HotReloadConfig<T>` is designed for single config files. Workflow definitions are binaries that emit JSON when invoked with `--graph`, requiring different handling:
- Binary execution (not file parsing)
- Separate validation step
- Different error handling

### Thread-safety
All registry operations use `RwLock` for safe concurrent access from multiple tasks.

## Files Created

```
crates/vo-core/src/workflow_definition_hot_reload/
├── mod.rs           - Module exports
├── error.rs         - Error types (SpawnFailed, BinaryTimeout, ValidationFailed, etc.)
├── registry.rs      - Thread-safe WorkflowDefinition registry
├── loader.rs        - Binary execution and definition loading
├── watcher.rs       - Filesystem watcher with debouncing
└── tests.rs        - Unit tests
```

## Integration Notes

The module is designed to be integrated into `vo-cli/src/commands/serve.rs` by:
1. Creating a shared registry with `create_shared_registry()`
2. Spawning a `WorkflowDefinitionWatcher` on the workflows directory
3. Adding the registry to `AppState` so handlers can query it

Example integration (not fully wired due to vo-cli compilation issues):
```rust
let workflow_registry = vo_core::workflow_definition_hot_reload::create_shared_registry();
let (_watcher, mut watcher_rx) = WorkflowDefinitionWatcher::new(
    workflows_dir,
    workflow_registry.clone(),
)?;
// Spawn task to handle watcher events
tokio::spawn(async move {
    while let Some(result) = watcher_rx.recv().await {
        match result {
            Ok(path) => tracing::info!(path = %path.display(), "workflow reloaded"),
            Err(e) => tracing::error!(error = %e, "workflow reload failed"),
        }
    }
});
```

## Test Results

All 7 unit tests pass:
- `registry_starts_empty`
- `registry_is_empty_after_creation`
- `registry_get_returns_none_for_unknown_workflow`
- `registry_contains_returns_false_for_unknown_workflow`
- `registry_get_binary_path_returns_none_for_unknown_workflow`
- `registry_get_by_binary_path_returns_none_for_unknown_path`
- `registry_list_workflows_returns_empty_for_new_registry`

## Pre-existing Issue

The `vo-cli` crate has a compilation error in `execute_with_graph` function (unrelated to this implementation) that prevents full workspace build. This is a pre-existing issue in the codebase.

## Future Work

1. Wire hot reload into serve.rs with proper AppState integration
2. Add SSE event emission when definitions change (so frontend can update)
3. Add metrics for reload success/failure rates
4. Consider garbage collection for deleted workflows (similar to GhostLifecycle)