# tw-x4e6: Integration test for dedupe_key rejection on duplicate workflow start

## Summary

Created `crates/vo-api/tests/dedupe_key_rejection_tests.rs` with 6 BDD-style integration tests verifying dedupe_key behavior on the workflow start endpoint.

## Tests Implemented

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 1 | `given_valid_dedupe_key_when_start_workflow_then_201_created` | Start with dedupe_key "order-123" | 201 Created |
| 2 | `given_duplicate_dedupe_key_when_start_workflow_then_409_conflict` | Start again with same key | 409 Conflict, error="already_exists" |
| 3 | `given_different_dedupe_key_when_start_workflow_then_201_created` | Start with different key "order-456" | 201 Created |
| 4 | `given_missing_dedupe_key_when_start_workflow_then_400_bad_request` | No dedupe_key field | 400 Bad Request, error="missing_dedupe_key" |
| 5 | `given_empty_dedupe_key_when_start_workflow_then_400_bad_request` | Empty string dedupe_key | 400 Bad Request |
| 6 | `given_third_duplicate_dedupe_key_when_start_workflow_then_409_conflict` | Third attempt with same key | 409 Conflict |

## Approach

Used the existing integration test pattern from `qa_api.rs` and `redqueen_api.rs`:
- `tower::ServiceExt::oneshot` for sending requests through the router
- Stub handler with `Arc<RwLock<HashSet<String>>>` to track seen dedupe keys
- BDD naming convention: `given_<condition>_when_<action>_then_<result>`

## Key Findings

- The production handler (`workflow.rs:58-70`) validates dedupe_key presence and returns 400
- `StartError::AlreadyExists` maps to 409 CONFLICT (`workflow.rs:186-192`)
- The stub faithfully mirrors the production handler's error envelope shape (`ApiError { error, message }`)
- All 6 tests pass: `cargo test -p vo-api --test dedupe_key_rejection_tests`

## File Changed

- `crates/vo-api/tests/dedupe_key_rejection_tests.rs` (new, 194 lines)
