# Findings: tw-elkp - Wire SSE broadcaster to workflow event stream

## Problem
SSE endpoint (`GET /api/v1/workflows/:id/events`) was not forwarding workflow events to clients.

## Root Cause
The orchestrator did not have a broadcast channel for publishing events, and the SSE handler did not subscribe to any event stream.

## Solution Implemented

### 1. vo-actor: Added OrchestratorEvent and broadcast channel

**File**: `crates/vo-actor/src/master.rs`

- Added `OrchestratorEvent` enum with variants:
  - `InstanceStarted { namespace, instance_id, workflow_name, input }`
  - `SignalReceived { namespace, instance_id, signal_name, payload }`

- Added `event_broadcaster` field to `OrchestratorConfig`
- Added `broadcaster` field to `MasterState` as `broadcast::Sender<OrchestratorEvent>`
- Added `SubscribeEvents` message variant to `OrchestratorMsg`
- Added handler for `SubscribeEvents` that returns a `Receiver<OrchestratorEvent>`
- Added `get_broadcaster()` method to `MasterState`
- Emitting `InstanceStarted` in `CommitWorkflowStart` after instance is created
- Emitting `SignalReceived` in `CommitSignal` after signal is committed

**File**: `crates/vo-actor/src/lib.rs`
- Re-exported `OrchestratorEvent`

### 2. vo-api: Wired SSE handler to event stream

**File**: `crates/vo-api/src/handlers/sse.rs`

- Added `use vo_types::InstanceId` import
- Added `should_emit_for_instance` helper that filters events by namespace and instance_id
- Modified SSE handler to:
  1. Subscribe to orchestrator events via `OrchestratorMsg::SubscribeEvents`
  2. Spawn async task to forward matching events to the SSE broadcaster
  3. Merge SSE keepalive stream with event stream

### 3. vo-api: Fixed compilation errors

**File**: `crates/vo-api/src/handlers/query.rs`
- Changed `HistoryQueryParams` from private `struct` to `pub struct` (pre-existing bug)

## Key Technical Decisions

1. **Broadcast channel over direct actor messaging**: Used `tokio::sync::broadcast` since multiple SSE clients may need the same events
2. **Event emission at commit point**: Events emitted after database commit to ensure consistency
3. **Filter at SSE handler**: Events filtered by namespace/instance_id at subscription time using the existing `should_emit_for_instance` logic

## Compilation Status

- `cargo build --workspace`: ✅ SUCCESS (warnings only)
- `cargo check -p vo-api`: ✅ SUCCESS
- Clippy errors in vo-ipc and vo-types are pre-existing issues unrelated to this change

## Files Modified

- `crates/vo-actor/src/lib.rs` (+5 lines)
- `crates/vo-actor/src/master.rs` (+61 lines)
- `crates/vo-api/src/handlers/query.rs` (+1 line - visibility fix)
- `crates/vo-api/src/handlers/sse.rs` (+66 lines)

## Verification

The implementation can be verified by:
1. Start the server with `./target/debug/vo-cli serve`
2. Start a workflow
3. Query `GET /api/v1/workflows/:namespace/:id/events`
4. Emit a signal to that workflow
5. Observe SSE events being received
