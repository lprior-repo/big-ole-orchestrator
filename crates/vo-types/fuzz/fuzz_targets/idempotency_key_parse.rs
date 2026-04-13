#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::IdempotencyKey;

/// Fuzz target for IdempotencyKey parsing.
///
/// Input type: &str (arbitrary UTF-8 strings)
/// Risk class: Panic/InvalidFormat/ExceedsMaxLength
/// Tests idempotency key validation
///
/// Corpus seeds:
/// - Empty string
/// - Single char
/// - Exactly 64 chars (max)
/// - 65 chars (exceeds max)
/// - Consecutive hyphens
/// - Invalid characters
/// - Unicode
fuzz_target!(|data: &str| {
    let _result = IdempotencyKey::parse(data);
});
