# Implementation Summary

## Bead: wtf-c0l
## Title: Implement get_journal handler

## What was implemented

- Added new handler module: `crates/wtf-api/src/handlers/journal.rs`
  - Introduced `get_journal(Extension(master), Path(id))` endpoint logic.
  - Implements validation-order requirement: invocation id is validated before store/actor access.
  - Supports replay-based journal retrieval through `EventStore::open_replay_stream`.
  - Maps replay events to `JournalEntry` DTOs and sorts by ascending `seq` before returning.
  - Error mapping:
    - invalid/empty id → `400 Bad Request`
    - replay stream open failure (missing workflow) → `404 Not Found`
    - replay read failure / unavailable store → `500 Internal Server Error`

- Wired handler into API surface:
  - `crates/wtf-api/src/handlers/mod.rs` exports `journal` module.
  - `crates/wtf-api/src/routes.rs` adds route:
    - `GET /api/v1/workflows/:id/journal`
  - `crates/wtf-api/src/app.rs` adds the same route in app assembly.

- Extended response type ergonomics:
  - `crates/wtf-api/src/types/responses.rs`
    - `JournalResponse.invocation_id` now uses `String` to support namespaced path ids.
    - added `JournalResponse::new(invocation_id, entries)` constructor.

- Added focused tests for request-id validation in `journal.rs`:
  - empty id rejected
  - whitespace id rejected
  - valid namespaced id accepted

## Contract adherence notes

- P1 enforced first (`parse_journal_request_id`) before any event-store lookup.
- Q4 enforced by sorting entries by `seq` before response.
- Explicit error taxonomy implemented via HTTP status mapping.
- No panics/unwraps used in handler implementation path.

## Verification

- `cargo test -p wtf-api -- --nocapture`

## Iteration 2 (contract hardening)

- Extracted deterministic sorter helper for journal entries and added unit coverage:
  - `sort_entries_by_seq(Vec<JournalEntry>) -> Vec<JournalEntry>`
  - test ensures unsorted inputs are returned in ascending sequence order

- Verification rerun:
  - `cargo test -p wtf-api -- --nocapture`

## Files changed

- `crates/wtf-api/src/handlers/journal.rs` (new)
- `crates/wtf-api/src/handlers/mod.rs`
- `crates/wtf-api/src/routes.rs`
- `crates/wtf-api/src/app.rs`
- `crates/wtf-api/src/types/responses.rs`
- `crates/wtf-api/src/types/mod.rs`
