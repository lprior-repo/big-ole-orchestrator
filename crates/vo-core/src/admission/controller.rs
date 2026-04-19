//! Admission controller with degraded-mode coupling.
//!
//! This module implements the AdmissionController that couples workflow admission
//! to storage health state. When storage is degraded, new workflow admissions
//! are rejected while in-flight workflows continue to execute.

use std::collections::HashSet;

use vo_types::{DedupeKey, FenceToken, InstanceId, StepId};

use super::check::check_admission_with_thresholds;
use super::control::{AdmissionCheck, AdmissionResult, DedupeToken};
use super::types::{AdmissionDeadLetterQueue, AdmissionError, AdmissionThresholds, RejectedRequest, WritePressureState};
use super::AdmissionThresholds as ConfiguredThresholds;

#[derive(Debug, Clone)]
pub struct AdmissionController<C: AdmissionCheck> {
    check: C,
    pressure_state: WritePressureState,
    thresholds: AdmissionThresholds,
    in_flight: HashSet<InstanceId>,
    dlq: AdmissionDeadLetterQueue,
}

#[derive(Debug, Clone)]
pub struct AdmissionControllerWithDlq<C: AdmissionCheck> {
    controller: AdmissionController<C>,
    dlq_max_size: usize,
}

impl<C: AdmissionCheck> AdmissionController<C> {
    pub fn new(check: C, pressure_state: WritePressureState) -> Self {
        Self {
            check,
            pressure_state,
            thresholds: AdmissionThresholds {
                writer_queue_depth_threshold: 100,
                batch_commit_latency_ms_threshold: 1000,
                blob_queue_depth_threshold: 50,
            },
            in_flight: HashSet::new(),
            dlq: AdmissionDeadLetterQueue::new(1000),
        }
    }

    pub fn with_thresholds(
        check: C,
        pressure_state: WritePressureState,
        thresholds: &ConfiguredThresholds,
    ) -> Self {
        Self {
            check,
            pressure_state,
            thresholds: AdmissionThresholds {
                writer_queue_depth_threshold: thresholds.writer_queue_depth_threshold,
                batch_commit_latency_ms_threshold: thresholds.batch_commit_latency_ms_threshold,
                blob_queue_depth_threshold: thresholds.blob_queue_depth_threshold,
            },
            in_flight: HashSet::new(),
            dlq: AdmissionDeadLetterQueue::new(1000),
        }
    }

    pub fn admit_new_workflow(
        &self,
        dedupe_key: &DedupeKey,
    ) -> Result<DedupeToken, AdmissionError> {
        let dedupe_result = self.check.check_deduplicate(dedupe_key);
        let dedupe_token = match dedupe_result {
            AdmissionResult::Admitted { dedupe_token } => dedupe_token,
            AdmissionResult::Duplicate {
                original_instance_id,
            } => {
                return Err(AdmissionError::Duplicate {
                    original_instance_id,
                });
            }
            AdmissionResult::Rejected { reason } => {
                return Err(AdmissionError::PolicyViolation(reason.to_string()));
            }
        };

        check_admission_with_thresholds(&self.pressure_state, &self.thresholds)?;

        Ok(dedupe_token)
    }

    pub fn mark_in_flight(&mut self, instance_id: &InstanceId) {
        self.in_flight.insert(instance_id.clone());
    }

    pub fn step_in_flight(&self, instance_id: &InstanceId) -> Result<(), AdmissionError> {
        if self.in_flight.contains(instance_id) {
            Ok(())
        } else {
            Err(AdmissionError::InvalidAdmissionContext)
        }
    }

    pub fn step_in_flight_with_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        fence_token: &FenceToken,
    ) -> Result<DedupeToken, AdmissionError> {
        if !self.in_flight.contains(instance_id) {
            return Err(AdmissionError::InvalidAdmissionContext);
        }

        let result = self.check.check_fence(instance_id, step_id, fence_token);
        match result {
            AdmissionResult::Admitted { dedupe_token } => Ok(dedupe_token),
            AdmissionResult::Rejected { reason } => {
                Err(AdmissionError::PolicyViolation(reason.to_string()))
            }
            AdmissionResult::Duplicate {
                original_instance_id,
            } => Err(AdmissionError::Duplicate {
                original_instance_id,
            }),
        }
    }

    pub fn is_in_flight(&self, instance_id: &InstanceId) -> bool {
        self.in_flight.contains(instance_id)
    }

    pub fn enqueue_rejected(&mut self, dedupe_key: DedupeKey, reason: AdmissionError) {
        self.dlq.enqueue(RejectedRequest::new(dedupe_key, reason));
    }

    pub fn dequeue_rejected(&mut self) -> Option<RejectedRequest> {
        self.dlq.dequeue()
    }

    pub fn dlq_len(&self) -> usize {
        self.dlq.len()
    }

    pub fn dlq_is_empty(&self) -> bool {
        self.dlq.is_empty()
    }

    pub fn dlq_entries(&self) -> &[RejectedRequest] {
        self.dlq.entries()
    }

    pub fn clear_dlq(&mut self) {
        self.dlq.clear();
    }

    pub fn retry_from_dlq(&mut self) -> Result<DedupeToken, AdmissionError> {
        let rejected = self.dlq.dequeue().ok_or(AdmissionError::InvalidAdmissionContext)?;
        self.admit_new_workflow(&rejected.dedupe_key)
    }
}

impl<C: AdmissionCheck> AdmissionControllerWithDlq<C> {
    pub fn new(check: C, pressure_state: WritePressureState, dlq_max_size: usize) -> Self {
        Self {
            controller: AdmissionController::new(check, pressure_state),
            dlq_max_size,
        }
    }

    pub fn with_thresholds(check: C, pressure_state: WritePressureState, thresholds: &ConfiguredThresholds, dlq_max_size: usize) -> Self {
        Self {
            controller: AdmissionController::with_thresholds(check, pressure_state, thresholds),
            dlq_max_size,
        }
    }

    pub fn admit_new_workflow(&self, dedupe_key: &DedupeKey) -> Result<DedupeToken, AdmissionError> {
        self.controller.admit_new_workflow(dedupe_key)
    }

    pub fn admit_new_workflow_with_dlq(
        &mut self,
        dedupe_key: &DedupeKey,
    ) -> Result<DedupeToken, AdmissionError> {
        match self.controller.admit_new_workflow(dedupe_key) {
            Ok(token) => Ok(token),
            Err(e) => {
                self.controller.enqueue_rejected(dedupe_key.clone(), e.clone());
                Err(e)
            }
        }
    }

    pub fn mark_in_flight(&mut self, instance_id: &InstanceId) {
        self.controller.mark_in_flight(instance_id);
    }

    pub fn step_in_flight(&self, instance_id: &InstanceId) -> Result<(), AdmissionError> {
        self.controller.step_in_flight(instance_id)
    }

    pub fn step_in_flight_with_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        fence_token: &FenceToken,
    ) -> Result<DedupeToken, AdmissionError> {
        self.controller.step_in_flight_with_fence(instance_id, step_id, fence_token)
    }

    pub fn is_in_flight(&self, instance_id: &InstanceId) -> bool {
        self.controller.is_in_flight(instance_id)
    }

    pub fn dequeue_rejected(&mut self) -> Option<RejectedRequest> {
        self.controller.dequeue_rejected()
    }

    pub fn enqueue_rejected(&mut self, dedupe_key: DedupeKey, reason: AdmissionError) {
        self.controller.enqueue_rejected(dedupe_key, reason);
    }

    pub fn dlq_len(&self) -> usize {
        self.controller.dlq_len()
    }

    pub fn dlq_is_empty(&self) -> bool {
        self.controller.dlq_is_empty()
    }

    pub fn dlq_entries(&self) -> &[RejectedRequest] {
        self.controller.dlq_entries()
    }

    pub fn clear_dlq(&mut self) {
        self.controller.clear_dlq();
    }

    pub fn retry_from_dlq(&mut self) -> Result<DedupeToken, AdmissionError> {
        self.controller.retry_from_dlq()
    }
}

impl AdmissionError {
    pub fn is_degraded_error(&self) -> bool {
        matches!(
            self,
            AdmissionError::WriterQueueDepthExceeded { .. }
                | AdmissionError::BatchCommitLatencyExceeded { .. }
                | AdmissionError::BlobQueueDepthExceeded { .. }
                | AdmissionError::CompactionStallActive
                | AdmissionError::StorageStallActive
                | AdmissionError::MultiplePressureIndicators { .. }
        )
    }
}
