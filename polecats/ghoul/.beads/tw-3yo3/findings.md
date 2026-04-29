# Findings: tw-3yo3 - WorkflowEvent Deserialization Unknown Variant Rejection

## Issue
Bead description stated that `WorkflowEvent` enum in `crates/vo-types/src/events/mod.rs` uses serde `tag=type` for deserialization and should reject unknown variants with informative error messages.

## Investigation

### File Location Discrepancy
The bead description mentioned `crates/vo-types/src/events/mod.rs` but:
- That file does not exist in the codebase
- `vo-types` has `events/` module that exports `EventEnvelope` and `EventPayload`
- The actual `WorkflowEvent` enum is in `crates/vo-common/src/events.rs`

### Original Implementation
The original `WorkflowEvent` in `vo-common`:
- Used `#[derive(Deserialize)]` without any serde tag attributes
- Serialization format was externally tagged: `{"TimerFired":{"event_id":"...","timer_id":"...","timestamp_ms":...}}`
- Unknown variants were rejected by serde (test existed), but error message didn't include variant name
- Extra fields were silently ignored (security issue)
- Duplicate fields were silently accepted (second value overwrote first)

## Fix Applied

### Changed Serialization Format
Changed from externally tagged to internally tagged format:
- Old: `{"TimerFired":{"event_id":"...","timer_id":"...","timestamp_ms":...}}`
- New: `{"type":"TimerFired","event_id":"...","timer_id":"...","timestamp_ms":...}`

### Custom Deserializer Implementation
Implemented custom `Deserialize` and `Serialize` for `WorkflowEvent`:
- Internal tagged format using `type` field
- Tracks seen fields to reject duplicates
- Provides informative error messages with variant name for unknown variants
- Rejects unknown fields (not silently ignored)

### Files Modified
1. `crates/vo-common/src/events.rs` - Main implementation
2. `crates/vo-common/tests/blackhat_common_types.rs` - Updated tests
3. `crates/vo-common/tests/blackhat_event_ordering.rs` - Updated tests
4. `crates/vo-common/tests/qa_common_types.rs` - Updated tests
5. `crates/vo-common/tests/redqueen_common.rs` - Updated tests

### Security Improvements
1. **Unknown variants rejected**: Error message includes variant name
2. **Extra fields rejected**: Previously silently ignored, now returns error
3. **Duplicate fields rejected**: Previously accepted (second overwrote first), now returns error with "duplicate" in message

### New Tests Added
- `workflow_event_internal_tag_format` - Verifies internally tagged format
- `workflow_event_rejects_unknown_variant_with_error_message` - Verifies error contains variant name
- `workflow_event_rejects_unknown_variant_short_form` - Tests minimal unknown variant JSON

## Breaking Change
This is a **breaking change** to the serialization format. Any code that serializes/serializes `WorkflowEvent` must be updated to use the new internally tagged format.

## Test Results
All 314 vo-common tests pass.

## Note
The bead description mentioned `#[serde(deny_unknown_fields)]` but this attribute only works for structs, not enums. The fix uses custom deserialization logic to achieve the same security goals.
