use crate::admission::control::{DedupeToken, RejectionReason};
use crate::admission::types::{AdmissionError, AdmissionThresholds, PressureIndicator, WritePressureState};

#[test]
fn dedupe_token_accepts_empty_string_violating_inv_adm_004() {
    let token = DedupeToken::new(String::new());
    assert_eq!(token.as_str(), "");
}

#[test]
fn dedupe_token_accepts_whitespace_only() {
    let token = DedupeToken::new("   ".to_string());
    assert_eq!(token.as_str(), "   ");
}

#[test]
fn dedupe_token_accepts_unicode_control_chars() {
    let token = DedupeToken::new("\0\x01\x02".to_string());
    assert_eq!(token.as_str(), "\0\x01\x02");
}

#[test]
fn rejection_reason_dedupe_key_too_long_carries_lengths() {
    let reason = RejectionReason::DedupeKeyTooLong {
        max_length: 256,
        actual_length: 512,
    };
    let msg = reason.to_string();
    assert!(msg.contains("512"));
    assert!(msg.contains("256"));
}

#[test]
fn rejection_reason_fence_token_mismatch_display() {
    use vo_types::FenceToken;
    let expected = FenceToken::new(42).unwrap();
    let actual = FenceToken::new(99).unwrap();
    let reason = RejectionReason::FenceTokenMismatch {
        expected: expected.clone(),
        actual: actual.clone(),
    };
    let msg = reason.to_string();
    assert!(msg.contains("mismatch"));
}

#[test]
fn admission_thresholds_all_zero_means_any_nonzero_rejected() {
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 0,
        batch_commit_latency_ms_threshold: 0,
        blob_queue_depth_threshold: 0,
    };
    let state = WritePressureState {
        writer_queue_depth: 1,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
    assert!(result.is_err());
}

#[test]
fn admission_thresholds_max_values_means_nothing_rejected() {
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: u64::MAX,
        batch_commit_latency_ms_threshold: u64::MAX,
        blob_queue_depth_threshold: u64::MAX,
    };
    let state = WritePressureState {
        writer_queue_depth: u64::MAX,
        batch_commit_latency_ms: u64::MAX,
        blob_queue_depth: u64::MAX,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
    assert!(result.is_ok());
}

#[test]
fn admission_error_multiple_indicators_aggregates() {
    let indicators = vec![
        PressureIndicator::WriterQueueDepth,
        PressureIndicator::BatchCommitLatency,
    ];
    let err = AdmissionError::MultiplePressureIndicators { indicators };
    let msg = format!("{err:?}");
    assert!(msg.contains("MultiplePressureIndicators"));
}

#[test]
fn admission_compaction_stall_active_rejects() {
    let state = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: true,
        storage_stall_active: false,
    };
    let result = crate::admission::check::check_admission(&state);
    assert!(result.is_err());
    assert!(matches!(result, Err(AdmissionError::CompactionStallActive)));
}

#[test]
fn admission_storage_stall_active_rejects() {
    let state = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: true,
    };
    let result = crate::admission::check::check_admission(&state);
    assert!(result.is_err());
    assert!(matches!(result, Err(AdmissionError::StorageStallActive)));
}

#[test]
fn admission_blob_queue_depth_exceeds_threshold() {
    let state = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 100,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let result = crate::admission::check::check_admission(&state);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(AdmissionError::BlobQueueDepthExceeded { .. })
    ));
}
