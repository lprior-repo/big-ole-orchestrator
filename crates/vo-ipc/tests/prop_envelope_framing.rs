//! Property test: envelope framing consistency.
//!
//! bead_id: ve-e2nsq

#![allow(clippy::unwrap_used)]

use std::io::Cursor;

use vo_ipc::{read_envelope, write_envelope, Fd3Envelope, Fd4Envelope, TaskResult};

proptest::proptest! {
    #[test]
    fn fd3_envelope_roundtrip_via_cursor(
        instance_id in "[a-zA-Z0-9]{1,64}",
        node_id in "[a-zA-Z0-9]{1,64}",
        input in proptest::collection::vec(proptest::arbitrary::any::<u8>(), 0..1000),
    ) {
        let envelope = Fd3Envelope {
            version: 1,
            instance_id: instance_id.clone(),
            node_id: node_id.clone(),
            input: serde_json::Value::String(String::from_utf8_lossy(&input).into_owned()),
            secrets: std::collections::BTreeMap::new(),
            metadata: std::collections::BTreeMap::new(),
        };

        let mut buf = Cursor::new(Vec::new());
        write_envelope(&mut buf, &envelope).unwrap();

        buf.set_position(0);
        let recovered: Fd3Envelope = read_envelope(&mut buf).unwrap();

        assert_eq!(recovered, envelope);
    }
}

proptest::proptest! {
    #[test]
    fn fd4_success_envelope_roundtrip(
        instance_id in "[a-zA-Z0-9]{1,64}",
        node_id in "[a-zA-Z0-9]{1,64}",
        output in proptest::collection::vec(proptest::arbitrary::any::<u8>(), 0..1000),
    ) {
        let envelope = Fd4Envelope {
            version: 1,
            instance_id,
            node_id,
            result: TaskResult::Success {
                output: serde_json::Value::String(String::from_utf8_lossy(&output).into_owned()),
            },
        };

        let mut buf = Cursor::new(Vec::new());
        write_envelope(&mut buf, &envelope).unwrap();

        buf.set_position(0);
        let recovered: Fd4Envelope = read_envelope(&mut buf).unwrap();

        assert_eq!(recovered, envelope);
    }
}
