#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(data) {
        match vo_types::extract_schema_version(&payload, Some(0)) {
            Ok(_) | Err(_) => {}
        }
        match vo_types::extract_schema_version(&payload, None) {
            Ok(_) | Err(_) => {}
        }
    }
});
