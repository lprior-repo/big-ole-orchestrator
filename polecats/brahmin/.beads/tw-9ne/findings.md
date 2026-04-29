# Findings: tw-9ne - Add workflow history API endpoint

## Bead Summary
- **Title**: Add workflow history API endpoint
- **Description**: Implement GET /workflows/:id/history that returns all events for a workflow instance with pagination support

## Work Performed

### 1. Code Analysis
- **Endpoint Already Existed**: The `GET /api/v1/workflows/{id}/history` endpoint was already implemented in `crates/vo-api/src/handlers/query.rs`
- **Router Registration**: The route is registered at line 96-97 in `router.rs`
- **Missing Feature**: The endpoint lacked pagination support (offset/limit query parameters)

### 2. Changes Made

#### File: `crates/vo-api/src/types/v3.rs`
- Added `PaginationParams` struct for parsing offset/limit query parameters
- Updated `HistoryResponse` to include `total_count`, `offset`, and `limit` fields for pagination metadata

#### File: `crates/vo-api/src/handlers/query.rs`
- Added `use serde::Deserialize;` import
- Added `HistoryQueryParams` struct to parse `offset` and `limit` query params
- Modified `get_history` handler to:
  - Accept `AxumQuery<HistoryQueryParams>`
  - Apply default offset of 0 if not provided
  - Apply default limit of 100, max 1000
  - Return paginated response with total_count, offset, and limit metadata

### 3. Implementation Details
- Pagination is applied after collecting all events (via `replay_events_in_namespace`)
- Default limit is 100, maximum allowed is 1000
- Response includes `total_count` (total events available), `offset` (current page start), and `limit` (page size)
- Events are sliced using `.skip(offset).take(limit)` after collection

### 4. API Contract

**Endpoint**: `GET /api/v1/workflows/{namespace}/{instance_id}/history`

**Query Parameters**:
- `offset` (optional): Number of events to skip (default: 0)
- `limit` (optional): Maximum events to return (default: 100, max: 1000)

**Response**:
```json
{
  "instance_id": "namespace/instance_id",
  "entries": [...],
  "total_count": 150,
  "offset": 0,
  "limit": 100
}
```

## Verification
- Built successfully with `cargo build -p vo-api`
- No new compile errors introduced
- Existing warnings unchanged
