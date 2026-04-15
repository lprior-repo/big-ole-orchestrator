use vo_types::command_history::{BatchId, CommandId, SnapshotId, WorkflowSnapshot};
use vo_types::command_metadata::{CommandMetadata, Issuer};
use vo_types::signal::LineageScope;
use vo_types::signal::{BufferPolicy, SignalAddress};
use vo_types::workspace::WorkspaceMetadata;
use vo_types::LineageScope;
use vo_types::{Epoch, IdempotencyKey, InstanceId, TimestampMs, WaitKey};
use vo_types::{FenceToken, SequenceNumber, TimeoutMs};
use vo_types::{InstanceKey, PluginState, SchemaVersion};

#[test]
fn test_fence_token_new_and_conversion() {
    let token = FenceToken::new(42).unwrap();
    assert_eq!(u64::from(token), 42);
}

#[test]
fn test_fence_token_ordering() {
    let token1 = FenceToken::new(1).unwrap();
    let token2 = FenceToken::new(2).unwrap();
    assert!(token1 < token2);
    assert!(token2 > token1);
}

#[test]
fn test_fence_token_max() {
    let max = FenceToken::new(u64::MAX).unwrap();
    assert_eq!(u64::from(max), u64::MAX);
}

#[test]
fn test_fence_token_parse() {
    let token = FenceToken::parse("12345").unwrap();
    assert_eq!(u64::from(token), 12345);
}

#[test]
fn test_fence_token_next() {
    let token1 = FenceToken::new(1).unwrap();
    let token2 = token1.next().unwrap();
    assert_eq!(u64::from(token2), 2);
}

#[test]
fn test_sequence_number_try_from() {
    let seq = SequenceNumber::try_from(1u64).unwrap();
    assert_eq!(u64::from(seq), 1);
}

#[test]
fn test_sequence_number_ordering() {
    let seq1 = SequenceNumber::try_from(1u64).unwrap();
    let seq2 = SequenceNumber::try_from(2u64).unwrap();
    assert!(seq1 < seq2);
}

#[test]
fn test_command_id_new_and_display() {
    let cmd_id = CommandId::new();
    assert!(!cmd_id.to_string().is_empty());
}

#[test]
fn test_batch_id_new_and_as_str() {
    let batch_id = BatchId::new();
    assert!(!batch_id.as_str().is_empty());
}

#[test]
fn test_snapshot_id_new_and_display() {
    let snap_id = SnapshotId::new();
    assert!(!snap_id.to_string().is_empty());
}

#[test]
fn test_workflow_snapshot_with_string_workflow_name() {
    let snap = WorkflowSnapshot::new("TestWorkflow".to_string(), vec![], vec![]);
    assert_eq!(snap.workflow_name, "TestWorkflow");
}

#[test]
fn test_workspace_metadata_empty_entries() {
    let meta = WorkspaceMetadata::empty();
    assert!(meta.entries.is_empty());
    assert!(meta.validate().is_ok());
}

#[test]
fn test_workspace_metadata_with_entries() {
    let mut meta = WorkspaceMetadata::empty();
    meta.entries.insert("key".to_string(), "value".to_string());
    assert_eq!(meta.entries.len(), 1);
    assert!(meta.validate().is_ok());
}

#[test]
fn test_plugin_state_variant() {
    let state = PluginState::Registered;
    assert!(!state.is_terminal());
    assert_eq!(state.is_terminal(), false);
}

#[test]
fn test_instance_key_new() {
    let key = InstanceKey::new();
    assert!(!key.to_string().is_empty());
}

#[test]
fn test_schema_version_derives_required_traits() {
    // SchemaVersion can only be constructed within vo-types, but we can verify
    // it derives the expected traits by using it in a type position
    fn _assert_clone<T: Clone>() {}
    fn _assert_debug<T: std::fmt::Debug>() {}
    fn _assert_eq<T: Eq>() {}
    fn _assert_hash<T: std::hash::Hash>() {}
    fn _assert_serde<T: serde::Serialize + serde::de::DeserializeOwned>() {}

    // These no-op assertions verify SchemaVersion implements the required traits
    _assert_clone::<SchemaVersion>();
    _assert_debug::<SchemaVersion>();
    _assert_eq::<SchemaVersion>();
    _assert_hash::<SchemaVersion>();
    _assert_serde::<SchemaVersion>();
}

#[test]
fn test_buffer_policy_is_buffering() {
    assert!(!BufferPolicy::Reject.is_buffering());
    assert!(BufferPolicy::BufferOne.is_buffering());
    assert!(BufferPolicy::BufferMany.is_buffering());
}

#[test]
fn test_buffer_policy_default() {
    let default = BufferPolicy::default();
    assert_eq!(default, BufferPolicy::Reject);
}

#[test]
fn test_command_metadata_construction() {
    let metadata = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd001").unwrap(),
        correlation_id: IdempotencyKey::parse("corr001").unwrap(),
        causation_id: IdempotencyKey::parse("cause001").unwrap(),
        issuer: Issuer::System,
        issued_at: TimestampMs::new_unchecked(1_700_000_000u64),
    };
    assert_eq!(metadata.issuer, Issuer::System);
}

#[test]
fn test_instance_id_parse_valid_ulid() {
    let id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    assert_eq!(id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
}

#[test]
fn test_idempotency_key_parse() {
    let key = IdempotencyKey::parse("idem-key-123").unwrap();
    assert_eq!(key.to_string(), "idem-key-123");
}

#[test]
fn test_timeout_ms_try_from() {
    let timeout = TimeoutMs::try_from(1000u64).unwrap();
    assert_eq!(u64::from(timeout), 1000);
}

#[test]
fn test_timestamp_ms_new_unchecked() {
    let ts = TimestampMs::new_unchecked(1000000u64);
    assert_eq!(u64::from(ts), 1000000);
}

#[test]
fn test_signal_address_lineage_wide() {
    let lineage_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FBV").unwrap();
    let wait_key = WaitKey::parse("test-key").unwrap();
    let addr = SignalAddress::lineage_wide(lineage_id, instance_id, wait_key);
    assert_eq!(addr.lineage_scope(), LineageScope::LineageWide);
}

#[test]
fn test_signal_address_epoch_local() {
    let lineage_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let instance_id = InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FBV").unwrap();
    let wait_key = WaitKey::parse("test-key").unwrap();
    let epoch = Epoch::new(1);
    let addr = SignalAddress::epoch_local(lineage_id, epoch, instance_id, wait_key);
    assert_eq!(addr.lineage_scope(), LineageScope::EpochLocal);
}

#[test]
fn test_epoch_new() {
    let epoch = Epoch::new(42);
    assert_eq!(epoch.0, 42);
}

#[test]
fn test_epoch_zero() {
    let epoch = Epoch::ZERO;
    assert_eq!(epoch.0, 0);
}
