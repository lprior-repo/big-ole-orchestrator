#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_executor::scheduler::JobId;

fuzz_target!(|data: &str| {
    let _ = JobId::parse(data);
});