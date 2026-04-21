#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(spec) = serde_json::from_slice::<vo_sdk::WorkflowSpec>(data) {
        let _ = spec.validate();
    }
});
