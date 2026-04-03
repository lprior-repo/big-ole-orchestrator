#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::StepId;

fuzz_target!(|data: &str| {
    match StepId::parse(data) {
        Ok(_) | Err(_) => {}
    }
});
