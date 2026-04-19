#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::events::envelope::EventEnvelope;

/// Fuzz target for EventEnvelope JSON deserialization.
///
/// Input type: &[u8] (raw bytes interpreted as UTF-8 JSON)
/// Risk class: Panic/InvalidUtf8/MalformedJson/MissingFields
/// Tests event envelope parsing from JSON
///
/// Corpus seeds:
/// - Empty bytes
/// - Invalid UTF-8 sequences
/// - Truncated JSON
/// - Missing required fields (instance_id, sequence, timestamp_ms, payload)
/// - Wrong field types (string instead of number)
/// - Null values
/// - Very deep nesting
/// - Very long strings
fuzz_target!(|data: &[u8]| {
    let _result = EventEnvelope::from_bytes(data);
});
