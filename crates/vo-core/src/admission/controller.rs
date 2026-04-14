//! Admission controller with degraded-mode coupling.
//!
//! This module implements the AdmissionController that couples workflow admission
//! to storage health state. When storage is degraded, new workflow admissions
//! are rejected while in-flight workflows continue to execute.

use std::collections::HashSet;

use vo_types::{DedupeKey, FenceToken, InstanceId, StepId};

use super::check::check_admission_with_thresholds;
use super::control::{AdmissionCheck, AdmissionResult, DedupeToken};
use super::types::{AdmissionError, AdmissionThresholds, WritePressureState};
use super::AdmissionThresholds as ConfiguredThresholds;
use crate::workload_class::{WorkloadBudget, WorkloadClass};

#[derive(Debug, Clone)]
pub struct AdmissionController<C: AdmissionCheck> {
    check: C,
    pressure_state: WritePressureState,
    thresholds: AdmissionThresholds,
    in_flight: HashSet<InstanceId>,
    workload_budget: Option<WorkloadBudget>,
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
            workload_budget: None,
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
            workload_budget: None,
        }
    }

    pub fn with_workload_budget(mut self, budget: WorkloadBudget) -> Self {
        self.workload_budget = Some(budget);
        self
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

    pub fn admit_new_workflow_with_class(
        &self,
        dedupe_key: &DedupeKey,
        class: WorkloadClass,
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

        if self.is_degraded() && class.is_capped_under_contention() {
            return Err(AdmissionError::WorkloadClassNotPermitted {
                class,
                reason: "lowest priority class not permitted during degraded mode",
            });
        }

        if !class.never_starved() {
            check_admission_with_thresholds(&self.pressure_state, &self.thresholds)?;
        }

        if let Some(ref budget) = self.workload_budget {
            if !budget.can_acquire(class) {
                return Err(AdmissionError::WorkloadBudgetExceeded {
                    class,
                    available: budget.remaining(class),
                    requested: 1,
                });
            }
            if budget.acquire(class).is_err() {
                return Err(AdmissionError::WorkloadBudgetExceeded {
                    class,
                    available: budget.remaining(class),
                    requested: 1,
                });
            }
        }

        Ok(dedupe_token)
    }

    pub fn is_degraded(&self) -> bool {
        self.pressure_state.writer_queue_depth > self.thresholds.writer_queue_depth_threshold
            || self.pressure_state.batch_commit_latency_ms
                > self.thresholds.batch_commit_latency_ms_threshold
            || self.pressure_state.blob_queue_depth > self.thresholds.blob_queue_depth_threshold
            || self.pressure_state.compaction_stall_active
            || self.pressure_state.storage_stall_active
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
                | AdmissionError::WorkloadBudgetExceeded { .. }
                | AdmissionError::WorkloadClassNotPermitted { .. }
        )
    }
}
