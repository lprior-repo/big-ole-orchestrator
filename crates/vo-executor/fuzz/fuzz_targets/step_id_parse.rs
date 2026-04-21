#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_executor::types::StepId;

fuzz_target!(|data: &str| {
    let _ = StepId::parse(data);
});