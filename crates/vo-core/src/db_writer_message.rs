//! DbWriterMessage enum for atomic control-plane transitions.
//!
//! Per ADR-016: DbWriterActor uses fjall::Batch for every control-plane
//! transition. All events are sent to DbWriterActor for batch commit.
//!
//! Per ADR-029: Execution leases with monotonic fence tokens for
//! (instance_id, step_id) pairs. All completion paths carry the fence.

use serde::{Deserialize, Serialize};
use vo_types::{FenceToken, IdempotencyKey, InstanceId, SequenceNumber, StepId};

/// Messages sent to `DbWriterActor` for atomic batch commits.
///
/// Per ADR-016: every control-plane transition must atomically update
/// all touched partitions in the same batch.
///
/// Per ADR-029: all completion paths carry fence tokens for lease validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum DbWriterMessage {
    /// Append an event to the event log with idempotency protection.
    AppendEvent {
        instance_id: InstanceId,
        sequence_number: SequenceNumber,
        idempotency_key: IdempotencyKey,
    },
    /// Record the current instance status in the instances index partition.
    RecordInstanceStatus {
        instance_id: InstanceId,
        status_byte: u8,
    },
    /// Acquire an execution lease for a (instance_id, step_id) pair (ADR-029).
    AcquireLease {
        instance_id: InstanceId,
        step_id: StepId,
        fence: FenceToken,
    },
    /// Release an execution lease for a (instance_id, step_id) pair (ADR-029).
    ReleaseLease {
        instance_id: InstanceId,
        step_id: StepId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{FenceToken, StepId};

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> SequenceNumber {
        SequenceNumber::new_unchecked(1)
    }

    fn valid_idempotency_key() -> IdempotencyKey {
        IdempotencyKey::parse("key-1").expect("valid key")
    }

    fn valid_step_id() -> StepId {
        StepId::parse("step-1").expect("valid step id")
    }

    fn valid_fence_token() -> FenceToken {
        FenceToken::new(1).expect("valid fence token")
    }

    #[test]
    fn append_event_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"append_event\""),
            "expected snake_case tag 'append_event', got: {json}"
        );
    }

    #[test]
    fn record_instance_status_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::RecordInstanceStatus {
            instance_id: valid_instance_id(),
            status_byte: 0x01,
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"record_instance_status\""),
            "expected snake_case tag 'record_instance_status', got: {json}"
        );
    }

    #[test]
    fn acquire_lease_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::AcquireLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
            fence: valid_fence_token(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"acquire_lease\""),
            "expected snake_case tag 'acquire_lease', got: {json}"
        );
    }

    #[test]
    fn release_lease_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::ReleaseLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"release_lease\""),
            "expected snake_case tag 'release_lease', got: {json}"
        );
    }
}
