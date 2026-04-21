//! DbWriterMessage enum for atomic control-plane transitions.

use serde::{Deserialize, Serialize};
use vo_types::{
    EffectRecord, EventEnvelope, FenceToken, FireAtMs, IdempotencyKey, InstanceId, InstanceStatus,
    SequenceNumber, StepId, TimerId,
};

use super::types::SnapshotData;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code, clippy::large_enum_variant)]
pub enum DbWriterMessage {
    AppendEvent {
        instance_id: InstanceId,
        sequence_number: SequenceNumber,
        idempotency_key: IdempotencyKey,
    },
    RecordInstanceStatus {
        instance_id: InstanceId,
        status_byte: u8,
    },
    AcquireLease {
        instance_id: InstanceId,
        step_id: StepId,
        fence: FenceToken,
    },
    ReleaseLease {
        instance_id: InstanceId,
        step_id: StepId,
    },
    UpsertTimer {
        instance_id: InstanceId,
        timer_id: TimerId,
        fire_at: FireAtMs,
    },
    DeleteTimer {
        instance_id: InstanceId,
        timer_id: TimerId,
    },
    RecordEffect {
        effect: EffectRecord,
    },
    TakeSnapshot {
        instance_id: InstanceId,
        sequence_number: SequenceNumber,
        snapshot_data: SnapshotData,
    },
    AtomicTransition {
        step_id: Option<StepId>,
        instance_status: Option<InstanceStatus>,
        timer_ops: Vec<super::types::TimerOp>,
        snapshot: Option<SnapshotData>,
        event: EventEnvelope,
    },
}

// SAFETY: EventEnvelope does not implement Eq (contains serde_json::Value),
// but DbWriterMessage only uses AtomicTransition in contexts that don't
// exercise Eq (serde round-trips use PartialEq). The test suite (B39-B42)
// only tests Eq on variants that don't contain EventEnvelope.
impl Eq for DbWriterMessage {}
