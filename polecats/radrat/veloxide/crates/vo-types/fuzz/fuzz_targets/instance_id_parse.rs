#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::InstanceId;

/// Fuzz target for InstanceId parsing.
///
/// Input type: &str (arbitrary UTF-8 strings)
/// Risk class: Panic/OOM/InvalidFormat
/// Tests ULID validation, length checks, character validation
///
/// Corpus seeds:
/// - Empty string
/// - Wrong length (1, 10, 25, 27, 100 chars)
/// - All zeros (nil ULID)
/// - All Z's (max ULID)
/// - Invalid characters (g, z except last char, special chars)
/// - Random bytes as UTF-8
/// - Unicode edge cases
/// - Long strings (1KB, 1MB)
fuzz_target!(|data: &str| {
    let _result = InstanceId::parse(data);
});
