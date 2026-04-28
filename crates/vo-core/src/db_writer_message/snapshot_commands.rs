//! Snapshot domain commands: TakeSnapshot, SnapshotData.

use serde::{Deserialize, Serialize};
use vo_types::{SequenceNumber, MAX_SUPPORTED_SCHEMA_VERSION};

fn default_schema_version() -> u16 {
    MAX_SUPPORTED_SCHEMA_VERSION
}

/// Snapshot data for instance state hibernation.
///
/// Invariant: `state_bytes` must be non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotData {
    sequence_number: SequenceNumber,
    #[serde(default = "default_schema_version")]
    schema_version: u16,
    state_bytes: Vec<u8>,
}

#[allow(dead_code)]
impl SnapshotData {
    /// Create a new `SnapshotData`.
    ///
    /// Returns `None` if `state_bytes` is empty (invariant: state must be non-empty).
    #[must_use]
    pub fn new(
        sequence_number: SequenceNumber,
        schema_version: u16,
        state_bytes: Vec<u8>,
    ) -> Option<Self> {
        if state_bytes.is_empty() {
            return None;
        }
        Some(Self {
            sequence_number,
            schema_version,
            state_bytes,
        })
    }

    #[must_use]
    pub fn sequence_number(&self) -> SequenceNumber {
        self.sequence_number
    }

    #[must_use]
    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub fn state_bytes(&self) -> &[u8] {
        &self.state_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::{SnapshotData, MAX_SUPPORTED_SCHEMA_VERSION};
    use crate::db_writer_message::message::DbWriterMessage;
    use vo_types::{InstanceId, SequenceNumber};

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> SequenceNumber {
        SequenceNumber::new_unchecked(1)
    }

    fn valid_snapshot_data() -> SnapshotData {
        SnapshotData::new(
            valid_sequence(),
            MAX_SUPPORTED_SCHEMA_VERSION,
            vec![0x01, 0x02, 0x03],
        )
        .expect("valid snapshot data")
    }

    // ========================================================================
    // B08: snake_case tag serialization
    // ========================================================================

    #[test]
    fn take_snapshot_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::TakeSnapshot {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            snapshot_data: valid_snapshot_data(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"take_snapshot\""),
            "expected snake_case tag 'take_snapshot', got: {json}"
        );
    }

    // ========================================================================
    // B17: Serde round-trip (DbWriterMessage)
    // ========================================================================

    #[test]
    fn take_snapshot_round_trips_through_serde_json() {
        let msg = DbWriterMessage::TakeSnapshot {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            snapshot_data: valid_snapshot_data(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    // ========================================================================
    // B22: Serde round-trip (SnapshotData)
    // ========================================================================

    #[test]
    fn snapshot_data_round_trips_through_serde_json() {
        let sd = valid_snapshot_data();
        let json = serde_json::to_string(&sd).expect("serialize");
        let recovered: SnapshotData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sd, recovered);
    }

    // ========================================================================
    // B42: SnapshotData PartialEq
    // ========================================================================

    #[test]
    fn snapshot_data_different_state_bytes_compare_unequal() {
        let sd1 = SnapshotData::new(valid_sequence(), MAX_SUPPORTED_SCHEMA_VERSION, vec![0x01])
            .expect("valid snapshot data");
        let sd2 = SnapshotData::new(valid_sequence(), MAX_SUPPORTED_SCHEMA_VERSION, vec![0x02])
            .expect("valid snapshot data");
        assert_ne!(sd1, sd2);
    }

    // ========================================================================
    // SnapshotData invariants
    // ========================================================================

    #[test]
    fn snapshot_data_new_returns_some_when_state_bytes_non_empty() {
        let snap = SnapshotData::new(
            valid_sequence(),
            MAX_SUPPORTED_SCHEMA_VERSION,
            vec![0x01, 0x02, 0x03],
        );
        assert!(snap.is_some());
    }

    #[test]
    fn snapshot_data_new_returns_none_when_state_bytes_empty() {
        let snap = SnapshotData::new(valid_sequence(), MAX_SUPPORTED_SCHEMA_VERSION, vec![]);
        assert_eq!(snap, None);
    }
}
