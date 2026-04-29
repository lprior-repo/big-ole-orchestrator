# Bead tw-609 Findings: vo-cli history command pagination

## Summary

Implemented `--offset` and `--limit` flags for the `vo-cli history` command to support pagination.

## Changes Made

### 1. `crates/vo-cli/src/cli.rs`
- Added `offset: Option<u64>` and `limit: Option<u64>` fields to `Command::History` variant
- Added `--offset` and `--limit` CLI arguments to the history subcommand
- Updated history subcommand parsing to extract offset and limit values

### 2. `crates/vo-cli/src/commands/workflow_history.rs`
- Added `offset: Option<u64>` and `limit: Option<u64>` fields to `WorkflowHistoryConfig` struct
- Updated `fetch_workflow_history()` to accept offset and limit parameters
- Modified URL construction to include query parameters for pagination:
  - `?offset=N` when offset is provided
  - `?limit=N` when limit is provided
  - `?offset=N&limit=M` when both are provided

### 3. `crates/vo-cli/src/registry.rs`
- Updated `HistoryHandler` to pass offset and limit to `WorkflowHistoryConfig`

## Implementation Details

The pagination is implemented as query parameters passed to the API endpoint:
- URL format: `{engine_url}/api/v1/workflows/{instance_id}/history?offset={N}&limit={M}`
- Both parameters are optional
- If only offset is provided: returns events starting from offset
- If only limit is provided: returns up to limit events from start
- If both are provided: returns up to limit events starting from offset

## Verification

- Library compiles successfully: `cargo check -p vo-cli --lib`
- The pre-existing test failures are unrelated to these changes (they reference `Command::Purge` with old fields and `NodeCapability` which no longer exists in vo_types)

## Notes

- The bead description mentioned "payload" which maps to the existing `output` field in `WorkflowHistoryEntry`
- The existing `--json` and `--canonical` flags continue to work as before
- The implementation follows the existing code patterns in the codebase