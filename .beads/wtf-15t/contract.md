# Contract Specification: get_journal handler

## Context
- Feature: HTTP GET /api/v1/workflows/:id/journal endpoint
- Location: vo-api/src/handlers/journal.rs
- Purpose: Replay and return all journal entries for a workflow instance

## Preconditions
- [ ] `Extension(master): Extension<ActorRef<OrchestratorMsg>>` is provided via Axum Extension layer
- [ ] `Path(id): Path<String>` contains a namespaced invocation ID (format: `namespace/instance_id`)
- [ ] Namespaced ID must not be empty or whitespace-only
- [ ] Namespaced ID must contain exactly one `/` separator

## Postconditions
- [ ] On success: Returns HTTP 200 with `JournalResponse { invocation_id, entries }`
- [ ] Entries are sorted by `seq` field in ascending order
- [ ] Each `JournalEntry` contains: seq, entry_type, name, input, output, timestamp, duration_ms, fire_at, status

## Invariants
- [ ] Response always has valid JSON structure even on error responses
- [ ] Content-Type is always application/json
- [ ] Sequence numbers in entries are monotonically increasing after sorting

## Error Taxonomy
- `ApiError::InvalidInput` (400) - when ID is empty, whitespace, or malformed
- `ApiError::NotFound` (404) - when event store returns error (workflow not found)
- `ApiError::ActorError` (500) - when event store is unavailable
- `ApiError::JournalError` (500) - when replay stream returns error

## Error Handling Flow
1. Parse ID -> 400 on failure
2. Get event store -> 500 if unavailable
3. Open replay stream -> 404 on failure
4. Iterate events -> 500 on iteration error
5. Return sorted entries -> 200 on success

## Non-goals
- [ ] Modifying workflow state
- [ ] Streaming responses (single JSON object only)
- [ ] Filtering or pagination of entries
