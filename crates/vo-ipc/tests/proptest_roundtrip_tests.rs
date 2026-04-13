use proptest::prelude::*;
use std::io::Cursor;
use vo_ipc::*;

fn arb_json_value() -> BoxedStrategy<serde_json::Value> {
    let leaf = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::Bool),
        any::<i64>().prop_map(|v| serde_json::json!(v)),
        "[a-zA-Z0-9_]{0,20}".prop_map(serde_json::Value::String),
        prop_oneof![
            any::<u64>().prop_map(|v| serde_json::json!(v)),
            any::<i64>().prop_map(|v| serde_json::json!(v)),
        ],
    ];
    leaf.prop_recursive(3, 64, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::Array),
            prop::collection::btree_map("[a-z]{1,4}", inner, 0..4)
                .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
        ]
    })
    .boxed()
}

fn arb_fd3_envelope() -> BoxedStrategy<Fd3Envelope> {
    (
        Just(1u8),
        "[a-zA-Z0-9]{1,12}",
        "[a-zA-Z0-9]{1,12}",
        arb_json_value(),
        prop::collection::btree_map("[a-zA-Z_]{1,8}", "[a-zA-Z0-9]{0,16}", 0..3),
        prop::collection::btree_map("[a-z_]{1,8}", "[a-zA-Z0-9]{0,16}", 0..3),
    )
        .prop_map(
            |(version, instance_id, node_id, input, secrets, metadata)| Fd3Envelope {
                version,
                instance_id,
                node_id,
                input,
                secrets,
                metadata,
            },
        )
        .boxed()
}

fn arb_task_result() -> BoxedStrategy<TaskResult> {
    prop_oneof![
        arb_json_value().prop_map(|output| TaskResult::Success { output }),
        ("[A-Z_]{2,12}", "[a-zA-Z0-9 ]{0,40}").prop_map(|(code, message)| TaskResult::Failure {
            error: TaskError {
                code,
                message,
                details: None,
            }
        }),
    ]
    .boxed()
}

fn arb_fd4_envelope() -> BoxedStrategy<Fd4Envelope> {
    (
        Just(1u8),
        "[a-zA-Z0-9]{1,12}",
        "[a-zA-Z0-9]{1,12}",
        arb_task_result(),
    )
        .prop_map(|(version, instance_id, node_id, result)| Fd4Envelope {
            version,
            instance_id,
            node_id,
            result,
        })
        .boxed()
}

proptest! {
    #[test]
    fn fd3_envelope_roundtrip_preserves_all_fields(env in arb_fd3_envelope()) {
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
        prop_assert_eq!(env, decoded);
    }

    #[test]
    fn fd4_envelope_roundtrip_preserves_all_fields(env in arb_fd4_envelope()) {
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
        prop_assert_eq!(env, decoded);
    }

    #[test]
    fn write_read_is_idempotent_fd3(env in arb_fd3_envelope()) {
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let d1: Fd3Envelope = read_envelope(&mut Cursor::new(buf.clone())).unwrap();
        let d2: Fd3Envelope = read_envelope(&mut Cursor::new(buf.clone())).unwrap();
        prop_assert_eq!(d1, d2);
    }

    #[test]
    fn write_read_is_idempotent_fd4(env in arb_fd4_envelope()) {
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let d1: Fd4Envelope = read_envelope(&mut Cursor::new(buf.clone())).unwrap();
        let d2: Fd4Envelope = read_envelope(&mut Cursor::new(buf.clone())).unwrap();
        prop_assert_eq!(d1, d2);
    }

    #[test]
    fn serialized_size_matches_length_prefix_fd3(env in arb_fd3_envelope()) {
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        prop_assert_eq!(len, buf.len() - 4);
    }

    #[test]
    fn serialized_size_matches_length_prefix_fd4(env in arb_fd4_envelope()) {
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        prop_assert_eq!(len, buf.len() - 4);
    }

    #[test]
    fn multiple_envelopes_in_sequence_decode_correctly(
        env1 in arb_fd3_envelope(),
        env2 in arb_fd3_envelope(),
        env3 in arb_fd3_envelope()
    ) {
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env1).unwrap();
        write_envelope(&mut buf, &env2).unwrap();
        write_envelope(&mut buf, &env3).unwrap();

        let mut cursor = Cursor::new(buf);
        let d1: Fd3Envelope = read_envelope(&mut cursor).unwrap();
        let d2: Fd3Envelope = read_envelope(&mut cursor).unwrap();
        let d3: Fd3Envelope = read_envelope(&mut cursor).unwrap();

        prop_assert_eq!(d1, env1);
        prop_assert_eq!(d2, env2);
        prop_assert_eq!(d3, env3);
    }

    #[test]
    fn validate_identity_succeeds_only_on_exact_match(
        instance_id in "[a-zA-Z0-9]{1,12}",
        node_id in "[a-zA-Z0-9]{1,12}",
        output in arb_json_value()
    ) {
        let env = Fd4Envelope {
            version: 1,
            instance_id: instance_id.clone(),
            node_id: node_id.clone(),
            result: TaskResult::Success { output },
        };
        prop_assert!(validate_identity(&env, &instance_id, &node_id).is_ok());
    }

    #[test]
    fn stderr_update_capture_is_associative_for_small_chunks(
        a in any::<Vec<u8>>().prop_filter("small", |v| v.len() < 100),
        b in any::<Vec<u8>>().prop_filter("small", |v| v.len() < 100),
        c in any::<Vec<u8>>().prop_filter("small", |v| v.len() < 100),
    ) {
        use vo_ipc::stderr::{update_capture, StderrCapture};

        let ab_then_c = update_capture(update_capture(update_capture(StderrCapture::empty(), &a), &b), &c);
        let a_then_bc = update_capture(update_capture(update_capture(StderrCapture::empty(), &a), &b), &c);

        prop_assert_eq!(ab_then_c.bytes, a_then_bc.bytes);
        prop_assert_eq!(ab_then_c.observed_bytes, a_then_bc.observed_bytes);
        prop_assert_eq!(ab_then_c.truncated, a_then_bc.truncated);
    }

    #[test]
    fn parse_fd3_payload_as_argv_handles_arbitrary_bytes(payload in any::<Vec<u8>>()) {
        let s = String::from_utf8_lossy(&payload);
        let args: Vec<&str> = s.split_whitespace().collect();
        for arg in &args {
            prop_assert!(!arg.is_empty());
        }
    }

    #[test]
    fn version_zero_envelope_rejected(version in 2u8..255u8) {
        let env_json = serde_json::json!({
            "version": version,
            "instance_id": "inst1",
            "node_id": "node1",
            "input": {},
            "secrets": {},
            "metadata": {}
        });
        let payload = serde_json::to_vec(&env_json).unwrap();
        let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
        buf.extend(payload);
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        prop_assert!(matches!(result, Err(IpcError::VersionMismatch(v)) if v == version));
    }

    #[test]
    fn non_alphanumeric_instance_id_rejected(
        id in "[a-zA-Z0-9]{2,6}[!@#$%^&*()][a-zA-Z0-9]{0,6}"
    ) {
        let env_json = serde_json::json!({
            "version": 1,
            "instance_id": id,
            "node_id": "node1",
            "input": {},
            "secrets": {},
            "metadata": {}
        });
        let payload = serde_json::to_vec(&env_json).unwrap();
        let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
        buf.extend(payload);
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        prop_assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
    }
}
