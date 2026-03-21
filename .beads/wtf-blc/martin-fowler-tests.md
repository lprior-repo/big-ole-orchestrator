# Martin Fowler Test Plan

## Metadata
- bead_id: wtf-blc
- bead_title: Define HTTP request/response types for API
- phase: 1
- updated_at: 2026-03-20T00:00:00Z

## Test Categories Summary

| Category | Count | Purpose |
|---|---|---|
| Unit Tests | 45 | Type construction, parsing, validation |
| Contract Verification Tests | 12 | Pre/post/invariant enforcement |
| Contract Violation Tests | 15 | Each violation example has a test |
| Integration Tests | 6 | End-to-end HTTP request/response |
| **Total** | **78** | Full behavior specification |

---

## Unit Tests

### Happy Path Tests

#### StartWorkflowRequest
- **test_start_workflow_request_deserializes_valid_json**
  - **Given**: Valid JSON `{ "workflow_name": "checkout", "input": { "order_id": "ord_123" } }`
  - **When**: Deserialize to `StartWorkflowRequest`
  - **Then**: `workflow_name.as_str() == "checkout"`, `input` contains `order_id` field

- **test_start_workflow_request_with_complex_input**
  - **Given**: JSON with nested objects, arrays, and strings
  - **When**: Deserialize
  - **Then**: All nested values preserved in `serde_json::Value`

#### StartWorkflowResponse
- **test_start_workflow_response_serializes_to_json**
  - **Given**: `StartWorkflowResponse` with valid `InvocationId`, `WorkflowStatusValue::Running`
  - **When**: Serialize to JSON
  - **Then**: Valid JSON object with all fields present

- **test_start_workflow_response_timestamp_is_rfc3339**
  - **Given**: `StartWorkflowResponse` with `started_at`
  - **When**: Serialize
  - **Then**: `started_at` matches RFC3339 pattern `YYYY-MM-DDTHH:MM:SSZ`

- **test_start_workflow_response_status_is_running_on_success**
  - **Given**: Successful workflow start
  - **When**: Construct `StartWorkflowResponse`
  - **Then**: `status == WorkflowStatusValue::Running`

#### WorkflowStatus
- **test_workflow_status_serialization_roundtrip**
  - **Given**: `WorkflowStatus` with all fields populated
  - **When**: Serialize then deserialize
  - **Then**: All fields match original values

- **test_workflow_status_running_has_current_step_zero**
  - **Given**: `WorkflowStatus` with `status: WorkflowStatusValue::Running`
  - **When**: Construct via constructor for running workflow
  - **Then**: `current_step == 0`

- **test_workflow_status_completed_has_final_step**
  - **Given**: `WorkflowStatus` with `status: WorkflowStatusValue::Completed`
  - **When**: Construct
  - **Then**: `current_step` reflects last executed step

#### SignalRequest
- **test_signal_request_deserializes_with_payload**
  - **Given**: Valid JSON `{ "signal_name": "payment_approved", "payload": { "approved": true } }`
  - **When**: Deserialize to `SignalRequest`
  - **Then**: `signal_name.as_str() == "payment_approved"`, `payload` correctly extracted

#### SignalResponse
- **test_signal_response_acknowledged_true**
  - **Given**: `SignalResponse { acknowledged: true }`
  - **When**: Serialize
  - **Then**: JSON contains `{ "acknowledged": true }`

#### JournalResponse
- **test_journal_response_with_run_entry**
  - **Given**: `JournalResponse` with `Run` entry (name, input, output, timestamp)
  - **When**: Serialize
  - **Then**: Valid JSON with `type="Run"` and all fields

- **test_journal_response_with_wait_entry**
  - **Given**: `JournalResponse` with `Wait` entry (duration_ms, fire_at, status)
  - **When**: Serialize
  - **Then**: Valid JSON with `type="Wait"` and wait-specific fields

- **test_journal_response_multiple_entries_sorted_by_seq**
  - **Given**: `JournalResponse` with 3 entries (seq 0, 1, 2)
  - **When**: Serialize
  - **Then**: All entries present in ascending seq order

#### ListWorkflowsResponse
- **test_list_workflows_response_empty**
  - **Given**: `ListWorkflowsResponse` with empty workflows vector
  - **When**: Serialize
  - **Then**: JSON contains `{ "workflows": [] }`

- **test_list_workflows_response_multiple_items**
  - **Given**: `ListWorkflowsResponse` with 5 workflow statuses
  - **When**: Serialize
  - **Then**: JSON contains all 5 workflows

#### ErrorResponse
- **test_error_response_without_retry**
  - **Given**: `ErrorResponse` for `not_found` error
  - **When**: Serialize
  - **Then**: JSON without `retry_after_seconds` field

- **test_error_response_with_retry**
  - **Given**: `ErrorResponse` for `at_capacity` error with `RetryAfterSeconds::new(5)`
  - **When**: Serialize
  - **Then**: JSON includes `retry_after_seconds: 5` field

---

### Error Path Tests

#### StartWorkflowRequest Validation
- **test_start_workflow_request_rejects_empty_workflow_name**
  - **Given**: JSON `{ "workflow_name": "", "input": {} }`
  - **When**: Deserialize
  - **Then**: Returns `Err(ParseError::EmptyWorkflowName)`

- **test_start_workflow_request_rejects_invalid_workflow_name_format**
  - **Given**: JSON `{ "workflow_name": "Invalid-Name", "input": {} }`
  - **When**: Deserialize
  - **Then**: Returns `Err(ParseError::InvalidWorkflowNameFormat)`

- **test_start_workflow_request_rejects_workflow_name_with_uppercase**
  - **Given**: JSON `{ "workflow_name": "CheckOut", "input": {} }`
  - **When**: Deserialize
  - **Then**: Returns `Err(ParseError::InvalidWorkflowNameFormat)`

- **test_start_workflow_request_rejects_workflow_name_with_special_chars**
  - **Given**: JSON `{ "workflow_name": "checkout@variant", "input": {} }`
  - **When**: Deserialize
  - **Then**: Returns `Err(ParseError::InvalidWorkflowNameFormat)`

#### InvocationId Validation
- **test_invocation_id_rejects_empty_string**
  - **Given**: `InvocationId::from_str("")`
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::InvalidUlidFormat)`

- **test_invocation_id_rejects_wrong_length_25_chars**
  - **Given**: `InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FA")` (25 chars)
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::InvalidUlidFormat)`

- **test_invocation_id_rejects_wrong_length_27_chars**
  - **Given**: `InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAVX")` (27 chars)
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::InvalidUlidFormat)`

- **test_invocation_id_rejects_invalid_characters**
  - **Given**: `InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5OI!")` (I, O, ! invalid)
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::InvalidUlidFormat)`

- **test_invocation_id_accepts_valid_ulid**
  - **Given**: `InvocationId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")`
  - **When**: Construct
  - **Then**: Returns `Ok(InvocationId)`

#### SignalRequest Validation
- **test_signal_request_rejects_empty_signal_name**
  - **Given**: JSON `{ "signal_name": "", "payload": {} }`
  - **When**: Deserialize
  - **Then**: Returns `Err(ParseError::EmptySignalName)`

- **test_signal_request_rejects_single_char_signal**
  - **Given**: JSON `{ "signal_name": "x", "payload": {} }`
  - **When**: Deserialize
  - **Then**: Returns `Err(ParseError::InvalidSignalNameFormat)` (must be 2+ chars)

- **test_signal_request_rejects_invalid_signal_name_format**
  - **Given**: JSON `{ "signal_name": "Invalid-Name", "payload": {} }`
  - **When**: Deserialize
  - **Then**: Returns `Err(ParseError::InvalidSignalNameFormat)`

#### WorkflowStatusValue Validation
- **test_workflow_status_value_deserializes_lowercase**
  - **Given**: JSON with `status: "running"`
  - **When**: Deserialize to `WorkflowStatusValue`
  - **Then**: Returns `Ok(WorkflowStatusValue::Running)`

- **test_workflow_status_value_rejects_unknown_variant**
  - **Given**: JSON with `status: "unknown"`
  - **When**: Deserialize to `WorkflowStatusValue`
  - **Then**: Returns `Err(ParseError::UnknownStatusVariant)`

#### RetryAfterSeconds Validation
- **test_retry_after_seconds_rejects_zero**
  - **Given**: `RetryAfterSeconds::new(0)`
  - **When**: Construct
  - **Then**: Returns `Err(ValidationError::InvalidRetryAfterSeconds)`

- **test_retry_after_seconds_accepts_positive_value**
  - **Given**: `RetryAfterSeconds::new(5)`
  - **When**: Construct
  - **Then**: Returns `Ok(RetryAfterSeconds)`, `.get() == 5`

#### Timestamp Validation
- **test_timestamp_rejects_invalid_format**
  - **Given**: `Timestamp::new("2024-13-45T99:99:99Z")`
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::InvalidTimestampFormat)`

- **test_timestamp_accepts_rfc3339_z_suffix**
  - **Given**: `Timestamp::new("2024-01-15T10:30:00Z")`
  - **When**: Construct
  - **Then**: Returns `Ok(Timestamp)`

- **test_timestamp_accepts_rfc3339_with_offset**
  - **Given**: `Timestamp::new("2024-01-15T10:30:00+05:00")`
  - **When**: Construct
  - **Then**: Returns `Ok(Timestamp)`

---

### Edge Case Tests

#### StartWorkflowRequest
- **test_start_workflow_request_with_empty_input_object**
  - **Given**: JSON with `input: {}`
  - **When**: Deserialize
  - **Then**: Succeeds with empty object `Value`

- **test_start_workflow_request_with_null_input**
  - **Given**: JSON with `input: null`
  - **When**: Deserialize
  - **Then**: Succeeds with `Value::Null`

- **test_start_workflow_request_with_array_input**
  - **Given**: JSON with `input: [1, 2, 3]`
  - **When**: Deserialize
  - **Then**: Succeeds with array `Value`

- **test_start_workflow_request_with_string_input**
  - **Given**: JSON with `input: "simple string"`
  - **When**: Deserialize
  - **Then**: Succeeds with string `Value`

- **test_start_workflow_request_minimum_valid_name**
  - **Given**: JSON with `workflow_name: "a"`
  - **When**: Deserialize
  - **Then**: Succeeds (single char valid)

- **test_start_workflow_request_name_with_underscores**
  - **Given**: JSON with `workflow_name: "checkout_flow"`
  - **When**: Deserialize
  - **Then**: Succeeds

- **test_start_workflow_request_name_with_numbers**
  - **Given**: JSON with `workflow_name: "order2checkout"`
  - **When**: Deserialize
  - **Then**: Succeeds

- **test_start_workflow_request_long_valid_name**
  - **Given**: JSON with `workflow_name: "a".repeat(64)`
  - **When**: Deserialize
  - **Then**: Succeeds (max reasonable length)

#### WorkflowStatus
- **test_workflow_status_current_step_zero_at_start**
  - **Given**: `WorkflowStatus` for newly started workflow
  - **When**: Serialize
  - **Then**: `current_step == 0`

- **test_workflow_status_large_step_number**
  - **Given**: `WorkflowStatus` with `current_step: 999999`
  - **When**: Serialize
  - **Then**: Serializes correctly as `999999`

- **test_workflow_status_all_status_variants**
  - **Given**: All 5 `WorkflowStatusValue` variants
  - **When**: Serialize each
  - **Then**: Each serializes to lowercase string

#### JournalEntry
- **test_journal_entry_run_without_input**
  - **Given**: `JournalEntry` of type `Run` without `input` field
  - **When**: Serialize
  - **Then**: `input` field skipped (`skip_serializing_if`)

- **test_journal_entry_run_without_output**
  - **Given**: `JournalEntry` of type `Run` without `output` field
  - **When**: Serialize
  - **Then**: `output` field skipped

- **test_journal_entry_wait_with_fire_at**
  - **Given**: `JournalEntry` of type `Wait` with `fire_at` timestamp
  - **When**: Serialize
  - **Then**: `fire_at` field present

- **test_journal_entry_wait_with_status**
  - **Given**: `JournalEntry` of type `Wait` with `status: "waiting"`
  - **When**: Serialize
  - **Then**: `status` field present

---

## Contract Verification Tests

### Precondition Tests

- **test_precondition_workflow_name_pattern_enforced**
  - **Given**: Regex pattern `^[a-z][a-z0-9_]*$`
  - **When**: Valid names `"a"`, `"checkout"`, `"order_2_process"` pass; Invalid names `"Checkout"`, `"1order"`, `"order-name"` fail
  - **Then**: Compile-time enforcement via `WorkflowName::new()`

- **test_precondition_invocation_id_ulid_format_enforced**
  - **Given**: ULID validation
  - **When**: Valid ULID `"01ARZ3NDEKTSV4RRFFQ69G5FAV"` passes; Invalid `"INVALID123"`, wrong length, invalid chars fail
  - **Then**: Compile-time enforcement via `InvocationId::from_str()`

- **test_precondition_signal_name_pattern_enforced**
  - **Given**: Signal name pattern `^[a-z][a-z0-9_]+$`
  - **When**: Valid names `"payment_approved"`, `"cancel"` pass; Invalid `"Invalid"`, `""`, `"a"` (single char) fail
  - **Then**: Compile-time enforcement via `SignalName::new()`

- **test_precondition_status_enum_enforced**
  - **Given**: `WorkflowStatusValue` enum with 5 variants
  - **When**: Deserialize JSON with valid variant; Deserialize with unknown variant
  - **Then**: Valid variant succeeds; Unknown variant returns `ParseError`

- **test_precondition_retry_after_seconds_positive_enforced**
  - **Given**: `RetryAfterSeconds::new()`
  - **When**: Call with `0`; Call with `1`; Call with `u64::MAX`
  - **Then**: `0` returns `Err(ValidationError::InvalidRetryAfterSeconds)`; Positive values succeed

### Postcondition Tests

- **test_postcondition_response_contains_valid_invocation_id**
  - **Given**: `StartWorkflowResponse`
  - **When**: After construction via constructor
  - **Then**: `invocation_id.as_str().len() == 26` and valid Crockford base32

- **test_postcondition_running_status_implies_current_step_present**
  - **Given**: `WorkflowStatus` with `status == Running`
  - **When**: After validation
  - **Then**: `current_step >= 0`

- **test_postcondition_updated_at_after_started_at**
  - **Given**: `WorkflowStatus` with `started_at` and `updated_at`
  - **When**: Call `WorkflowStatus::validate()`
  - **Then**: Returns `Ok(())` when `updated_at >= started_at`

- **test_postcondition_entries_sorted_ascending**
  - **Given**: `JournalResponse` with entries
  - **When**: Call `JournalResponse::validate()`
  - **Then**: Returns `Ok(())` when seq is strictly ascending

- **test_postcondition_non_retryable_errors_have_no_retry_field**
  - **Given**: Error name `"not_found"`
  - **When**: Call `ErrorResponse::new()` with `retry_after: Some(_)`
  - **Then**: Returns `Err(InvariantViolation::InvalidRetryForErrorType)`

### Invariant Tests

- **test_invariant_timestamp_format_valid_rfc3339**
  - **Given**: Any timestamp field
  - **When**: Construct via `Timestamp::new()`
  - **Then**: Returns `Err(ParseError::InvalidTimestampFormat)` for invalid; `Ok(Timestamp)` for valid RFC3339

- **test_invariant_invocation_id_immutable**
  - **Given**: `InvocationId` instance
  - **When**: Attempt to find any `set_*` or `modify` method
  - **Then**: No such methods exist (type provides no mutators)

- **test_invariant_journal_entries_append_only**
  - **Given**: `JournalResponse` with entries
  - **When**: Call `JournalResponse::validate()`
  - **Then**: Rejects entries where seq does not strictly increase

- **test_invariant_all_types_roundtrip_json**
  - **Given**: Instance of each type
  - **When**: Serialize -> Deserialize
  - **Then**: Original == Deserialized for all types

---

## Contract Violation Tests

### P1 Violations
- **test_violates_p1_empty_workflow_name_returns_parse_error**
  - **Given**: `StartWorkflowRequest { workflow_name: "", input: {} }`
  - **When**: Deserialize via `WorkflowName::new("")`
  - **Then**: Returns `Err(ParseError::EmptyWorkflowName)`

- **test_violates_p1_invalid_workflow_name_returns_parse_error**
  - **Given**: `WorkflowName::new("Invalid-Name")`
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::InvalidWorkflowNameFormat)`

### P2 Violations
- **test_violates_p2_short_invocation_id_returns_parse_error**
  - **Given**: `InvocationId::from_str("x")`
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::InvalidUlidFormat)`

### P3 Violations
- **test_violates_p3_empty_signal_name_returns_parse_error**
  - **Given**: `SignalName::new("")`
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::EmptySignalName)`

- **test_violates_p3_invalid_signal_name_format_returns_parse_error**
  - **Given**: `SignalName::new("Invalid-Name")`
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::InvalidSignalNameFormat)`

### P4 Violations
- **test_violates_p4_unknown_status_returns_parse_error**
  - **Given**: JSON `{ "status": "unknown" }`
  - **When**: Deserialize to `WorkflowStatusValue`
  - **Then**: Returns `Err(ParseError::UnknownStatusVariant)`

### P6 Violations
- **test_violates_p6_zero_retry_after_returns_validation_error**
  - **Given**: `RetryAfterSeconds::new(0)`
  - **When**: Construct
  - **Then**: Returns `Err(ValidationError::InvalidRetryAfterSeconds)`

### Q1 Violations
- **test_violates_q1_invalid_invocation_id_caught_at_construction**
  - **Given**: Attempt to construct `StartWorkflowResponse` with invalid invocation_id
  - **When**: Call constructor
  - **Then**: Returns `Err(ParseError::InvalidUlidFormat)` before response can be created

### Q2 Violations
- **test_violates_q2_non_running_status_on_success_returns_invariant_error**
  - **Given**: `StartWorkflowResponse { status: WorkflowStatusValue::Completed, ... }`
  - **When**: Call `StartWorkflowResponse::validate()`
  - **Then**: Returns `Err(InvariantViolation::InvalidStatusForResponse)`

### Q3 Violations
- **test_violates_q3_updated_before_started_returns_invariant_error**
  - **Given**: `WorkflowStatus { started_at: "2024-01-15T10:31:00Z", updated_at: "2024-01-15T10:30:00Z", ... }`
  - **When**: Call `WorkflowStatus::validate()`
  - **Then**: Returns `Err(InvariantViolation::UpdatedBeforeStarted)`

### Q4 Violations
- **test_violates_q4_running_with_nonzero_step_returns_validation_error**
  - **Given**: `WorkflowStatus { status: Running, current_step: 5, ... }`
  - **When**: Construct or validate
  - **Then**: Returns `Err(ValidationError::InvalidCurrentStep)` (running workflows must start at step 0)

### Q5 Violations
- **test_violates_q5_entries_not_sorted_returns_invariant_error**
  - **Given**: `JournalResponse { entries: [seq=1, seq=0] }`
  - **When**: Call `JournalResponse::validate()`
  - **Then**: Returns `Err(InvariantViolation::EntriesNotSorted)`

### Q6 Violations
- **test_violates_q6_signal_response_false_on_success_returns_invariant_error**
  - **Given**: `SignalResponse { acknowledged: false }` for successful signal
  - **When**: Construct via success path
  - **Then**: Constructor panics or returns error (success path always sets true)

### Q7 Violations
- **test_violates_q7_retry_for_non_retryable_error_returns_invariant_error**
  - **Given**: `ErrorResponse::new("not_found", "...", Some(RetryAfterSeconds::new(5)?))`
  - **When**: Construct
  - **Then**: Returns `Err(InvariantViolation::InvalidRetryForErrorType)`

### I1 Violations
- **test_violates_i1_invalid_timestamp_format_returns_parse_error**
  - **Given**: `Timestamp::new("not-a-timestamp")`
  - **When**: Construct
  - **Then**: Returns `Err(ParseError::InvalidTimestampFormat)`

### I2 Violations
- **test_violates_i2_invocation_id_immutability_proven_by_design**
  - **Given**: `InvocationId` type definition
  - **When**: Inspect for mutator methods
  - **Then**: No `set_*`, `modify`, `as_mut`, or similar methods exist (compile-time proof)

### I3 Violations
- **test_violates_i3_duplicate_seq_returns_invariant_error**
  - **Given**: `JournalResponse { entries: [seq=1, seq=1] }` (duplicate, not strictly increasing)
  - **When**: Call `JournalResponse::validate()`
  - **Then**: Returns `Err(InvariantViolation::EntriesNotSorted)`

### I4 Violations
- **test_violates_i4_all_types_fail_json_roundtrip_returns_error**
  - **Given**: Instance of each response type
  - **When**: Serialize -> Deserialize via `serde_json`
  - **Then**: All types roundtrip successfully (violation would be if any failed)

---

## Integration Tests

### End-to-End HTTP Scenarios

#### Scenario 1: Start Workflow End-to-End
- **test_integration_start_workflow_returns_201_with_valid_response**
  - **Given**: A valid workflow definition "checkout" exists in the system
  - **And**: API server is running on configured port
  - **And**: System is not at capacity
  - **When**: Client sends `POST /api/v1/workflows` with body:
    ```json
    { "workflow_name": "checkout", "input": { "order_id": "ord_123" } }
    ```
  - **Then**: Response status is `201 Created`
  - **And**: Response body contains `invocation_id` (26-char ULID)
  - **And**: Response body.`workflow_name` == `"checkout"`
  - **And**: Response body.`status` == `"running"`
  - **And**: Response body.`started_at` is valid RFC3339 timestamp
  - **And**: `invocation_id` passes `InvocationId::from_str()` validation

#### Scenario 2: Get Non-Existent Workflow
- **test_integration_get_nonexistent_workflow_returns_404**
  - **Given**: No workflow with invocation_id `"01ARZ3NDEKTSV4RRFFQ69G5FAV"` exists
  - **When**: Client sends `GET /api/v1/workflows/01ARZ3NDEKTSV4RRFFQ69G5FAV`
  - **Then**: Response status is `404 Not Found`
  - **And**: Response body.`error` == `"not_found"`
  - **And**: Response body.`retry_after_seconds` is `null` or absent

#### Scenario 3: System at Capacity
- **test_integration_at_capacity_returns_409_with_retry_after**
  - **Given**: System is running at maximum workflow capacity
  - **When**: Client sends `POST /api/v1/workflows`
  - **Then**: Response status is `409 Conflict`
  - **And**: Response body.`error` == `"at_capacity"`
  - **And**: Response body.`retry_after_seconds` == `5`

#### Scenario 4: Send Signal to Running Workflow
- **test_integration_send_signal_returns_200_acknowledged**
  - **Given**: Workflow invocation_id `"01ARZ3NDEKTSV4RRFFQ69G5FAV"` exists and is in `"running"` state
  - **And**: Workflow accepts signal `"payment_approved"`
  - **When**: Client sends `POST /api/v1/workflows/01ARZ3NDEKTSV4RRFFQ69G5FAV/signals` with body:
    ```json
    { "signal_name": "payment_approved", "payload": { "approved": true } }
    ```
  - **Then**: Response status is `200 OK`
  - **And**: Response body.`acknowledged` == `true`

#### Scenario 5: Get Journal Entries
- **test_integration_get_journal_returns_sorted_entries**
  - **Given**: Workflow invocation_id `"01ARZ3NDEKTSV4RRFFQ69G5FAV"` has 3 journal entries
  - **When**: Client sends `GET /api/v1/workflows/01ARZ3NDEKTSV4RRFFQ69G5FAV/journal`
  - **Then**: Response status is `200 OK`
  - **And**: Response body.`invocation_id` == `"01ARZ3NDEKTSV4RRFFQ69G5FAV"`
  - **And**: Response body.`entries` has 3 items
  - **And**: Entries are sorted by `seq` ascending (0, 1, 2)
  - **And**: Each entry has appropriate fields for its `type`

#### Scenario 6: List Workflows
- **test_integration_list_workflows_returns_all_running**
  - **Given**: System has 2 running workflows: "checkout" and "order_process"
  - **When**: Client sends `GET /api/v1/workflows`
  - **Then**: Response status is `200 OK`
  - **And**: Response body.`workflows` has 2 items
  - **And**: Each workflow has `invocation_id`, `workflow_name`, `status`, `current_step`, `started_at`, `updated_at`
  - **And**: All `invocation_id` values are valid 26-char ULIDs

---

## Traceability Matrix

| Contract Clause | Test(s) | Category |
|---|---|---|
| P1: workflow_name pattern | test_start_workflow_request_rejects_*, test_violates_p1_*, test_precondition_workflow_name_pattern_enforced | Unit, Violation |
| P2: invocation_id ULID | test_invocation_id_*, test_violates_p2_*, test_precondition_invocation_id_ulid_format_enforced | Unit, Violation |
| P3: signal_name non-empty | test_signal_request_rejects_*, test_violates_p3_*, test_precondition_signal_name_pattern_enforced | Unit, Violation |
| P4: status enum values | test_workflow_status_value_*, test_violates_p4_* | Unit, Violation |
| P5: current_step >= 0 | test_workflow_status_*, compile-time u32 | Unit |
| P6: retry_after > 0 | test_retry_after_seconds_*, test_violates_p6_*, test_precondition_retry_after_seconds_positive_enforced | Unit, Violation |
| Q1: valid invocation_id | test_violates_q1_*, test_postcondition_response_contains_valid_invocation_id | Violation, Contract |
| Q2: status "running" | test_violates_q2_*, test_start_workflow_response_status_is_running_on_success | Violation, Unit |
| Q3: updated_at >= started_at | test_violates_q3_*, test_postcondition_updated_at_after_started_at | Violation, Contract |
| Q4: current_step=0 when running | test_violates_q4_*, test_workflow_status_running_has_current_step_zero | Violation, Unit |
| Q5: entries sorted | test_violates_q5_*, test_violates_i3_*, test_postcondition_entries_sorted_ascending, test_invariant_journal_entries_append_only | Violation, Contract |
| Q6: acknowledged=true on success | test_violates_q6_*, test_integration_send_signal_returns_200_acknowledged | Violation, Integration |
| Q7: retry_after for retryable only | test_violates_q7_*, test_error_response_without_retry, test_postcondition_non_retryable_errors_have_no_retry_field | Violation, Contract |
| I1: RFC3339 timestamps | test_timestamp_*, test_violates_i1_*, test_invariant_timestamp_format_valid_rfc3339 | Unit, Violation, Contract |
| I2: invocation_id immutable | test_violates_i2_*, test_invariant_invocation_id_immutable | Violation, Contract |
| I3: append-only journal | test_journal_response_*, test_invariant_journal_entries_append_only | Unit, Contract |
| I4: valid JSON roundtrip | test_violates_i4_*, test_invariant_all_types_roundtrip_json | Violation, Contract |

---

## Test Execution Order

### Phase 1: Unit Tests (can run in parallel)
```
cargo test --lib types::workflow_name
cargo test --lib types::invocation_id  
cargo test --lib types::signal_name
cargo test --lib types::retry_after_seconds
cargo test --lib types::timestamp
cargo test --lib types::workflow_status
cargo test --lib types::journal_response
cargo test --lib types::error_response
```

### Phase 2: Contract Verification Tests
```
cargo test --lib contract_verification
cargo test --lib precondition
cargo test --lib postcondition
cargo test --lib invariant
```

### Phase 3: Integration Tests
```
cargo test --test api_integration
```

(End of file - total 620 lines)
