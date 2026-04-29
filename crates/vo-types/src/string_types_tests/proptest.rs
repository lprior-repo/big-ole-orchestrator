use crate::*;

#[cfg(feature = "proptest")]
mod proptests {
    use crate::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn workflow_name_round_trip_proptest(s in "([a-zA-Z0-9_][a-zA-Z0-9_-]*[a-zA-Z0-9])|[a-zA-Z0-9]".prop_filter("no consecutive separators", |s| !s.contains("--") && !s.contains("__") && !s.contains("-_") && !s.contains("_-"))) {
            prop_assume!(s.len() <= 128);
            let v = WorkflowName(s);
            prop_assert_eq!(WorkflowName::parse(&v.to_string()), Ok(v));
        }

        #[test]
        fn node_name_round_trip_proptest(s in "[a-zA-Z0-9][a-zA-Z0-9][a-zA-Z0-9]") {
            let v = NodeName(s);
            prop_assert_eq!(NodeName::parse(&v.to_string()), Ok(v));
        }

        #[test]
        fn binary_hash_round_trip_proptest(byte_len in 4u32..128u32) {
            let hex_len = (byte_len * 2) as usize;
            let s: String = "0123456789abcdef".chars().cycle().take(hex_len).collect();
            let v = BinaryHash(s);
            prop_assert_eq!(BinaryHash::parse(&v.to_string()), Ok(v));
        }

        #[test]
        fn timer_id_round_trip_proptest(s in ".{1,256}") {
            let v = TimerId(s);
            prop_assert_eq!(TimerId::parse(&v.to_string()), Ok(v));
        }

        #[test]
        fn idempotency_key_round_trip_proptest(s in ".{1,1024}") {
            let v = IdempotencyKey(s);
            prop_assert_eq!(IdempotencyKey::parse(&v.to_string()), Ok(v));
        }

        #[test]
        fn instance_id_round_trip_proptest(s in "[0-7][0-9A-HJKMNP-TV-Z]{25}") {
            let v = InstanceId(s);
            prop_assert_eq!(InstanceId::parse(&v.to_string()), Ok(v));
        }

        #[test]
        fn serde_round_trip_workflow_name_proptest(s in "([a-zA-Z0-9_][a-zA-Z0-9_-]*[a-zA-Z0-9])|[a-zA-Z0-9]".prop_filter("no consecutive separators", |s| !s.contains("--") && !s.contains("__") && !s.contains("-_") && !s.contains("_-"))) {
            prop_assume!(s.len() <= 128);
            let v = WorkflowName(s);
            let json = serde_json::to_value(&v).expect("serialize");
            let restored: WorkflowName = serde_json::from_value(json).expect("deserialize");
            prop_assert_eq!(restored, v);
        }

        #[test]
        fn serde_round_trip_timer_id_proptest(s in ".{1,256}") {
            let v = TimerId(s);
            let json = serde_json::to_value(&v).expect("serialize");
            let restored: TimerId = serde_json::from_value(json).expect("deserialize");
            prop_assert_eq!(restored, v);
        }

        #[test]
        fn node_name_underscore_prefix_round_trip_proptest(s in "_[a-zA-Z0-9][a-zA-Z0-9]") {
            let v = NodeName(s.clone());
            prop_assert_eq!(NodeName::parse(&v.to_string()), Ok(v));
        }
    }

    #[test]
    fn boundary_consistency_invariant_underscore_prefix() {
        assert_eq!(
            WorkflowName::parse("_valid"),
            Ok(WorkflowName("_valid".to_string()))
        );
    }
}
