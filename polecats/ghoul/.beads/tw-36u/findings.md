# Findings: Implement API error response standardization (tw-36u)

## Summary
Implemented API error response standardization by enhancing the `ApiError` type in `crates/vo-api/src/types/v3.rs`.

## Changes Made

### 1. Enhanced `ApiError` Struct
- Added `details: Option<serde_json::Value>` field with `#[serde(skip_serializing_if = "Option::is_none")]`
- Added documentation comments for all fields

### 2. New Methods on `ApiError`
- `with_details(mut self, details: serde_json::Value) -> Self` - Builder pattern to add structured details
- `status_code(&self) -> u16` - Maps error codes to HTTP status codes

### 3. HTTP Status Code Mapping
The `status_code()` method maps error codes:
| HTTP Status | Error Codes |
|-------------|-------------|
| 400 | invalid_namespace, invalid_instance_id, invalid_workflow_type, invalid_input, invalid_dedupe_key, invalid_paradigm, missing_dedupe_key, unknown_status_variant |
| 403 | workflow_quarantined, workflow_deactivated, forbidden |
| 404 | not_found, workflow_not_found, instance_not_found |
| 409 | already_exists, duplicate_ingress, conflict |
| 429 | budget_exhausted, writer_pressure_shed, workflow_cap_exceeded, too_many_requests |
| 500 | internal_error, event_persist_failed, actor_unavailable, actor_timeout, actor_error, spawn_failed, event_replay_failed, dedupe_storage_error, terminate_failed, compensation_failed, ghost_instance, invalid_config |
| 503 | global_concurrency_limit, at_capacity, service_unavailable |

### 4. New Tests Added
- `api_error_with_details` - Tests builder pattern
- `api_error_details_not_serialized_when_none` - Verifies details is omitted when None
- `api_error_details_serialized_when_present` - Verifies details is included when Some
- `api_error_status_code_*` - Tests for each HTTP status code mapping (not_found, bad_request, conflict, forbidden, too_many_requests, internal_error, service_unavailable)

### 5. Pre-existing Test Fixes
Fixed `V3StartRequest` test struct initializations in `v3.rs` that were missing the `workflow_binary_hash` field.

## Files Modified
- `crates/vo-api/src/types/v3.rs` - Main implementation

## Verification
- Library builds successfully: `cargo build -p vo-api --lib`
- Pre-existing issues in `sse.rs` (evt.data() calls) and separate `v3_test.rs` file were NOT modified as they are unrelated pre-existing issues

## Notes
- The `ApiError` is used consistently throughout the handlers via `ApiError::new(error_code, message)`
- The `with_details` method allows adding structured error context when needed
- The `status_code()` method provides a programmatic way to get the HTTP status code from an error code
