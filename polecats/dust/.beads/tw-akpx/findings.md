# Findings: tw-akpx - Propagate command_metadata into signal events

## Task
When creating signal-related events in signal.rs, extract command_metadata from CommandEnvelope and populate EventMetadata.command_metadata instead of None.

## Changes Made

### 1. Extended V3SignalRequest (crates/vo-api/src/types/v3.rs)
Added `command_envelope: Option<vo_types::CommandEnvelope>` field to V3SignalRequest struct.
This allows clients to pass a command envelope with signal requests.

### 2. Updated signal.rs (crates/vo-api/src/handlers/signal.rs)
- Added import for `CommandMetadata` from vo_types
- Modified `send_signal` handler to extract command_metadata from the request's command_envelope:
  ```rust
  let command_metadata = req.command_envelope.as_ref().map(|env| env.metadata.clone());
  ```
- Updated `persist_lifecycle_event` function signature to accept `command_metadata: Option<CommandMetadata>` parameter
- Updated `EventMetadata` construction to use the passed command_metadata instead of None

### 3. Fixed v3_test.rs
Updated `V3SignalRequest` test construction to include `command_envelope: None`.

## How It Works
1. Client includes optional `command_envelope` in V3SignalRequest
2. Handler extracts `command_metadata` (via `env.metadata.clone()`)
3. `persist_lifecycle_event` receives the command_metadata and passes it to EventMetadata
4. The SignalAccepted event now carries proper command lineage

## Compilation Status
- vo-api lib compiles successfully with `cargo check -p vo-api --lib`
- Pre-existing test failures in vo-api tests (unrelated to this change - they have missing fields like `workflow_binary_hash` that were broken before this change)

## Files Modified
- crates/vo-api/src/types/v3.rs
- crates/vo-api/src/handlers/signal.rs
- crates/vo-api/src/types/v3_test.rs