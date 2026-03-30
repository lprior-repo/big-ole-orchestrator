import os
import re

def replace_in_file(path, old, new):
    with open(path, 'r') as f:
        content = f.read()
    if old in content:
        content = content.replace(old, new)
        with open(path, 'w') as f:
            f.write(content)

def regex_replace_in_file(path, pattern, new):
    with open(path, 'r') as f:
        content = f.read()
    content, count = re.subn(pattern, new, content)
    if count > 0:
        with open(path, 'w') as f:
            f.write(content)

# 1. crates/vo-api/src/types/tests.rs
path = "crates/vo-api/src/types/tests.rs"
regex_replace_in_file(path, r'fn test_', r'fn ')

# Replace `.is_ok()` and `.is_err()` assertions
regex_replace_in_file(path, r'assert!\(([^.]*)\.is_ok\(\)[^)]*\);', r'assert!(matches!(\1, Ok(_)));')
regex_replace_in_file(path, r'assert!\(([^.]*)\.is_err\(\)[^)]*\);', r'assert!(matches!(\1, Err(_)));')
# some have `.validate().is_ok()` -> `matches!(..., Ok(_))`
regex_replace_in_file(path, r'assert!\(([^.]*\.validate\(\))\.is_ok\(\)\);', r'assert!(matches!(\1, Ok(_)));')
regex_replace_in_file(path, r'assert!\(([^.]*\.validate\(\))\.is_err\(\)\);', r'assert!(matches!(\1, Err(_)));')

# 2. crates/vo-types/src/red_queen_tests.rs
path = "crates/vo-types/src/red_queen_tests.rs"
replace_in_file(path, 'assert!(result.is_err(), "NaN must be rejected");', 'assert!(matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })), "NaN must be rejected");')
replace_in_file(path, 'assert!(result.is_err());', 'assert!(matches!(result, Err(_)));')
replace_in_file(path, 'assert!(result.is_err(),', 'assert!(matches!(result, Err(_)),')
replace_in_file(path, 'let _ = result.unwrap();', 'result.unwrap();')

# 3. crates/vo-types/src/events.rs
path = "crates/vo-types/src/events.rs"
regex_replace_in_file(path, r'assert!\(([^.]*)\.is_ok\(\)\);', r'assert!(matches!(\1, Ok(_)));')
regex_replace_in_file(path, r'assert!\(([^.]*)\.is_ok\(\),[^)]*\);', r'assert!(matches!(\1, Ok(_)));')
regex_replace_in_file(path, r'assert!\(([^.]*)\.is_err\(\)[^)]*\);', r'assert!(matches!(\1, Err(_)));')

# Remove loops in events.rs
events_loops_old = """    #[test]
    fn proptest_envelope_roundtrip_preserves_data() {
        let test_cases = vec![(0u64, 1, "wf-1", 0), (1u64, 100, "wf-abc", 1000)];

        for (version, seq, instance_id, ts) in test_cases {
            let json = serde_json::json!({
                "version": version,
                "instance_id": instance_id,
                "sequence": seq,
                "timestamp_ms": ts,
                "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
                "metadata": {}
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let result = EventEnvelope::from_bytes(&bytes);
            assert!(result.is_ok(), "Failed for version {}", version);
        }
    }"""
events_loops_new = """    use rstest::rstest;
    #[rstest]
    #[case(0u64, 1, "wf-1", 0)]
    #[case(1u64, 100, "wf-abc", 1000)]
    fn proptest_envelope_roundtrip_preserves_data(
        #[case] version: u64,
        #[case] seq: u64,
        #[case] instance_id: &str,
        #[case] ts: u64,
    ) {
        let json = serde_json::json!({
            "version": version,
            "instance_id": instance_id,
            "sequence": seq,
            "timestamp_ms": ts,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        assert!(matches!(result, Ok(_)), "Failed for version {}", version);
    }"""
replace_in_file(path, events_loops_old, events_loops_new)

events_loops_old2 = """    #[test]
    fn proptest_version_support_is_consistent_across_envelope_and_payload() {
        for version in 0..=5u8 {
            let envelope = EventEnvelope {
                version,
                instance_id: "wf-123".to_string(),
                sequence: 1,
                timestamp_ms: 1000,
                payload: serde_json::json!({}),
                metadata: serde_json::json!({}),
            };
            let envelope_supported = envelope.is_supported();
            let payload_supported = EventPayload::is_version_supported(version);
            assert_eq!(
                envelope_supported, payload_supported,
                "Inconsistent for version {}",
                version
            );
        }
    }"""
events_loops_new2 = """    #[rstest]
    #[case(0)]
    #[case(1)]
    #[case(2)]
    #[case(3)]
    #[case(4)]
    #[case(5)]
    fn proptest_version_support_is_consistent_across_envelope_and_payload(#[case] version: u8) {
        let envelope = EventEnvelope {
            version,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        let envelope_supported = envelope.is_supported();
        let payload_supported = EventPayload::is_version_supported(version);
        assert_eq!(
            envelope_supported, payload_supported,
            "Inconsistent for version {}",
            version
        );
    }"""
replace_in_file(path, events_loops_old2, events_loops_new2)

events_loops_old3 = """    #[test]
    fn proptest_sequence_is_always_positive_on_success() {
        let valid_cases = vec![(1u64, "wf-1"), (100u64, "wf-100"), (999999999u64, "wf-max")];
        let invalid_cases = vec![(0u64, "wf-zero")];

        for (seq, instance_id) in valid_cases {
            let json = serde_json::json!({
                "version": 1,
                "instance_id": instance_id,
                "sequence": seq,
                "timestamp_ms": 1000,
                "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
                "metadata": {}
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let result = EventEnvelope::from_bytes(&bytes);
            assert!(result.is_ok(), "Failed for valid sequence: {}", seq);
        }

        for (seq, instance_id) in invalid_cases {
            let json = serde_json::json!({
                "version": 1,
                "instance_id": instance_id,
                "sequence": seq,
                "timestamp_ms": 1000,
                "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
                "metadata": {}
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let result = EventEnvelope::from_bytes(&bytes);
            assert!(result.is_err(), "Should have failed for sequence: {}", seq);
        }
    }"""
events_loops_new3 = """    #[rstest]
    #[case(1u64, "wf-1")]
    #[case(100u64, "wf-100")]
    #[case(999999999u64, "wf-max")]
    fn proptest_sequence_is_always_positive_on_success_valid(#[case] seq: u64, #[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": seq,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        assert!(matches!(result, Ok(_)));
    }

    #[rstest]
    #[case(0u64, "wf-zero")]
    fn proptest_sequence_is_always_positive_on_success_invalid(#[case] seq: u64, #[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": seq,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        assert!(matches!(result, Err(_)));
    }"""
replace_in_file(path, events_loops_old3, events_loops_new3)

events_loops_old4 = """    #[test]
    fn proptest_instance_id_is_always_nonempty_on_success() {
        let valid_cases = vec!["a", "wf-123", "instance_with_underscores"];
        let invalid_cases = vec![""];

        for instance_id in valid_cases {
            let json = serde_json::json!({
                "version": 1,
                "instance_id": instance_id,
                "sequence": 1,
                "timestamp_ms": 1000,
                "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
                "metadata": {}
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let result = EventEnvelope::from_bytes(&bytes);
            assert!(
                result.is_ok(),
                "Failed for valid instance_id: {}",
                instance_id
            );
        }

        for instance_id in invalid_cases {
            let json = serde_json::json!({
                "version": 1,
                "instance_id": instance_id,
                "sequence": 1,
                "timestamp_ms": 1000,
                "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
                "metadata": {}
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let result = EventEnvelope::from_bytes(&bytes);
            assert!(result.is_err(), "Should have failed for empty instance_id");
        }
    }"""
events_loops_new4 = """    #[rstest]
    #[case("a")]
    #[case("wf-123")]
    #[case("instance_with_underscores")]
    fn proptest_instance_id_is_always_nonempty_on_success_valid(#[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        assert!(matches!(result, Ok(_)));
    }

    #[rstest]
    #[case("")]
    fn proptest_instance_id_is_always_nonempty_on_success_invalid(#[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        assert!(matches!(result, Err(_)));
    }"""
replace_in_file(path, events_loops_old4, events_loops_new4)

events_loops_old5 = """    #[test]
    fn proptest_metadata_is_always_object_on_success() {
        let valid_metadata = vec![
            serde_json::json!({}),
            serde_json::json!({"key": "value"}),
            serde_json::json!({"key1": "value1", "key2": 123}),
        ];

        let invalid_metadata = vec![
            serde_json::json!([]),
            serde_json::json!("string"),
            serde_json::Value::Null,
            serde_json::json!(123),
        ];

        for metadata in valid_metadata {
            let json = serde_json::json!({
                "version": 1,
                "instance_id": "wf-123",
                "sequence": 1,
                "timestamp_ms": 1000,
                "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
                "metadata": metadata
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let result = EventEnvelope::from_bytes(&bytes);
            assert!(result.is_ok(), "Failed for valid metadata");
        }

        for metadata in invalid_metadata {
            let json = serde_json::json!({
                "version": 1,
                "instance_id": "wf-123",
                "sequence": 1,
                "timestamp_ms": 1000,
                "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
                "metadata": metadata
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let result = EventEnvelope::from_bytes(&bytes);
            assert!(result.is_err(), "Should have failed for invalid metadata");
        }
    }"""
events_loops_new5 = """    #[rstest]
    #[case(serde_json::json!({}))]
    #[case(serde_json::json!({"key": "value"}))]
    #[case(serde_json::json!({"key1": "value1", "key2": 123}))]
    fn proptest_metadata_is_always_object_on_success_valid(#[case] metadata: serde_json::Value) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": "wf-123",
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": metadata
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        assert!(matches!(result, Ok(_)));
    }

    #[rstest]
    #[case(serde_json::json!([]))]
    #[case(serde_json::json!("string"))]
    #[case(serde_json::Value::Null)]
    #[case(serde_json::json!(123))]
    fn proptest_metadata_is_always_object_on_success_invalid(#[case] metadata: serde_json::Value) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": "wf-123",
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": metadata
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        assert!(matches!(result, Err(_)));
    }"""
replace_in_file(path, events_loops_old5, events_loops_new5)

# 4. crates/vo-types/src/non_empty_vec.rs
path = "crates/vo-types/src/non_empty_vec.rs"
replace_in_file(path, 'let _ = NonEmptyVec::new_unchecked(Vec::<i32>::new());', 'NonEmptyVec::new_unchecked(Vec::<i32>::new());')

# 5. crates/vo-storage/src/codec.rs
path = "crates/vo-storage/src/codec.rs"
replace_in_file(path, 'let _ = decode_event_key(&bytes);', 'decode_event_key(&bytes).unwrap_or((min_id(), SequenceNumber::try_from(1u64).unwrap()));')

# 6. crates/vo-frontend/src/ui/app_io.rs
path = "crates/vo-frontend/src/ui/app_io.rs"
replace_in_file(path, 'let _ = Url::revoke_object_url(&url);', 'Url::revoke_object_url(&url).unwrap_or(());')

# 7. crates/vo-storage/tests/red_queen_tests.rs
path = "crates/vo-storage/tests/red_queen_tests.rs"
regex_replace_in_file(path, r'fn test_adversarial_', r'fn ')

# 8. crates/vo-types/src/workflow/mod.rs
path = "crates/vo-types/src/workflow/mod.rs"
workflow_mod_old = """        // Step 3: RetryPolicy validation per node
        for node in &unvalidated.nodes {
            RetryPolicy::new(
                node.retry_policy.max_attempts,
                node.retry_policy.backoff_ms,
                node.retry_policy.backoff_multiplier,
            )
            .map_err(|reason| WorkflowDefinitionError::InvalidRetryPolicy {
                node_name: node.node_name.clone(),
                reason,
            })?;
        }

        // Step 4: Edge referential integrity
        let node_names: HashSet<&NodeName> =
            unvalidated.nodes.iter().map(|n| &n.node_name).collect();
        for edge in &unvalidated.edges {
            if !node_names.contains(&edge.source_node) {
                return Err(WorkflowDefinitionError::UnknownNode {
                    edge_source: edge.source_node.clone(),
                    unknown_target: edge.source_node.clone(),
                });
            }
            if !node_names.contains(&edge.target_node) {
                return Err(WorkflowDefinitionError::UnknownNode {
                    edge_source: edge.source_node.clone(),
                    unknown_target: edge.target_node.clone(),
                });
            }
        }"""
workflow_mod_new = """        // Step 3: RetryPolicy validation per node
        unvalidated.nodes.iter().try_for_each(|node| {
            RetryPolicy::new(
                node.retry_policy.max_attempts,
                node.retry_policy.backoff_ms,
                node.retry_policy.backoff_multiplier,
            )
            .map_err(|reason| WorkflowDefinitionError::InvalidRetryPolicy {
                node_name: node.node_name.clone(),
                reason,
            })?;
            Ok::<(), WorkflowDefinitionError>(())
        })?;

        // Step 4: Edge referential integrity
        let node_names: HashSet<&NodeName> =
            unvalidated.nodes.iter().map(|n| &n.node_name).collect();
        unvalidated.edges.iter().try_for_each(|edge| {
            if !node_names.contains(&edge.source_node) {
                return Err(WorkflowDefinitionError::UnknownNode {
                    edge_source: edge.source_node.clone(),
                    unknown_target: edge.source_node.clone(),
                });
            }
            if !node_names.contains(&edge.target_node) {
                return Err(WorkflowDefinitionError::UnknownNode {
                    edge_source: edge.source_node.clone(),
                    unknown_target: edge.target_node.clone(),
                });
            }
            Ok(())
        })?;"""
replace_in_file(path, workflow_mod_old, workflow_mod_new)

workflow_mod_old_dfs = """            if let Some(neighbors) = adj.get(current) {
                for neighbor in neighbors.iter() {
                    if let Some(cycle) = dfs_cycle(neighbor, adj, state, path) {
                        return Some(cycle);
                    }
                }
            }"""
workflow_mod_new_dfs = """            if let Some(neighbors) = adj.get(current) {
                if let Some(cycle) = neighbors.iter().find_map(|neighbor| dfs_cycle(neighbor, adj, state, path)) {
                    return Some(cycle);
                }
            }"""
replace_in_file(path, workflow_mod_old_dfs, workflow_mod_new_dfs)

# 9. crates/vo-frontend/src/ui/simulate_mode.rs
path = "crates/vo-frontend/src/ui/simulate_mode.rs"
simulate_old = """    #[test]
    fn invariant_current_op_never_exceeds_ops_length() {
        let mut state = SimProceduralState::new();
        for i in 0..5 {
            let result = state.provide_result(format!("r{i}"), format!("act-{i}"), 5);
            assert!(result.unwrap();
        }
        assert!(!state.can_advance(5));
    }

    #[test]
    fn invariant_checkpoint_map_len_matches_current_op() {
        let mut state = SimProceduralState::new();
        for i in 0..3 {
            state
                .provide_result(format!("r{i}"), format!("act-{i}"), 3)
                .unwrap();
            assert_eq!(state.checkpoint_map.len(), state.current_op as usize);
        }
    }

    #[test]
    fn invariant_event_log_len_matches_current_op() {
        let mut state = SimProceduralState::new();
        for i in 0..3 {
            state
                .provide_result(format!("r{i}"), format!("act-{i}"), 3)
                .unwrap();
            assert_eq!(state.event_log.len(), state.current_op as usize);
        }
    }"""
simulate_new = """    #[test]
    fn invariant_current_op_never_exceeds_ops_length() {
        let mut state = SimProceduralState::new();
        (0..5).for_each(|i| {
            let result = state.provide_result(format!("r{i}"), format!("act-{i}"), 5);
            result.unwrap();
        });
        assert!(!state.can_advance(5));
    }

    #[test]
    fn invariant_checkpoint_map_len_matches_current_op() {
        let mut state = SimProceduralState::new();
        (0..3).for_each(|i| {
            state
                .provide_result(format!("r{i}"), format!("act-{i}"), 3)
                .unwrap();
            assert_eq!(state.checkpoint_map.len(), state.current_op as usize);
        });
    }

    #[test]
    fn invariant_event_log_len_matches_current_op() {
        let mut state = SimProceduralState::new();
        (0..3).for_each(|i| {
            state
                .provide_result(format!("r{i}"), format!("act-{i}"), 3)
                .unwrap();
            assert_eq!(state.event_log.len(), state.current_op as usize);
        });
    }"""
replace_in_file(path, simulate_old, simulate_new)

print("Done")
