#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_storage::snapshots::decode_snapshot_key;

/// Fuzz target for SnapshotKey decoding.
///
/// Input type: &[u8] (raw bytes)
/// Risk class: Panic/CorruptKey/InvalidLength
/// Tests snapshot key binary decoding
///
/// Corpus seeds:
/// - Empty bytes
/// - Too short (<16 bytes)
/// - Exactly 16 bytes
/// - Too long (>16 bytes)
/// - All zeros
/// - All 0xFF
/// - Random bytes
fuzz_target!(|data: &[u8]| {
    let _result = decode_snapshot_key(data);
});
