#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut reader = data;
    let mut is_read = false;
    let _ignored = vo_sdk::io::read_input_inner_with_state(&mut reader, &mut is_read);
});
