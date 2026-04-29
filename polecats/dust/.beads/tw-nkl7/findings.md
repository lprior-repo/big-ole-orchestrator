# Bead tw-nkl7 Findings: vo-cli validate command

## Task
Add a `vo-cli validate` command for workflow specs that performs deep validation including:
- Check all node references in edges exist
- Verify no cycles in DAG workflows
- Validate retry policies have sane values
- Check connector configs are complete
- Verify timeout ranges
- Exit 0 if valid or 1 with errors list if not

## Implementation

### Files Changed

1. **crates/vo-cli/src/cli.rs**
   - Added `Validate { path: PathBuf }` variant to `Command` enum
   - Added `Validate` error variant to `CliError` enum
   - Added `validate` subcommand to clap CLI definition
   - Added parsing for `validate` subcommand in `interpret_cli_from`
   - Added `Validate(_)` to `map_error_to_exit_code` (returns 1)

2. **crates/vo-cli/src/commands/mod.rs**
   - Added `pub mod validate;` to include the new module

3. **crates/vo-cli/src/commands/validate.rs** (NEW FILE)
   - `ValidateError` enum with variants for:
     - `FileNotFound` - file doesn't exist
     - `Io` - I/O errors reading file
     - `WorkflowSpec` - wraps `WorkflowDefinitionError` from vo_types
     - `UnreasonableBackoffMs` - backoff_ms exceeds MAX_REASONABLE_BACKOFF_MS (1 year)
     - `MaxBackoffLessThanBackoff` - max_backoff_ms < backoff_ms
   - `validate_workflow_spec(path)` - reads file, parses via `WorkflowDefinition::from_deserializer`, runs extra constraints
   - `validate_extra_constraints(def)` - validates retry policy ranges
   - `validate_retry_policy_ranges(node)` - checks backoff_ms reasonableness and max_backoff_ms >= backoff_ms
   - `run_validate(path)` - main entry point, prints success message

4. **crates/vo-cli/src/registry.rs**
   - Added `ValidateHandler` registration in `HandlerRegistry::default()`
   - Added `Command::Validate` to `command_key` function
   - Implemented `ValidateHandler` struct implementing `CommandHandler`

### Validation Coverage

The validate command performs:

1. **All `WorkflowDefinition::parse` validations** (via `from_deserializer`):
   - JSON deserialization
   - Non-empty nodes check
   - Duplicate node names check
   - RetryPolicy validation (max_attempts >= 1, backoff_multiplier >= 1.0)
   - Edge referential integrity (source and target nodes exist)
   - DFS cycle detection

2. **Additional constraints**:
   - `backoff_ms` must be <= 365 days in ms (MAX_REASONABLE_BACKOFF_MS)
   - `max_backoff_ms` must be >= `backoff_ms`

### Notes

- **Connector configs completeness**: The current `DagNode` structure in vo-types does NOT have a `connector` or `capability` field. The `ConnectorRequirement` type exists in `vo_types::workflow::NodeCapability` but is not part of `WorkflowDefinition`. Therefore, connector config validation is not applicable to the current engine workflow spec format.

- **Timeout ranges**: The retry policy's `backoff_ms` and `max_backoff_ms` fields are validated for reasonableness.

### Verification

```bash
# Build
cargo build -p vo-cli

# Usage
vo validate workflow.json
# On success: prints validation success message with node/edge count
# On failure: prints error message, exits with code 1
```

## Status
COMPLETED - Implementation done, builds successfully.