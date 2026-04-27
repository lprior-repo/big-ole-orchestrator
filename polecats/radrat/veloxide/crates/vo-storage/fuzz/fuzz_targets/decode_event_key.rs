#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_storage::codec::decode_event_key;

/// Fuzz target for EventKey decoding.
///
/// Input type: &[u8] (raw bytes)
/// Risk class: Panic/CorruptKey/InvalidLength
/// Tests event key binary decoding (InstanceId + SequenceNumber)
///
/// Corpus seeds:
/// - Empty bytes
/// - Too short (<24 bytes)
/// - Too long (>24 bytes)
/// - Exactly 24 bytes
/// - All zeros
/// - All 0xFF
/// - Random bytes
fuzz_target!(|data: &[u8]| {
    let _result = decode_event_key(data);
});
