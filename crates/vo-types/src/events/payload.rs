//! Event payload types and parsing.

use crate::events::error::Error;
use crate::events::MAX_SUPPORTED_VERSION;
use crate::payload_parser::{optional_u64, require_string, require_string_field, require_u64};

#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    WorkflowStarted {
        workflow_id: String,
        dag_topology: serde_json::Value,
        binary_hash: String,
    },
    WorkflowCompleted {
        workflow_id: String,
        completion_time_ms: u64,
    },
    WorkflowFailed {
        workflow_id: String,
        failure_reason: String,
    },
    WorkflowCancelled {
        workflow_id: String,
        cancelled_by: String,
    },
    StepScheduled {
        workflow_id: String,
        step_id: String,
        attempt: u32,
        execution_id: String,
    },
    StepStarted {
        workflow_id: String,
        step_id: String,
        started_at_ms: u64,
    },
    StepCompleted {
        workflow_id: String,
        step_id: String,
        completed_at_ms: u64,
        output: serde_json::Value,
    },
    StepFailed {
        workflow_id: String,
        step_id: String,
        failure_reason: String,
        attempt: u32,
    },
    TimerSet {
        workflow_id: String,
        timer_id: String,
        fire_at_ms: u64,
    },
    TimerFired {
        workflow_id: String,
        timer_id: String,
        fired_at_ms: u64,
    },
    CancelRequested {
        workflow_id: String,
        requested_by: String,
    },
    InstanceResumed {
        workflow_id: String,
        resumed_at_ms: u64,
    },
    /// Emitted when a workflow continues-as-new to a new epoch (ADR-038).
    ContinuedAsNew {
        workflow_id: String,
        lineage_id: String,
        old_epoch: u64,
        new_epoch: u64,
    },
}

impl EventPayload {
    /// Decode an `EventPayload` from a JSON value.
    ///
    /// # Errors
    ///
    /// Returns payload-level errors if the JSON is not a valid object,
    /// missing required fields, or has an unsupported version/type.
    pub fn try_from_json(payload_json: &serde_json::Value) -> Result<Self, Error> {
        let obj = payload_json
            .as_object()
            .ok_or(Error::InvalidPayloadFormat)?;

        let payload_type = require_string(obj, "type")?;
        let payload_version_u64 = optional_u64(obj, "version", 0);
        let payload_version = u8::try_from(payload_version_u64).map_err(|_| {
            Error::InvalidPayloadField("version exceeds maximum supported value".to_string())
        })?;
        if payload_version > MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedPayloadVersion(payload_version));
        }

        match payload_type.as_str() {
            "WorkflowStarted" => Ok(EventPayload::WorkflowStarted {
                workflow_id: require_string_field(obj, "workflow_id")?,
                dag_topology: obj
                    .get("dag_topology")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                binary_hash: require_string(obj, "binary_hash")?,
            }),
            "WorkflowCompleted" => Ok(EventPayload::WorkflowCompleted {
                workflow_id: require_string_field(obj, "workflow_id")?,
                completion_time_ms: require_u64(obj, "completion_time_ms")?,
            }),
            "WorkflowFailed" => Ok(EventPayload::WorkflowFailed {
                workflow_id: require_string_field(obj, "workflow_id")?,
                failure_reason: require_string(obj, "failure_reason")?,
            }),
            "WorkflowCancelled" => Ok(EventPayload::WorkflowCancelled {
                workflow_id: require_string_field(obj, "workflow_id")?,
                cancelled_by: require_string(obj, "cancelled_by")?,
            }),
            "StepScheduled" => Ok(EventPayload::StepScheduled {
                workflow_id: require_string_field(obj, "workflow_id")?,
                step_id: require_string(obj, "step_id")?,
                #[allow(clippy::cast_possible_truncation)]
                attempt: require_u64(obj, "attempt")? as u32,
                execution_id: require_string(obj, "execution_id")?,
            }),
            "StepStarted" => Ok(EventPayload::StepStarted {
                workflow_id: require_string_field(obj, "workflow_id")?,
                step_id: require_string(obj, "step_id")?,
                started_at_ms: require_u64(obj, "started_at_ms")?,
            }),
            "StepCompleted" => Ok(EventPayload::StepCompleted {
                workflow_id: require_string_field(obj, "workflow_id")?,
                step_id: require_string(obj, "step_id")?,
                completed_at_ms: require_u64(obj, "completed_at_ms")?,
                output: obj
                    .get("output")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            }),
            "StepFailed" => Ok(EventPayload::StepFailed {
                workflow_id: require_string_field(obj, "workflow_id")?,
                step_id: require_string(obj, "step_id")?,
                failure_reason: require_string(obj, "failure_reason")?,
                #[allow(clippy::cast_possible_truncation)]
                attempt: require_u64(obj, "attempt")? as u32,
            }),
            "TimerSet" => Ok(EventPayload::TimerSet {
                workflow_id: require_string_field(obj, "workflow_id")?,
                timer_id: require_string(obj, "timer_id")?,
                fire_at_ms: require_u64(obj, "fire_at_ms")?,
            }),
            "TimerFired" => Ok(EventPayload::TimerFired {
                workflow_id: require_string_field(obj, "workflow_id")?,
                timer_id: require_string(obj, "timer_id")?,
                fired_at_ms: require_u64(obj, "fired_at_ms")?,
            }),
            "CancelRequested" => Ok(EventPayload::CancelRequested {
                workflow_id: require_string_field(obj, "workflow_id")?,
                requested_by: require_string(obj, "requested_by")?,
            }),
            "InstanceResumed" => Ok(EventPayload::InstanceResumed {
                workflow_id: require_string_field(obj, "workflow_id")?,
                resumed_at_ms: require_u64(obj, "resumed_at_ms")?,
            }),
            "ContinuedAsNew" => Ok(EventPayload::ContinuedAsNew {
                workflow_id: require_string_field(obj, "workflow_id")?,
                lineage_id: require_string(obj, "lineage_id")?,
                old_epoch: require_u64(obj, "old_epoch")?,
                new_epoch: require_u64(obj, "new_epoch")?,
            }),
            other => Err(Error::UnknownPayloadType(other.to_string())),
        }
    }

    #[must_use]
    pub fn is_version_supported(version: u8) -> bool {
        version <= MAX_SUPPORTED_VERSION
    }
}
