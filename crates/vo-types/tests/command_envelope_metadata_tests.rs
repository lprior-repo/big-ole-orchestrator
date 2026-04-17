//! Integration tests for command envelope metadata (ADR-036).
//!
//! Tests identity fields on all mutating surfaces, metadata propagation through
//! the execution pipeline, command dedup using identity, identity validation,
//! authority checks, and cross-rig command routing.

use vo_types::*;

#[test]
fn command_metadata_has_all_five_identity_fields() {
    let meta = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-001").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-001").unwrap(),
        causation_id: IdempotencyKey::parse("cause-001").unwrap(),
        issuer: Issuer::System,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };
    assert!(!meta.command_id.as_str().is_empty());
    assert!(!meta.correlation_id.as_str().is_empty());
    assert!(!meta.causation_id.as_str().is_empty());
    assert_ne!(meta.issued_at.as_u64(), 0);
}

#[test]
fn command_envelope_exposes_metadata_on_mutating_surface() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-mutate-001",
        "correlation_id": "corr-mutate-001",
        "causation_id": "cause-mutate-001",
        "issuer": "operator",
        "issued_at": 1700000000
    }"#;
    let envelope = CommandEnvelope::from_str(json).unwrap();
    assert_eq!(envelope.metadata.command_id.as_str(), "cmd-mutate-001");
    assert_eq!(envelope.metadata.correlation_id.as_str(), "corr-mutate-001");
    assert_eq!(envelope.metadata.causation_id.as_str(), "cause-mutate-001");
    assert_eq!(envelope.metadata.issuer, Issuer::Operator);
    assert_eq!(envelope.metadata.issued_at.as_u64(), 1_700_000_000);
}

#[test]
fn history_entry_carries_envelope_identity() {
<<<<<<< HEAD
    use vo_types::command_history::{
        CommandHistory, CommandKind, HistoryEntryStatus, WorkflowSnapshot,
    };
=======
    use vo_types::command_history::{CommandHistory, CommandKind, HistoryEntryStatus, WorkflowSnapshot};
>>>>>>> origin/polecat/synth-mnw6kj8v
    use vo_types::{DagNode, NodeName, RetryPolicy};

    let snapshot = WorkflowSnapshot::new(
        "test-wf".into(),
        vec![DagNode {
            node_name: NodeName::parse("n1").unwrap(),
            retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
<<<<<<< HEAD
            compensation_policy: None,
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
        }],
        vec![],
    );

    let mut history = CommandHistory::new();
    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, snapshot)
        .unwrap();

    let entry = history
        .entries()
        .iter()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
        .unwrap();

    assert_eq!(entry.envelope.schema_version, 1);
    assert!(!entry.envelope.metadata.command_id.as_str().is_empty());
    assert!(!entry.envelope.metadata.correlation_id.as_str().is_empty());
    assert!(!entry.envelope.metadata.causation_id.as_str().is_empty());
    assert_eq!(entry.envelope.metadata.issuer, Issuer::Operator);
    assert!(entry.envelope.metadata.issued_at.as_u64() > 0);
    assert_eq!(entry.status, HistoryEntryStatus::Committed);
}

#[test]
fn all_issuer_variants_are_valid_on_command_envelope() {
    let issuers = [
        ("system", Issuer::System),
        ("api_client", Issuer::ApiClient),
        ("operator", Issuer::Operator),
        ("ai_agent", Issuer::AiAgent),
        ("timer_loop", Issuer::TimerLoop),
        ("recovery_loop", Issuer::RecoveryLoop),
    ];
    for (issuer_str, expected) in issuers {
        let json = format!(
            r#"{{
                "version": 1,
                "command_id": "cmd-{issuer_str}",
                "correlation_id": "corr-{issuer_str}",
                "causation_id": "cause-{issuer_str}",
                "issuer": "{issuer_str}",
                "issued_at": 1700000000
            }}"#
        );
        let envelope = CommandEnvelope::from_str(&json).unwrap();
<<<<<<< HEAD
        assert_eq!(
            envelope.metadata.issuer, expected,
            "issuer '{issuer_str}' should map to {expected:?}"
        );
=======
        assert_eq!(envelope.metadata.issuer, expected, "issuer '{issuer_str}' should map to {expected:?}");
>>>>>>> origin/polecat/synth-mnw6kj8v
    }
}

#[test]
fn command_metadata_survives_json_roundtrip_through_envelope() {
    let original = CommandMetadata {
        command_id: IdempotencyKey::parse("pipeline-cmd-001").unwrap(),
        correlation_id: IdempotencyKey::parse("pipeline-corr-001").unwrap(),
        causation_id: IdempotencyKey::parse("pipeline-cause-001").unwrap(),
        issuer: Issuer::AiAgent,
        issued_at: TimestampMs::try_from(1_730_000_000u64).unwrap(),
    };
    let envelope = CommandEnvelope {
        schema_version: 1,
        metadata: original.clone(),
    };
    let json = serde_json::to_string(&envelope).unwrap();
    let recovered: CommandEnvelope = CommandEnvelope::from_str(&json).unwrap();
    assert_eq!(recovered.metadata.command_id, original.command_id);
    assert_eq!(recovered.metadata.correlation_id, original.correlation_id);
    assert_eq!(recovered.metadata.causation_id, original.causation_id);
    assert_eq!(recovered.metadata.issuer, original.issuer);
    assert_eq!(recovered.metadata.issued_at, original.issued_at);
}

#[test]
fn command_metadata_propagates_through_command_history_entries() {
    use vo_types::command_history::{CommandHistory, CommandKind, WorkflowSnapshot};
    use vo_types::{DagNode, NodeName, RetryPolicy};

    let snapshot = WorkflowSnapshot::new(
        "wf".into(),
        vec![DagNode {
            node_name: NodeName::parse("n1").unwrap(),
            retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
<<<<<<< HEAD
            compensation_policy: None,
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
        }],
        vec![],
    );
    let mut history = CommandHistory::new();
    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, snapshot)
        .unwrap();

    let entry = history
        .entries()
        .iter()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
        .unwrap();
    assert_eq!(
        entry.envelope.metadata.command_id.as_str(),
        cmd_id.as_str(),
        "command_id must propagate from CommandId to envelope metadata"
    );
<<<<<<< HEAD
    assert_eq!(
        entry.envelope.metadata.issuer,
        Issuer::Operator,
        "issuer must propagate to history entry"
    );
=======
    assert_eq!(entry.envelope.metadata.issuer, Issuer::Operator, "issuer must propagate to history entry");
>>>>>>> origin/polecat/synth-mnw6kj8v
}

#[test]
fn command_metadata_correlation_and_causation_are_independent() {
    let meta = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-indep").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-business-request-xyz").unwrap(),
        causation_id: IdempotencyKey::parse("cause-parent-event-abc").unwrap(),
        issuer: Issuer::ApiClient,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };
    assert_ne!(meta.correlation_id, meta.causation_id);
    assert_ne!(meta.command_id, meta.correlation_id);
    assert_ne!(meta.command_id, meta.causation_id);
    let json = serde_json::to_value(&meta).unwrap();
    assert_eq!(json["command_id"], "cmd-indep");
    assert_eq!(json["correlation_id"], "corr-business-request-xyz");
    assert_eq!(json["causation_id"], "cause-parent-event-abc");
}

#[test]
fn apply_command_preserves_metadata_through_undo_redo_cycle() {
    use vo_types::command_history::{CommandHistory, CommandKind, WorkflowSnapshot};
    use vo_types::{DagNode, NodeName, RetryPolicy};

    let snapshot = WorkflowSnapshot::new(
        "wf".into(),
        vec![DagNode {
            node_name: NodeName::parse("n1").unwrap(),
            retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
<<<<<<< HEAD
            compensation_policy: None,
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
        }],
        vec![],
    );
    let mut history = CommandHistory::new();
    let cmd_id = history
<<<<<<< HEAD
        .apply_command(
            CommandKind::EdgeCreate,
            snapshot.clone(),
            snapshot.clone(),
            None,
        )
=======
        .apply_command(CommandKind::EdgeCreate, snapshot.clone(), snapshot.clone(), None)
>>>>>>> origin/polecat/synth-mnw6kj8v
        .unwrap();
    history.undo().unwrap();
    history.redo().unwrap();
    let entry = history
        .entries()
        .iter()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
        .unwrap();
    assert_eq!(entry.envelope.metadata.command_id.as_str(), cmd_id.as_str());
}

#[test]
fn multiple_history_entries_have_distinct_command_ids() {
    use vo_types::command_history::{CommandHistory, CommandKind, WorkflowSnapshot};
    use vo_types::{DagNode, NodeName, RetryPolicy};

    let snapshot = WorkflowSnapshot::new(
        "wf".into(),
        vec![DagNode {
            node_name: NodeName::parse("n1").unwrap(),
            retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
<<<<<<< HEAD
            compensation_policy: None,
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
        }],
        vec![],
    );
    let mut history = CommandHistory::new();
    let mut cmd_ids = std::collections::HashSet::new();
    for kind in [
        CommandKind::NodeCreate,
        CommandKind::NodeDelete,
        CommandKind::EdgeCreate,
        CommandKind::EdgeDelete,
        CommandKind::ConfigUpdate,
    ] {
        let cmd_id = history.save_undo_point(kind, snapshot.clone()).unwrap();
<<<<<<< HEAD
        assert!(
            cmd_ids.insert(cmd_id.as_str().to_string()),
            "each history entry must have a unique command_id"
        );
=======
        assert!(cmd_ids.insert(cmd_id.as_str().to_string()), "each history entry must have a unique command_id");
>>>>>>> origin/polecat/synth-mnw6kj8v
    }
    assert_eq!(cmd_ids.len(), 5);
}

#[test]
fn dedupe_key_derives_from_command_id() {
    let cmd_id = IdempotencyKey::parse("cmd-dedup-001").unwrap();
    let dedupe_key = DedupeKey::parse(cmd_id.as_str()).unwrap();
    assert_eq!(dedupe_key.as_str(), "cmd-dedup-001");
    assert_eq!(dedupe_key.as_str(), cmd_id.as_str());
}

#[test]
fn dedupe_key_detects_duplicate_command_ids() {
    let key1 = DedupeKey::parse("cmd-same-001").unwrap();
    let key2 = DedupeKey::parse("cmd-same-001").unwrap();
    let key3 = DedupeKey::parse("cmd-different-002").unwrap();
    assert_eq!(key1, key2, "identical command_ids produce equal DedupeKeys");
<<<<<<< HEAD
    assert_ne!(
        key1, key3,
        "different command_ids produce different DedupeKeys"
    );
=======
    assert_ne!(key1, key3, "different command_ids produce different DedupeKeys");
>>>>>>> origin/polecat/synth-mnw6kj8v
}

#[test]
fn dedupe_partition_key_combines_instance_and_command_type() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let pk = DedupePartitionKey::new(instance_id.clone(), "workflow_start").unwrap();
    assert_eq!(pk.instance_id(), &instance_id);
    assert_eq!(pk.command_type(), "workflow_start");
}

#[test]
fn dedupe_key_hash_enables_hashset_deduplication() {
    use std::collections::HashSet;
    let mut seen: HashSet<DedupeKey> = HashSet::new();
    let key = DedupeKey::parse("cmd-once-001").unwrap();
    assert!(seen.insert(key.clone()), "first insert should succeed");
    let dup = DedupeKey::parse("cmd-once-001").unwrap();
    assert!(!seen.insert(dup), "duplicate should not be inserted");
    let other = DedupeKey::parse("cmd-once-002").unwrap();
    assert!(seen.insert(other), "different command should be inserted");
    assert_eq!(seen.len(), 2);
}

#[test]
fn unknown_issuer_is_rejected_during_parsing() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-bad",
        "correlation_id": "corr-bad",
        "causation_id": "cause-bad",
        "issuer": "hacker",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(result.is_err(), "unknown issuer must be rejected");
<<<<<<< HEAD
    assert!(matches!(
        result,
        Err(CommandEnvelopeError::InvalidEnvelopeField(_))
    ));
=======
    assert!(matches!(result, Err(CommandEnvelopeError::InvalidEnvelopeField(_))));
>>>>>>> origin/polecat/synth-mnw6kj8v
}

#[test]
fn issuer_admin_is_rejected_as_unauthorized() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-admin",
        "correlation_id": "corr-admin",
        "causation_id": "cause-admin",
        "issuer": "admin",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(result.is_err(), "issuer 'admin' should be rejected");
}

#[test]
fn issuer_enum_has_six_variants_for_authority_levels() {
    let all_issuers = [
        Issuer::System,
        Issuer::ApiClient,
        Issuer::Operator,
        Issuer::AiAgent,
        Issuer::TimerLoop,
        Issuer::RecoveryLoop,
    ];
    assert_eq!(all_issuers.len(), 6, "Issuer must have exactly 6 variants");
}

#[test]
fn command_envelope_command_id_is_stable_for_routing() {
    let cmd_id_str = "cross-rig-cmd-stable-001";
    let meta = CommandMetadata {
        command_id: IdempotencyKey::parse(cmd_id_str).unwrap(),
        correlation_id: IdempotencyKey::parse("corr-cross-001").unwrap(),
        causation_id: IdempotencyKey::parse("cause-cross-001").unwrap(),
        issuer: Issuer::ApiClient,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };
    let json = serde_json::to_string(&meta).unwrap();
    let recovered: CommandMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered.command_id.as_str(), cmd_id_str);
}

#[test]
fn command_envelope_correlation_id_groups_cross_rig_operations() {
    let correlation = "business-request-xyz-001";
    let cmd1 = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-rig-a-001").unwrap(),
        correlation_id: IdempotencyKey::parse(correlation).unwrap(),
        causation_id: IdempotencyKey::parse("cause-parent-001").unwrap(),
        issuer: Issuer::System,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };
    let cmd2 = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-rig-b-002").unwrap(),
        correlation_id: IdempotencyKey::parse(correlation).unwrap(),
        causation_id: IdempotencyKey::parse("cause-rig-a-001").unwrap(),
        issuer: Issuer::System,
        issued_at: TimestampMs::try_from(1_700_000_100u64).unwrap(),
    };
    assert_eq!(cmd1.correlation_id, cmd2.correlation_id);
    assert_ne!(cmd1.command_id, cmd2.command_id);
    assert_ne!(cmd1.causation_id, cmd2.causation_id);
}

#[test]
fn command_envelope_causation_chain_traces_execution_order() {
    let parent_cmd = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-parent").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-chain").unwrap(),
        causation_id: IdempotencyKey::parse("cause-external").unwrap(),
        issuer: Issuer::Operator,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };
    let child_cmd = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-child").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-chain").unwrap(),
        causation_id: IdempotencyKey::parse("cmd-parent").unwrap(),
        issuer: Issuer::System,
        issued_at: TimestampMs::try_from(1_700_000_100u64).unwrap(),
    };
<<<<<<< HEAD
    assert_eq!(
        child_cmd.causation_id.as_str(),
        parent_cmd.command_id.as_str()
    );
=======
    assert_eq!(child_cmd.causation_id.as_str(), parent_cmd.command_id.as_str());
>>>>>>> origin/polecat/synth-mnw6kj8v
    assert_eq!(parent_cmd.correlation_id, child_cmd.correlation_id);
    assert!(child_cmd.issued_at.as_u64() > parent_cmd.issued_at.as_u64());
}

#[test]
fn command_envelope_rejects_empty_command_id() {
    let json = r#"{
        "version": 1,
        "command_id": "",
        "correlation_id": "corr-empty",
        "causation_id": "cause-empty",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(result.is_err(), "empty command_id must be rejected");
}

#[test]
fn command_envelope_rejects_empty_correlation_id() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-ok",
        "correlation_id": "",
        "causation_id": "cause-ok",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(result.is_err(), "empty correlation_id must be rejected");
}

#[test]
fn command_envelope_rejects_empty_causation_id() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-ok",
        "correlation_id": "corr-ok",
        "causation_id": "",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json);
    assert!(result.is_err(), "empty causation_id must be rejected");
}

#[test]
fn command_envelope_rejects_command_id_as_number() {
    let json = r#"{
        "version": 1,
        "command_id": 123,
        "correlation_id": "corr-ok",
        "causation_id": "cause-ok",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    assert!(CommandEnvelope::from_str(json).is_err());
}

#[test]
fn command_envelope_rejects_issued_at_as_string() {
    let json = r#"{
        "version": 1,
        "command_id": "cmd-ok",
        "correlation_id": "corr-ok",
        "causation_id": "cause-ok",
        "issuer": "system",
        "issued_at": "not-a-number"
    }"#;
    assert!(CommandEnvelope::from_str(json).is_err());
}

#[test]
fn command_envelope_max_version_constant_is_one() {
    assert_eq!(MAX_SUPPORTED_COMMAND_VERSION, 1);
}

#[test]
fn command_envelope_version_gate_prevents_future_version_routing() {
    let json_v2 = r#"{
        "version": 2,
        "command_id": "cmd-v2",
        "correlation_id": "corr-v2",
        "causation_id": "cause-v2",
        "issuer": "system",
        "issued_at": 1700000000
    }"#;
    let result = CommandEnvelope::from_str(json_v2);
<<<<<<< HEAD
    assert!(matches!(
        result,
        Err(CommandEnvelopeError::UnsupportedEnvelopeVersion(2))
    ));
=======
    assert!(matches!(result, Err(CommandEnvelopeError::UnsupportedEnvelopeVersion(2))));
>>>>>>> origin/polecat/synth-mnw6kj8v
}

#[test]
fn command_envelope_from_bytes_validates_all_identity_fields() {
    let bytes = br#"{
        "version": 1,
        "command_id": "cmd-bytes",
        "correlation_id": "corr-bytes",
        "causation_id": "cause-bytes",
        "issuer": "timer_loop",
        "issued_at": 1700000000
    }"#;
    let envelope = CommandEnvelope::from_bytes(bytes).unwrap();
    assert_eq!(envelope.metadata.command_id.as_str(), "cmd-bytes");
}

#[test]
fn command_metadata_with_max_timestamp_serializes_correctly() {
    let meta = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-max-ts").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-max-ts").unwrap(),
        causation_id: IdempotencyKey::parse("cause-max-ts").unwrap(),
        issuer: Issuer::RecoveryLoop,
        issued_at: TimestampMs::try_from(u64::MAX).unwrap(),
    };
    let json = serde_json::to_value(&meta).unwrap();
    assert_eq!(json["issued_at"], u64::MAX);
}

#[test]
fn command_metadata_with_zero_timestamp_serializes_correctly() {
    let meta = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-zero-ts").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-zero-ts").unwrap(),
        causation_id: IdempotencyKey::parse("cause-zero-ts").unwrap(),
        issuer: Issuer::TimerLoop,
        issued_at: TimestampMs::try_from(0u64).unwrap(),
    };
    let json = serde_json::to_value(&meta).unwrap();
    assert_eq!(json["issued_at"], 0);
}
