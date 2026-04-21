#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_executor::scheduler::Schedule;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<Schedule>(s);
    }
});