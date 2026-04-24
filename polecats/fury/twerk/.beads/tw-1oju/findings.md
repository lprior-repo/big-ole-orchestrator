# Findings: tw-1oju - node-api: align node get cli with server routes

## Issue Summary
The `twerk node get <id>` CLI command was calling `GET /nodes/{id}` but this endpoint did not exist on the server, causing a false NOT_FOUND error for valid node IDs.

## Root Cause
- CLI (`crates/twerk-cli/src/handlers/node.rs:73`): `node_get()` calls `GET /nodes/{id}`
- Server (`crates/twerk-web/src/api/router.rs`): Only `GET /nodes` was mounted (line 80), no `GET /nodes/{id}` route

## Fix Applied
Added `GET /nodes/{id}` endpoint to the server:

### Files Changed

1. **`crates/twerk-web/src/api/handlers/system.rs`**
   - Added `Path` to imports
   - Added `get_node_handler` async function that:
     - Takes `State<AppState>` and `Path<String>` (node ID)
     - Calls `state.ds.get_node_by_id(&id).await.map_err(ApiError::from)?`
     - Returns `axum::Json(node).into_response()`
   - Includes OpenAPI path specification for docs

2. **`crates/twerk-web/src/api/handlers/mod.rs`**
   - Added `get_node_handler` to the `pub use system::...` export list

3. **`crates/twerk-web/src/api/router.rs`**
   - Added `.route("/nodes/{id}", get(handlers::get_node_handler))` to the nodes route block

## Verification
- `cargo build -p twerk-web`: Compiles successfully
- `cargo build -p twerk-cli`: Compiles successfully
- The CLI now calls an endpoint that exists on the server

## Dolt Infrastructure Issue
**UNRESOLVED**: The `bd` tool cannot connect to the Dolt server due to project ID mismatch.
- metadata.json project ID: `e73a37e0-a1e9-417b-940b-bce186abda73`
- Database project ID: `af445fe7-feaa-48f5-b33b-258b66d93a10`
- Bead tw-1oju could NOT be closed via `bd close` due to this infrastructure issue
- Manual intervention required to close the bead in Dolt

## Status
Code fix COMPLETE. Bead closure FAILED due to Dolt infrastructure issues.