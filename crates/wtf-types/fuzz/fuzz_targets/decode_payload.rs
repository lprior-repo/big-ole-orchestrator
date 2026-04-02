#![no_main]
use libfuzzer_sys::fuzz_target;
use serde_json::Value;
use std::num::NonZeroU64;

fuzz_target!(|data: &[u8]| {
    let payload: Value = match serde_json::from_slice(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    let envelope = wtf_types::EventEnvelope {
        version: wtf_types::EventVersion(unsafe { NonZeroU64::new_unchecked(1) }),
        instance_id: wtf_types::InstanceId("01H5JYV4XHGSR2F8KZ9BWNRFMA".to_string()),
        sequence: wtf_types::SequenceNumber(unsafe { NonZeroU64::new_unchecked(1) }),
        timestamp_ms: wtf_types::TimestampMs(0),
        payload,
        metadata: None,
    };
    let _ = envelope.decode_payload();
});
