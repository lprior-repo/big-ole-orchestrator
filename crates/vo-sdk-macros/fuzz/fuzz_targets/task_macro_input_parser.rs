#![no_main]
use libfuzzer_sys::fuzz_target;
use proc_macro2::TokenStream;
use vo_sdk_macros::task::TaskOpts;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(item) = s.parse::<TokenStream>() {
            let attr = proc_macro2::TokenStream::new();
            // Call the parsing function inside vo_sdk_macros. We just need to make sure it doesn't panic.
            let _ = vo_sdk_macros::task::parse_task(&item, TaskOpts::default());
        }
    }
});
