# Martin Fowler Test Plan: Journal Replay Endpoint

## Test Suite: GET /api/v1/workflows/:id/journal

### 1. Happy Path Tests

#### GIVEN a valid namespaced instance ID with multiple journal events
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the response status is 200 OK
#### AND the response body is a JournalResponse with matching invocation_id
#### AND entries are sorted by seq in ascending order
#### AND each entry contains valid JournalEntry fields

#### GIVEN a valid namespaced instance ID (e.g., "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV")
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the returned invocation_id equals the requested ID

#### GIVEN a valid instance with events of mixed types (ActivityDispatched, TimerScheduled, ActivityCompleted)
#### WHEN journal is requested
#### THEN all events are returned with correct JournalEntryType (Run for activities/signals, Wait for timers)

---

### 2. Empty Path Tests

#### GIVEN a valid namespaced instance ID with zero journal events
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the response status is 200 OK
#### AND the response body has entries array with length 0
#### AND the invocation_id matches the requested ID

#### GIVEN an instance that exists but has no recorded events
#### WHEN journal is requested
#### THEN returns empty entries array (NOT an error)

---

### 3. Not Found Path Tests

#### GIVEN a valid-format but non-existent instance ID
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the response status is 404 Not Found
#### AND the error code is "not_found"
#### AND the error message contains the requested ID

#### GIVEN an instance ID with valid namespace but unknown instance
#### WHEN journal is requested
#### THEN returns 404 (not 400)

---

### 4. Routing-Layer Rejection Tests (Path Structure)

#### GIVEN a request with double-slash in path (e.g., `/api/v1/workflows//journal`)
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the response status is 404 Not Found
#### AND the error code is "route_not_found"
#### AND the handler is NOT invoked

#### GIVEN a request with missing ID segment (e.g., `/api/v1/workflows//journal`)
#### WHEN the router cannot match the route pattern
#### THEN the response status is 404 Not Found
#### AND the error code is "route_not_found"

---

### 5. Handler-Layer Input Validation Tests

#### GIVEN an empty string as the instance ID (routing passes, handler receives empty string)
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the response status is 400 Bad Request
#### AND the error code is "invalid_id"
#### AND the error message is "empty invocation id"

#### GIVEN a whitespace-only string as the instance ID
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the response status is 400 Bad Request
#### AND the error code is "invalid_id"

#### GIVEN an ID without namespace separator (e.g., "01ARZ3NDEKTSV4RRFFQ69G5FAV")
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the response status is 400 Bad Request
#### AND the error code is "invalid_id"

---

### 6. Edge Case: Single Event

#### GIVEN a valid instance ID with exactly one journal event
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the response contains exactly one entry
#### AND the single entry has seq value of 1

---

### 7. Edge Case: Large Event List

#### GIVEN a valid instance ID with many events (100+)
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN all events are returned in the single response
#### AND entries are sorted by seq ascending
#### AND no events are dropped or duplicated

---

### 8. Sorting Verification Tests

#### GIVEN events are stored out-of-order in the journal
#### WHEN journal is requested
#### THEN returned entries are sorted by seq ascending (invariant)

#### GIVEN entries with seq values [5, 2, 8, 1, 9]
#### WHEN journal is requested
#### THEN returned order is [1, 2, 5, 8, 9]

---

### 9. Storage Error Tests

#### GIVEN the event store is unavailable
#### WHEN GET /api/v1/workflows/:id/journal is called
#### THEN the response status is 500 Internal Server Error
#### AND the error code is "actor_error"
#### AND the error message contains "event store unavailable"

#### GIVEN the event store returns an error during replay
#### WHEN journal is requested
#### THEN the response status is 500 Internal Server Error
#### AND the error code is "journal_error"

---

### 10. Contract Verification Tests

#### GIVEN a successful journal response
#### THEN validate() on JournalResponse returns Ok(()) (entries are sorted)

#### GIVEN the response entries are manipulated to be unsorted
#### THEN validate() returns Err(InvariantViolation::EntriesNotSorted)

---

### 11. Event Field Mapping Tests

#### GIVEN an ActivityDispatched event with activity_type="process_payment" and payload={"amount": 100}
#### WHEN mapped to JournalEntry
#### THEN entry_type is JournalEntryType::Run
#### AND name is Some("process_payment")
#### AND input is Some({"amount": 100})
#### AND status is Some("dispatched")

#### GIVEN an ActivityCompleted event with result={"status": "success"} and duration_ms=150
#### WHEN mapped to JournalEntry
#### THEN output is Some({"status": "success"})
#### AND duration_ms is Some(150)
#### AND status is Some("completed")

#### GIVEN a TimerScheduled event with timer_id="timeout-1" and fire_at timestamp
#### WHEN mapped to JournalEntry
#### THEN entry_type is JournalEntryType::Wait
#### AND name is Some("timeout-1")
#### AND fire_at contains the scheduled time

---

### 12. Response Structure Tests

#### GIVEN a valid journal response
#### THEN the JSON contains "invocation_id" field
#### AND the JSON contains "entries" array field

#### GIVEN a JournalEntry with all optional fields present
#### THEN all fields serialize correctly (no skip_serializing_if triggers)

#### GIVEN a JournalEntry with no optional fields
#### THEN only seq and entry_type appear in JSON output
