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
| 400 Bad Request | `ApiError` | Invalid or empty ID |
| 404 Not Found | `ApiError` | Instance not found in storage |
| 500 Internal Server Error | `ApiError` | Event store unavailable or replay error |

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

1. **ID format**: The `id` path parameter must be non-empty and match the format `namespace/instance_id`
2. **ID whitespace**: The ID must not be empty or whitespace-only after trimming
3. **Event store availability**: The orchestrator's event store must be accessible

## Postconditions

1. **Success (200)**:
   - Returns a `JournalResponse` with the requested `invocation_id`
   - `entries` contains all journal entries for the instance
   - Entries are sorted by `seq` in ascending order
   - If no events exist, returns empty `entries` vector (not an error)
2. **Invalid ID (400)**: Returns `ApiError` with code `"invalid_id"` and message `"empty invocation id"` or `"bad id"`
3. **Instance Not Found (404)**: Returns `ApiError` with code `"not_found"` and the requested ID in the message
4. **Storage Error (500)**: Returns `ApiError` with code `"actor_error"` or `"journal_error"`

## Invariants

1. **Sorted output**: All returned `JournalEntry` records MUST be sorted by `seq` in ascending order
2. **Valid sequence numbers**: `seq` values must be positive integers starting from 1
3. **Complete event mapping**: Each `WorkflowEvent` type maps to exactly one `JournalEntry`

## Error Taxonomy

| Error Code | HTTP Status | Condition |
|------------|-------------|-----------|
| `invalid_id` | 400 | Empty or malformed instance ID |
| `not_found` | 404 | Instance ID not found in event store |
| `actor_error` | 500 | Event store unavailable |
| `journal_error` | 500 | Error during journal replay |

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

1. **Precondition violation**: Empty ID string → 400 with `"invalid_id"`
2. **Precondition violation**: Whitespace-only ID → 400 with `"invalid_id"`
3. **Postcondition violation**: Unsorted entries → Sort occurs before response
4. **Invariant violation**: Negative sequence number → Map to `u32::MAX`
5. **Not found**: Valid format but non-existent instance → 404 with `"not_found"`
