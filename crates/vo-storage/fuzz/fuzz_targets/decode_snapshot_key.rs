#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Must not panic — any input is valid to attempt parsing
    drop(vo_storage::snapshots::decode_snapshot_key(data));
});
