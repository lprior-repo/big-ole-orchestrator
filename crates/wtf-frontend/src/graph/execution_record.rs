#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use super::{ExecutionState, NodeId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExecutionError(String);

impl ExecutionError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_message(self) -> String {
        self.0
    }
}

impl TryFrom<String> for ExecutionError {
    type Error = EmptyErrorMessage;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(EmptyErrorMessage);
        }
        Ok(Self(value))
    }
}

impl From<ExecutionError> for String {
    fn from(value: ExecutionError) -> Self {
        value.0
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ExecutionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyErrorMessage;

impl fmt::Display for EmptyErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error message cannot be empty")
    }
}

impl std::error::Error for EmptyErrorMessage {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepName(String);

impl StepName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StepName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for StepName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for StepName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepType(String);

impl StepType {
    #[must_use]
    pub fn new(step_type: impl Into<String>) -> Self {
        Self(step_type.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StepType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for StepType {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for StepType {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttemptNumber(pub u32);

impl AttemptNumber {
    #[must_use]
    pub const fn first() -> Self {
        Self(1)
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExecutionRecordId(Uuid);

impl ExecutionRecordId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ExecutionRecordId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for ExecutionRecordId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ExecutionRecordId> for Uuid {
    fn from(id: ExecutionRecordId) -> Self {
        id.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowName(String);

impl WorkflowName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkflowName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for WorkflowName {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepCount(pub u32);

impl StepCount {
    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn increment(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for StepCount {
    fn default() -> Self {
        Self::zero()
    }
}

/// Overall status of a complete workflow execution run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionOverallStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl ExecutionOverallStatus {
    /// Returns `true` if the execution has reached a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

/// The output produced by a single step execution.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum StepOutput {
    Success(serde_json::Value),
    Failure {
        error: ExecutionError,
        stack_trace: Option<String>,
    },
    Pending,
}

/// A frozen record of a single step's execution within a workflow run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepRecord {
    pub step_name: StepName,
    pub step_type: StepType,
    pub status: ExecutionState,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub attempt: AttemptNumber,
    pub input: Option<serde_json::Value>,
    pub output: StepOutput,
}

impl StepRecord {
    /// Computes the wall-clock duration of this step in milliseconds.
    ///
    /// Returns `None` if either `start_time` or `end_time` is absent,
    /// or if the duration cannot be represented as `i64`.
    #[must_use]
    pub fn duration_ms(&self) -> Option<i64> {
        let start = self.start_time?;
        let end = self.end_time?;
        end.signed_duration_since(start).num_milliseconds().into()
    }
}

/// A complete, frozen snapshot of a single workflow execution run.
///
/// Once `status` is terminal (`is_frozen()` returns `true`) this record
/// must not be mutated — it serves as the immutable history entry for
/// replay and inspection.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: ExecutionRecordId,
    pub workflow_name: WorkflowName,
    pub status: ExecutionOverallStatus,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub steps: Vec<(NodeId, StepRecord)>,
    pub steps_completed: StepCount,
    pub steps_failed: StepCount,
}

impl ExecutionRecord {
    /// Computes the total wall-clock duration of this execution in milliseconds.
    ///
    /// Returns `None` if `end_time` has not yet been set.
    #[must_use]
    pub fn duration_ms(&self) -> Option<i64> {
        let end = self.end_time?;
        end.signed_duration_since(self.start_time)
            .num_milliseconds()
            .into()
    }

    /// Returns `true` if this record is frozen, i.e. the execution has reached
    /// a terminal state and no further mutations should occur.
    #[must_use]
    pub const fn is_frozen(&self) -> bool {
        self.status.is_terminal()
    }

    /// Looks up the [`StepRecord`] for a given [`NodeId`].
    ///
    /// Returns `None` if no step with that ID was recorded.
    #[must_use]
    pub fn step_for_node(&self, id: NodeId) -> Option<&StepRecord> {
        self.steps
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .map(|(_, record)| record)
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Converts a legacy [`super::RunRecord`] into an [`ExecutionRecord`] for
/// display purposes.
///
/// Pure function — no side effects.
#[must_use]
pub fn from_run_record(record: &super::RunRecord) -> ExecutionRecord {
    let steps_completed = StepCount(u32::try_from(record.results.len()).unwrap_or(0));
    let steps_failed = if record.success {
        StepCount::zero()
    } else {
        StepCount::zero().increment()
    };
    let status = if record.success {
        ExecutionOverallStatus::Succeeded
    } else {
        ExecutionOverallStatus::Failed
    };

    ExecutionRecord {
        id: ExecutionRecordId::from(record.id),
        workflow_name: WorkflowName::default(),
        status,
        start_time: record.timestamp,
        end_time: None,
        steps: record
            .results
            .iter()
            .map(|(node_id, output)| {
                use crate::graph::ExecutionState;
                (
                    *node_id,
                    StepRecord {
                        step_name: StepName::new(""),
                        step_type: StepType::new(""),
                        status: if record.success {
                            ExecutionState::Completed
                        } else {
                            ExecutionState::Failed
                        },
                        start_time: Some(record.timestamp),
                        end_time: None,
                        attempt: AttemptNumber::first(),
                        input: None,
                        output: StepOutput::Success(output.clone()),
                    },
                )
            })
            .collect(),
        steps_completed,
        steps_failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(secs: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.timestamp_opt(secs, 0).single().unwrap()
    }

    fn make_step(start: Option<i64>, end: Option<i64>) -> StepRecord {
        StepRecord {
            step_name: StepName::new("test-step"),
            step_type: StepType::new("run"),
            status: ExecutionState::Completed,
            start_time: start.map(utc),
            end_time: end.map(utc),
            attempt: AttemptNumber::first(),
            input: None,
            output: StepOutput::Pending,
        }
    }

    fn make_record(
        start: i64,
        end: Option<i64>,
        status: ExecutionOverallStatus,
    ) -> ExecutionRecord {
        ExecutionRecord {
            id: ExecutionRecordId::new(),
            workflow_name: WorkflowName::new("wf"),
            status,
            start_time: utc(start),
            end_time: end.map(utc),
            steps: Vec::new(),
            steps_completed: StepCount::zero(),
            steps_failed: StepCount::zero(),
        }
    }

    // ── StepRecord::duration_ms ──────────────────────────────────────────────

    #[test]
    fn given_step_with_both_times_when_duration_ms_then_correct_delta_returned() {
        let step = make_step(Some(1_000), Some(1_002));
        assert_eq!(step.duration_ms(), Some(2_000));
    }

    #[test]
    fn given_step_with_no_start_time_when_duration_ms_then_none_returned() {
        let step = make_step(None, Some(1_002));
        assert_eq!(step.duration_ms(), None);
    }

    #[test]
    fn given_step_with_no_end_time_when_duration_ms_then_none_returned() {
        let step = make_step(Some(1_000), None);
        assert_eq!(step.duration_ms(), None);
    }

    #[test]
    fn given_step_with_no_times_when_duration_ms_then_none_returned() {
        let step = make_step(None, None);
        assert_eq!(step.duration_ms(), None);
    }

    // ── ExecutionRecord::duration_ms ─────────────────────────────────────────

    #[test]
    fn given_record_with_end_time_when_duration_ms_then_correct_delta_returned() {
        let record = make_record(2_000, Some(2_005), ExecutionOverallStatus::Succeeded);
        assert_eq!(record.duration_ms(), Some(5_000));
    }

    #[test]
    fn given_record_with_no_end_time_when_duration_ms_then_none_returned() {
        let record = make_record(2_000, None, ExecutionOverallStatus::Running);
        assert_eq!(record.duration_ms(), None);
    }

    // ── ExecutionRecord::is_frozen ───────────────────────────────────────────

    #[test]
    fn given_running_record_when_is_frozen_then_false() {
        let record = make_record(0, None, ExecutionOverallStatus::Running);
        assert!(!record.is_frozen());
    }

    #[test]
    fn given_succeeded_record_when_is_frozen_then_true() {
        let record = make_record(0, Some(1), ExecutionOverallStatus::Succeeded);
        assert!(record.is_frozen());
    }

    #[test]
    fn given_failed_record_when_is_frozen_then_true() {
        let record = make_record(0, Some(1), ExecutionOverallStatus::Failed);
        assert!(record.is_frozen());
    }

    #[test]
    fn given_cancelled_record_when_is_frozen_then_true() {
        let record = make_record(0, Some(1), ExecutionOverallStatus::Cancelled);
        assert!(record.is_frozen());
    }

    // ── ExecutionRecord::step_for_node ───────────────────────────────────────

    #[test]
    fn given_record_with_step_when_step_for_node_with_matching_id_then_step_returned() {
        let node_id = NodeId::new();
        let step = make_step(Some(0), Some(1));
        let record = ExecutionRecord {
            id: ExecutionRecordId::new(),
            workflow_name: WorkflowName::new("wf"),
            status: ExecutionOverallStatus::Succeeded,
            start_time: utc(0),
            end_time: Some(utc(1)),
            steps: vec![(node_id, step.clone())],
            steps_completed: StepCount::zero().increment(),
            steps_failed: StepCount::zero(),
        };

        let found = record.step_for_node(node_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().step_name.as_str(), "test-step");
    }

    #[test]
    fn given_record_with_step_when_step_for_node_with_unknown_id_then_none_returned() {
        let node_id = NodeId::new();
        let other_id = NodeId::new();
        let step = make_step(Some(0), Some(1));
        let record = ExecutionRecord {
            id: ExecutionRecordId::new(),
            workflow_name: WorkflowName::new("wf"),
            status: ExecutionOverallStatus::Succeeded,
            start_time: utc(0),
            end_time: Some(utc(1)),
            steps: vec![(node_id, step)],
            steps_completed: StepCount::zero().increment(),
            steps_failed: StepCount::zero(),
        };

        assert!(record.step_for_node(other_id).is_none());
    }

    #[test]
    fn given_empty_record_when_step_for_node_then_none_returned() {
        let record = make_record(0, Some(1), ExecutionOverallStatus::Succeeded);
        assert!(record.step_for_node(NodeId::new()).is_none());
    }

    // ── ExecutionOverallStatus::is_terminal ──────────────────────────────────

    #[test]
    fn given_running_status_when_is_terminal_then_false() {
        assert!(!ExecutionOverallStatus::Running.is_terminal());
    }

    #[test]
    fn given_succeeded_status_when_is_terminal_then_true() {
        assert!(ExecutionOverallStatus::Succeeded.is_terminal());
    }

    #[test]
    fn given_failed_status_when_is_terminal_then_true() {
        assert!(ExecutionOverallStatus::Failed.is_terminal());
    }

    #[test]
    fn given_cancelled_status_when_is_terminal_then_true() {
        assert!(ExecutionOverallStatus::Cancelled.is_terminal());
    }

    // ── Serde round-trips ────────────────────────────────────────────────────

    #[test]
    fn step_output_success_roundtrips_through_json() {
        let output = StepOutput::Success(serde_json::json!({"key": 42}));
        let json = serde_json::to_string(&output).unwrap();
        let back: StepOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, back);
    }

    #[test]
    fn step_output_failure_roundtrips_through_json() {
        let output = StepOutput::Failure {
            error: ExecutionError::new("oops"),
            stack_trace: Some("at line 1".to_string()),
        };
        let json = serde_json::to_string(&output).unwrap();
        let back: StepOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, back);
    }

    #[test]
    fn step_output_pending_roundtrips_through_json() {
        let output = StepOutput::Pending;
        let json = serde_json::to_string(&output).unwrap();
        let back: StepOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, back);
    }

    #[test]
    fn execution_overall_status_roundtrips_through_json() {
        let statuses = [
            ExecutionOverallStatus::Running,
            ExecutionOverallStatus::Succeeded,
            ExecutionOverallStatus::Failed,
            ExecutionOverallStatus::Cancelled,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let back: ExecutionOverallStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    // ── from_run_record ──────────────────────────────────────────────────────

    #[test]
    fn given_successful_run_record_when_converting_then_execution_record_reflects_success() {
        use super::{from_run_record, NodeId};
        use crate::graph::RunRecord;
        use std::collections::HashMap;

        let node_id = NodeId::new();
        let mut results = HashMap::new();
        results.insert(node_id, serde_json::json!({"ok": true}));

        let run = RunRecord {
            id: Uuid::new_v4(),
            timestamp: utc(1),
            results,
            success: true,
            restate_invocation_id: None,
        };

        let record = from_run_record(&run);

        assert_eq!(record.id.as_uuid(), run.id);
        assert_eq!(record.status, ExecutionOverallStatus::Succeeded);
        assert_eq!(record.steps_completed.get(), 1);
        assert_eq!(record.steps_failed.get(), 0);
        assert_eq!(record.start_time, utc(1));
        assert!(record.end_time.is_none());
        assert_eq!(record.steps.len(), 1);
        assert_eq!(record.steps[0].0, node_id);
    }

    #[test]
    fn given_failed_run_record_when_converting_then_execution_record_reflects_failure() {
        use super::{from_run_record, NodeId};
        use crate::graph::RunRecord;
        use std::collections::HashMap;

        let node_id = NodeId::new();
        let mut results = HashMap::new();
        results.insert(node_id, serde_json::json!(null));

        let run = RunRecord {
            id: Uuid::new_v4(),
            timestamp: utc(2_000),
            results,
            success: false,
        };

        let record = from_run_record(&run);

        assert_eq!(record.status, ExecutionOverallStatus::Failed);
        assert_eq!(record.steps_completed.get(), 1);
        assert_eq!(record.steps_failed.get(), 1);
    }

    #[test]
    fn given_empty_run_record_when_converting_then_steps_are_empty() {
        use super::from_run_record;
        use crate::graph::RunRecord;
        use std::collections::HashMap;

        let run = RunRecord {
            id: Uuid::new_v4(),
            timestamp: utc(0),
            results: HashMap::new(),
            success: true,
        };

        let record = from_run_record(&run);
        assert!(record.steps.is_empty());
        assert_eq!(record.steps_completed.get(), 0);
        assert_eq!(record.steps_failed.get(), 0);
    }
}
