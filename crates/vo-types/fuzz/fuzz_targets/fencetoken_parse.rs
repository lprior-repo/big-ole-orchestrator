#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::FenceToken;

fuzz_target!(|data: &str| {
    match FenceToken::parse(data) {
        Ok(_) | Err(_) => {}
    }
});
