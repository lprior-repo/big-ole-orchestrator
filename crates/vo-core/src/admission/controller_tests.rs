//! Tests for admission controller with degraded-mode coupling.
//!
//! These tests verify that the AdmissionController correctly couples
//! workflow admission to storage health state.

use super::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Barrier, Mutex};
use vo_types::{DedupeKey, FenceToken, InstanceId, StepId};

#[derive(Debug, Clone)]
struct MockAdmissionCheck {
    admitted_keys: HashSet<String>,
}

impl MockAdmissionCheck {
    fn new() -> Self {
        Self {
            admitted_keys: HashSet::new(),
        }
    }

    fn with_admitted_key(mut self, key: &str) -> Self {
        self.admitted_keys.insert(key.to_string());
        self
    }
}

impl AdmissionCheck for MockAdmissionCheck {
    fn check_deduplicate(&self, dedupe_key: &DedupeKey) -> AdmissionResult {
        if self.admitted_keys.contains(dedupe_key.as_str()) {
            AdmissionResult::Duplicate {
                original_instance_id: InstanceId::from_bytes([1u8; 16]),
            }
        } else {
            AdmissionResult::Admitted {
                dedupe_token: DedupeToken::new("token".to_string()),
            }
        }
    }

    fn check_fence(
        &self,
        _instance_id: &InstanceId,
        _step_id: &StepId,
        _fence_token: &FenceToken,
    ) -> AdmissionResult {
        AdmissionResult::Admitted {
            dedupe_token: DedupeToken::new("fence-token".to_string()),
        }
    }
}

fn healthy_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    }
}

fn storage_stall_only_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: true,
    }
}

fn degraded_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 1500,
        blob_queue_depth: 100,
        compaction_stall_active: true,
        storage_stall_active: true,
    }
}

#[test]
fn admits_new_workflow_when_storage_is_healthy() {
    let check = MockAdmissionCheck::new();
    let controller = AdmissionController::new(check, healthy_state());

    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key-1").expect("valid"));
    assert!(
        result.is_ok(),
        "Should admit when storage is healthy, got {:?}",
        result
    );
}

#[test]
fn rejects_new_workflow_when_storage_is_stalled() {
    let check = MockAdmissionCheck::new();
    let controller = AdmissionController::new(check, storage_stall_only_state());

    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key-2").expect("valid"));
    assert!(result.is_err());
    match result.unwrap_err() {
        AdmissionError::StorageStallActive => {}
        other => panic!("Expected StorageStallActive, got {:?}", other),
    }
}

#[test]
fn in_flight_workflow_proceeds_regardless_of_state() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, healthy_state());

    let instance_id = InstanceId::from_bytes([42u8; 16]);
    controller.mark_in_flight(&instance_id);

    let result = controller.step_in_flight(&instance_id);
    assert!(
        result.is_ok(),
        "In-flight workflow should proceed regardless of state, got {:?}",
        result
    );
}

#[test]
fn multiple_pressure_indicators_returned_when_storage_degraded() {
    let check = MockAdmissionCheck::new();
    let controller = AdmissionController::new(check, degraded_state());

    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_err());
    match result.unwrap_err() {
        AdmissionError::MultiplePressureIndicators { indicators } => {
            assert!(indicators.contains(&PressureIndicator::WriterQueueDepth));
            assert!(indicators.contains(&PressureIndicator::BatchCommitLatency));
            assert!(indicators.contains(&PressureIndicator::BlobQueueDepth));
            assert!(indicators.contains(&PressureIndicator::CompactionStall));
            assert!(indicators.contains(&PressureIndicator::StorageStall));
        }
        other => panic!("Expected MultiplePressureIndicators, got {:?}", other),
    }
}

#[test]
fn admit_new_workflow_checks_dedupe_first() {
    let check = MockAdmissionCheck::new().with_admitted_key("duplicate-key");
    let controller = AdmissionController::new(check, healthy_state());

    let result = controller.admit_new_workflow(&DedupeKey::parse("duplicate-key").expect("valid"));
    assert!(matches!(result, Err(AdmissionError::Duplicate { .. })));
}

#[test]
fn step_in_flight_checks_fence() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, healthy_state());

    let instance_id = InstanceId::from_bytes([1u8; 16]);
    controller.mark_in_flight(&instance_id);
    let step_id = StepId::parse("test-step").expect("valid");
    let fence_token = FenceToken::new(1).expect("valid");

    let result = controller.step_in_flight_with_fence(&instance_id, &step_id, &fence_token);
    assert!(
        result.is_ok(),
        "Should allow fenced step for in-flight workflow"
    );
}

#[test]
fn controller_with_zero_thresholds_accepts_zero_values() {
    let check = MockAdmissionCheck::new();
    let state = WritePressureState {
        writer_queue_depth: 0,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 0,
        batch_commit_latency_ms_threshold: 0,
        blob_queue_depth_threshold: 0,
    };
    let controller = AdmissionController::with_thresholds(check, state, &thresholds);

    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(result.is_ok());
}

#[test]
fn controller_returns_dedupe_token_on_successful_admission() {
    let check = MockAdmissionCheck::new();
    let controller = AdmissionController::new(check, healthy_state());

    let result = controller.admit_new_workflow(&DedupeKey::parse("unique-key").expect("valid"));
    assert!(result.is_ok());
    let token = result.unwrap();
    assert_eq!(token.as_str(), "token");
}

#[test]
fn is_in_flight_returns_true_for_marked_instance() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, healthy_state());

    let instance_id = InstanceId::from_bytes([99u8; 16]);
    assert!(!controller.is_in_flight(&instance_id));

    controller.mark_in_flight(&instance_id);
    assert!(controller.is_in_flight(&instance_id));
}

#[test]
fn step_in_flight_returns_error_for_unknown_instance() {
    let check = MockAdmissionCheck::new();
    let controller = AdmissionController::new(check, healthy_state());

    let unknown_id = InstanceId::from_bytes([5u8; 16]);
    let result = controller.step_in_flight(&unknown_id);
    assert!(result.is_err());
    match result.unwrap_err() {
        AdmissionError::InvalidAdmissionContext => {}
        other => panic!("Expected InvalidAdmissionContext, got {:?}", other),
    }
}

#[test]
fn is_degraded_error_identifies_storage_stall() {
    assert!(AdmissionError::StorageStallActive.is_degraded_error());
}

#[test]
fn is_degraded_error_identifies_multiple_indicators() {
    let indicators = vec![
        PressureIndicator::WriterQueueDepth,
        PressureIndicator::BatchCommitLatency,
    ];
    let err = AdmissionError::MultiplePressureIndicators { indicators };
    assert!(err.is_degraded_error());
}

#[test]
fn is_degraded_error_identifies_false_for_non_degraded() {
    let err = AdmissionError::InvalidAdmissionContext;
    assert!(!err.is_degraded_error());
}

#[test]
fn step_in_flight_with_fence_returns_error_for_unknown_instance() {
    let check = MockAdmissionCheck::new();
    let controller = AdmissionController::new(check, healthy_state());

    let unknown_id = InstanceId::from_bytes([7u8; 16]);
    let step_id = StepId::parse("test-step").expect("valid");
    let fence_token = FenceToken::new(1).expect("valid");

    let result = controller.step_in_flight_with_fence(&unknown_id, &step_id, &fence_token);
    assert!(result.is_err());
    match result.unwrap_err() {
        AdmissionError::InvalidAdmissionContext => {}
        other => panic!("Expected InvalidAdmissionContext, got {:?}", other),
    }
}

#[test]
fn new_workflow_admitted_when_storage_is_healthy() {
    let check = MockAdmissionCheck::new();
    let controller = AdmissionController::new(check, healthy_state());

    let result = controller.admit_new_workflow(&DedupeKey::parse("new-key").expect("valid"));
    assert!(
        result.is_ok(),
        "New workflow should be admitted when storage is healthy"
    );
}

// ============================================================================
// BDD: Concurrent duplicate starts → exactly one instance (ADR-028, ADR-043)
// ============================================================================

#[derive(Clone)]
struct ConcurrentDedupeCheck {
    entries: Arc<Mutex<HashMap<String, InstanceId>>>,
}

impl ConcurrentDedupeCheck {
    fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl AdmissionCheck for ConcurrentDedupeCheck {
    fn check_deduplicate(&self, dedupe_key: &DedupeKey) -> AdmissionResult {
        let mut entries = self.entries.lock().expect("lock poisoned");
        if let Some(original_id) = entries.get(dedupe_key.as_str()) {
            return AdmissionResult::Duplicate {
                original_instance_id: original_id.clone(),
            };
        }
        let instance_id = InstanceId::from_bytes([0xAB; 16]);
        entries.insert(dedupe_key.as_str().to_string(), instance_id.clone());
        AdmissionResult::Admitted {
            dedupe_token: DedupeToken::new(format!("token-{}", dedupe_key.as_str())),
        }
    }

    fn check_fence(
        &self,
        _instance_id: &InstanceId,
        _step_id: &StepId,
        _fence_token: &FenceToken,
    ) -> AdmissionResult {
        AdmissionResult::Admitted {
            dedupe_token: DedupeToken::new("fence-ok".to_string()),
        }
    }
}

#[test]
fn given_concurrent_duplicate_starts_when_admitted_then_only_one_instance_exists() {
    let check = Arc::new(ConcurrentDedupeCheck::new());
    let dedupe_key = DedupeKey::parse("concurrent-dedupe-start-bdd").expect("valid");
    let num_threads: usize = 8;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let check = (*check).clone();
            let dedupe_key = dedupe_key.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let controller = AdmissionController::new(check, healthy_state());
                controller.admit_new_workflow(&dedupe_key)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread panicked")).collect();

    let admitted: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();
    let duplicates: Vec<_> = results
        .iter()
        .filter_map(|r| match r {
            Err(AdmissionError::Duplicate { original_instance_id }) => {
                Some(original_instance_id.clone())
            }
            _ => None,
        })
        .collect();

    assert_eq!(
        admitted.len(),
        1,
        "Exactly one concurrent start must be admitted, got {}",
        admitted.len()
    );
    assert_eq!(
        duplicates.len(),
        num_threads - 1,
        "All other concurrent starts must be duplicates, got {}",
        duplicates.len()
    );

    let winner_id = InstanceId::from_bytes([0xAB; 16]);
    for dup_id in &duplicates {
        assert_eq!(
            dup_id, &winner_id,
            "Every duplicate must reference the same winner instance_id"
        );
    }
}
