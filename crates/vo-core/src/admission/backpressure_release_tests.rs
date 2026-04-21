//! Backpressure release tests.
//!
//! These tests verify that admission control correctly releases backpressure
//! when pressure indicators drop below thresholds.

use super::*;
use std::collections::HashSet;
use vo_types::{DedupeKey, InstanceId};

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
        _step_id: &vo_types::StepId,
        _fence_token: &vo_types::FenceToken,
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

fn degraded_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 1500,
        blob_queue_depth: 100,
        compaction_stall_active: true,
        storage_stall_active: true,
    }
}

fn writer_queue_degraded_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    }
}

fn batch_latency_degraded_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 1500,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    }
}

fn blob_queue_degraded_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 100,
        compaction_stall_active: false,
        storage_stall_active: false,
    }
}

fn storage_stall_degraded_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: true,
    }
}

// ── Threshold Detection Tests ─────────────────────────────────────────────────

#[test]
fn threshold_detection_writer_queue_boundary_at() {
    let check = MockAdmissionCheck::new();
    let state = WritePressureState {
        writer_queue_depth: 100,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };
    let controller = AdmissionController::with_thresholds(check, state, &thresholds);

    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(
        result.is_ok(),
        "Should admit at exact threshold (100), got {:?}",
        result
    );
}

#[test]
fn threshold_detection_writer_queue_boundary_over() {
    let check = MockAdmissionCheck::new();
    let state = WritePressureState {
        writer_queue_depth: 101,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };
    let controller = AdmissionController::with_thresholds(check, state, &thresholds);

    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(
        result.is_err(),
        "Should reject over threshold (101 > 100), got {:?}",
        result
    );
}

#[test]
fn threshold_detection_batch_latency_boundary_at() {
    let check = MockAdmissionCheck::new();
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 1000,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };
    let controller = AdmissionController::with_thresholds(check, state, &thresholds);

    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(
        result.is_ok(),
        "Should admit at exact threshold (1000ms), got {:?}",
        result
    );
}

#[test]
fn threshold_detection_batch_latency_boundary_over() {
    let check = MockAdmissionCheck::new();
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 1001,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };
    let controller = AdmissionController::with_thresholds(check, state, &thresholds);

    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(
        result.is_err(),
        "Should reject over threshold (1001ms > 1000ms), got {:?}",
        result
    );
}

#[test]
fn threshold_detection_blob_queue_boundary_at() {
    let check = MockAdmissionCheck::new();
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 50,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };
    let controller = AdmissionController::with_thresholds(check, state, &thresholds);

    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(
        result.is_ok(),
        "Should admit at exact threshold (50), got {:?}",
        result
    );
}

#[test]
fn threshold_detection_blob_queue_boundary_over() {
    let check = MockAdmissionCheck::new();
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 51,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 100,
        batch_commit_latency_ms_threshold: 1000,
        blob_queue_depth_threshold: 50,
    };
    let controller = AdmissionController::with_thresholds(check, state, &thresholds);

    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(
        result.is_err(),
        "Should reject over threshold (51 > 50), got {:?}",
        result
    );
}

#[test]
fn threshold_detection_compaction_stall_active_rejects() {
    let check = MockAdmissionCheck::new();
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: true,
        storage_stall_active: false,
    };
    let controller = AdmissionController::new(check, state);

    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(
        result.is_err(),
        "Should reject when compaction stall is active, got {:?}",
        result
    );
}

#[test]
fn threshold_detection_storage_stall_active_rejects() {
    let check = MockAdmissionCheck::new();
    let state = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: true,
    };
    let controller = AdmissionController::new(check, state);

    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(
        result.is_err(),
        "Should reject when storage stall is active, got {:?}",
        result
    );
}

// ── Backpressure Release Tests ─────────────────────────────────────────────────

#[test]
fn backpressure_release_all_indicators_drop_below_threshold() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, degraded_state());

    // Initially rejected due to degraded state
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(
        result.is_err(),
        "Should reject in degraded state, got {:?}",
        result
    );

    // Update to healthy state (all indicators drop below threshold)
    let healthy = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    controller.update_pressure_state(healthy);

    // Should now admit after release
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(
        result.is_ok(),
        "Should admit after backpressure release, got {:?}",
        result
    );
}

#[test]
fn backpressure_release_writer_queue_only() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, writer_queue_degraded_state());

    // Initially rejected due to writer queue
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_err(), "Should reject with writer queue degraded");

    // Update to healthy state
    let healthy = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    controller.update_pressure_state(healthy);

    // Should now admit
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_ok(), "Should admit after writer queue release");
}

#[test]
fn backpressure_release_batch_latency_only() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, batch_latency_degraded_state());

    // Initially rejected due to batch latency
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_err(), "Should reject with batch latency degraded");

    // Update to healthy state
    let healthy = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    controller.update_pressure_state(healthy);

    // Should now admit
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_ok(), "Should admit after batch latency release");
}

#[test]
fn backpressure_release_blob_queue_only() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, blob_queue_degraded_state());

    // Initially rejected due to blob queue
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_err(), "Should reject with blob queue degraded");

    // Update to healthy state
    let healthy = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    controller.update_pressure_state(healthy);

    // Should now admit
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_ok(), "Should admit after blob queue release");
}

#[test]
fn backpressure_release_storage_stall_only() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, storage_stall_degraded_state());

    // Initially rejected due to storage stall
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_err(), "Should reject with storage stall active");

    // Update to healthy state
    let healthy = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    controller.update_pressure_state(healthy);

    // Should now admit
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_ok(), "Should admit after storage stall release");
}

#[test]
fn backpressure_release_partial_indicator_clears() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, degraded_state());

    // Initially rejected due to multiple indicators
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_err(), "Should reject with multiple indicators");

    // Update to state where only writer queue is still high
    let partial_healthy = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    controller.update_pressure_state(partial_healthy);

    // Should still be rejected (writer queue still exceeds threshold)
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(
        result.is_err(),
        "Should still reject with writer queue exceeded, got {:?}",
        result
    );

    // Finally update to fully healthy
    let healthy = WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    controller.update_pressure_state(healthy);

    // Should now admit
    let result = controller.admit_new_workflow(&DedupeKey::parse("test-key").expect("valid"));
    assert!(result.is_ok(), "Should admit after all indicators released");
}

#[test]
fn backpressure_release_sequence_pressure_rises_admitted_drops_rejected() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, healthy_state());

    // Start healthy - admit
    let result = controller.admit_new_workflow(&DedupeKey::parse("key-1").expect("valid"));
    assert!(result.is_ok(), "Should admit when healthy initially");

    // Pressure rises - reject
    controller.update_pressure_state(degraded_state());
    let result = controller.admit_new_workflow(&DedupeKey::parse("key-2").expect("valid"));
    assert!(
        result.is_err(),
        "Should reject when pressure rises"
    );

    // Pressure drops - admit again
    controller.update_pressure_state(healthy_state());
    let result = controller.admit_new_workflow(&DedupeKey::parse("key-3").expect("valid"));
    assert!(
        result.is_ok(),
        "Should admit when pressure drops again"
    );
}

  #[test]
   fn backpressure_release_sequence_preserves_in_flight_workflows() {
       let check = MockAdmissionCheck::new();
       let mut controller = AdmissionController::new(check, healthy_state());
   
       let instance_id = InstanceId::from_bytes([1u8; 16]);
       controller.mark_in_flight(&instance_id);
   
       // Marked in-flight should proceed regardless of pressure state
       let result = controller.step_in_flight(&instance_id);
       assert!(result.is_ok(), "In-flight workflow should proceed");
   
       // Pressure rises
       controller.update_pressure_state(degraded_state());
   
       // Still in-flight, should proceed
       let result = controller.step_in_flight(&instance_id);
       assert!(result.is_ok(), "In-flight workflow should proceed under pressure");
   
       // New workflow should be rejected
       let result = controller.admit_new_workflow(&DedupeKey::parse("new-key").expect("valid"));
       assert!(result.is_err(), "New workflow should be rejected under pressure");
   }

#[test]
fn backpressure_release_immediate_after_threshold_drop() {
    let check = MockAdmissionCheck::new();
    let mut controller = AdmissionController::new(check, writer_queue_degraded_state());

    // Reject when degraded
    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(result.is_err());

    // Drop below threshold by 1
    let just_under = WritePressureState {
        writer_queue_depth: 99,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    controller.update_pressure_state(just_under);

    // Should immediately admit
    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(result.is_ok(), "Should admit immediately when dropping below threshold");
}

#[test]
fn backpressure_release_zero_threshold_edge_case() {
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
    let mut controller = AdmissionController::with_thresholds(check, state.clone(), &thresholds);

    // Should admit at zero with zero thresholds
    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(result.is_ok());

    // Add pressure
    let degraded = WritePressureState {
        writer_queue_depth: 1,
        batch_commit_latency_ms: 0,
        blob_queue_depth: 0,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    controller.update_pressure_state(degraded);

    // Should reject
    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(result.is_err());

    // Release back to zero
    controller.update_pressure_state(state);

    // Should admit again
    let result = controller.admit_new_workflow(&DedupeKey::parse("test").expect("valid"));
    assert!(result.is_ok());
}
