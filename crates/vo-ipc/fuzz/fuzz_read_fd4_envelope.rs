#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;
use vo_ipc::read_envelope;
use vo_ipc::Fd4Envelope;

fuzz_target!(|data: &[u8]| {
    let _ = read_envelope::<Fd4Envelope>(&mut Cursor::new(data));
});
