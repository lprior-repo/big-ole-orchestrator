# Contract Specification: Journal Replay Endpoint

## Endpoint

```
GET /api/v1/workflows/:id/journal
```

## Handler

`crates/wtf-api/src/handlers/journal.rs::get_journal`

## Type Signature

```rust
pub async fn get_journal(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse
```

## Request

| Field | Location | Type | Description |
|-------|----------|------|-------------|
| `id` | Path parameter | `String` | Namespaced instance ID in format `namespace/instance_id` (e.g., `payments/01ARZ3NDEKTSV4RRFFQ69G5FAV`) |

## Response

| Status | Body | Description |
|--------|------|-------------|
| 200 OK | `JournalResponse` | Events found (or empty list) |
| 400 Bad Request | `ApiError` | Handler-layer: empty or whitespace ID (only if routing passes) |
| 404 Not Found | `ApiError` | Routing-layer: invalid path structure (double-slash, missing segments) OR Handler-layer: instance not found |
| 500 Internal Server Error | `ApiError` | Event store unavailable or replay error |

### HTTP Status Code Decision Tree

```
Request arrives at router
        │
        ▼
┌───────────────────────────────────────┐
│ Does path match /api/v1/workflows/:id/journal? │
└───────────────────────────────────────┘
        │
   NO   │   YES
    ┌───┴────────┐
    ▼            ▼
  404         ┌───────────────────────────────────────┐
(routing)     │ Is :id parameter empty or whitespace? │
              └───────────────────────────────────────┘
                    │                    │
               YES  │                    │  NO
              ┌─────┴─────┐              ▼
              ▼           ▼     ┌───────────────────┐
            400           400   │ Instance exists    │
          (handler)    (handler) │ in event store?   │
                                └───────────────────┘
                                      │           │
                                 YES  │           │  NO
                                 ┌────┴───┐       ▼
                                 ▼        ▼     404
                               200      200   (handler)
                              (empty)  (events)
```

### Routing-Layer Rejections (404)

- **Double-slash in path**: `/api/v1/workflows//journal` → 404 (router rejects before handler)
- **Missing path segments**: `/api/v1/workflows/` → 404 (no `:id` parameter captured)
- **Wrong HTTP method**: Any non-GET → 405 Method Not Allowed

### Handler-Layer Rejections

| Condition | Status | Error Code |
|-----------|--------|------------|
| Empty string ID | 400 | `invalid_id` |
| Whitespace-only ID | 400 | `invalid_id` |
| ID without namespace separator | 400 | `invalid_id` |
| Instance not found in event store | 404 | `not_found` |

### JournalResponse

```rust
pub struct JournalResponse {
    pub invocation_id: String,
    pub entries: Vec<JournalEntry>,
}
```

### JournalEntry

```rust
pub struct JournalEntry {
    pub seq: u32,                    // Sequence number (1-indexed)
    pub entry_type: JournalEntryType, // Run | Wait
    pub name: Option<String>,        // Activity type or timer/signal name
    pub input: Option<serde_json::Value>,   // Activity input payload
    pub output: Option<serde_json::Value>,  // Activity result
    pub timestamp: Option<String>,   // RFC3339 formatted timestamp
    pub duration_ms: Option<u64>,    // Activity execution duration
    pub fire_at: Option<String>,      // Timer fire-at time (RFC3339)
    pub status: Option<String>,      // Event status (dispatched/completed/failed/scheduled/fired/signal)
}
```

### JournalEntryType

```rust
pub enum JournalEntryType {
    Run,   // Activity or signal event
    Wait,  // Timer event
}
```

## Preconditions

1. **Valid path structure**: The request path must be well-formed (routing layer)
2. **ID format**: If routing passes, the `id` path parameter must be non-empty and match the format `namespace/instance_id`
3. **ID whitespace**: If routing passes, the ID must not be empty or whitespace-only after trimming
4. **Event store availability**: The orchestrator's event store must be accessible

## Postconditions

1. **Success (200)**:
   - Returns a `JournalResponse` with the requested `invocation_id`
   - `entries` contains all journal entries for the instance
   - Entries are sorted by `seq` in ascending order
   - If no events exist, returns empty `entries` vector (not an error)
2. **Invalid Path Structure (404)**: Routing-layer rejects malformed paths before handler runs
3. **Invalid ID (400)**: Returns `ApiError` with code `"invalid_id"` and message `"empty invocation id"` or `"bad id"` (only if routing passes)
4. **Instance Not Found (404)**: Returns `ApiError` with code `"not_found"` and the requested ID in the message
5. **Storage Error (500)**: Returns `ApiError` with code `"actor_error"` or `"journal_error"`

## Invariants

1. **Sorted output**: All returned `JournalEntry` records MUST be sorted by `seq` in ascending order
2. **Valid sequence numbers**: `seq` values must be positive integers starting from 1
3. **Complete event mapping**: Each `WorkflowEvent` type maps to exactly one `JournalEntry`

## Error Taxonomy

| Error Code | HTTP Status | Layer | Condition |
|------------|-------------|-------|-----------|
| `invalid_id` | 400 | Handler | Empty or whitespace instance ID |
| `not_found` | 404 | Handler | Instance ID not found in event store |
| `route_not_found` | 404 | Routing | Invalid path structure (double-slash, missing segments) |
| `method_not_allowed` | 405 | Routing | Wrong HTTP method |
| `actor_error` | 500 | Handler | Event store unavailable |
| `journal_error` | 500 | Handler | Error during journal replay |

## Event Type Mapping

| WorkflowEvent | JournalEntryType | Status |
|---------------|------------------|--------|
| ActivityDispatched | Run | "dispatched" |
| ActivityCompleted | Run | "completed" |
| ActivityFailed | Run | "failed" |
| TimerScheduled | Wait | "scheduled" |
| TimerFired | Wait | "fired" |
| SignalReceived | Run | "signal" |
| Other variants | Run | "recorded" |

## Ownership Contract

- The handler does not retain ownership of any input parameters beyond the async scope
- Event store reference is acquired via `get_event_store()` and released after use
- All allocations (entries vector) are owned by the response construction

## Violation Examples

1. **Precondition violation (routing)**: Double-slash in path → 404 with `"route_not_found"`
2. **Precondition violation (routing)**: Missing `:id` segment → 404 with `"route_not_found"`
3. **Precondition violation (handler)**: Empty ID string → 400 with `"invalid_id"`
4. **Precondition violation (handler)**: Whitespace-only ID → 400 with `"invalid_id"`
5. **Postcondition violation**: Unsorted entries → Sort occurs before response
6. **Invariant violation**: Negative sequence number → Map to `u32::MAX`
7. **Not found (handler)**: Valid format but non-existent instance → 404 with `"not_found"`
