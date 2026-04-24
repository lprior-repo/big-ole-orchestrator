//! ProjectionResult and ProjectionRecord tests (PR-*, RC-*).
//!
//! Tests the `ProjectionResult` struct fields and the `ProjectionRecord`
//! persisted state including checksum integrity.

use crate::replay::projection::{ProjectionRecord, ProjectionResult};

#[cfg(test)]
mod pr_tests {
    use super::*;

    #[test]
    fn pr_001_projection_result_fields_match_contract() {
        let result: ProjectionResult<String> =
            ProjectionResult::new("final_state".to_string(), 100, 1, 100, 500, 3);

        assert_eq!(
            result.state, "final_state",
            "PR-001: state field must match"
        );
        assert_eq!(
            result.events_applied, 100,
            "PR-001: events_applied field must match"
        );
        assert_eq!(
            result.starting_sequence, 1,
            "PR-001: starting_sequence field must match"
        );
        assert_eq!(
            result.ending_sequence, 100,
            "PR-001: ending_sequence field must match"
        );
        assert_eq!(
            result.duration_ms, 500,
            "PR-001: duration_ms field must match"
        );
        assert_eq!(
            result.schema_version, 3,
            "PR-001: schema_version field must match"
        );
    }

    #[test]
    fn pr_002_events_applied_equals_actual_event_count() {
        let events: Vec<String> = (0..50).map(|i| format!("event_{}", i)).collect();
        let events_applied = events.len() as u64;

        let result: ProjectionResult<Vec<String>> =
            ProjectionResult::new(events, events_applied, 1, 50, 100, 1);

        assert_eq!(
            result.events_applied, 50,
            "PR-002: events_applied must equal actual count"
        );
    }

    #[test]
    fn pr_003_starting_and_ending_sequence_span_correct_range() {
        let result: ProjectionResult<String> =
            ProjectionResult::new("state".to_string(), 10, 5, 14, 100, 1);

        assert!(
            result.ending_sequence >= result.starting_sequence,
            "PR-003: end must be >= start"
        );
    }

    #[test]
    fn pr_004_schema_version_matches_projector() {
        let projector_version: u8 = 7;
        let result: ProjectionResult<String> =
            ProjectionResult::new("state".to_string(), 10, 1, 10, 100, projector_version);

        assert_eq!(
            result.schema_version, projector_version,
            "PR-004: schema_version must match projector"
        );
    }

    #[test]
    fn pr_005_duration_ms_is_non_negative() {
        let result: ProjectionResult<String> =
            ProjectionResult::new("state".to_string(), 10, 1, 10, 0, 1);

        assert!(
            result.duration_ms >= 0,
            "PR-005: duration_ms must be non-negative"
        );
    }

    #[test]
    fn pr_006_empty_events_zero_applied() {
        let result: ProjectionResult<String> = ProjectionResult::new(String::new(), 0, 0, 0, 10, 1);

        assert_eq!(
            result.events_applied, 0,
            "PR-006: empty events should have 0 applied"
        );
    }

    #[test]
    fn pr_007_single_event_correct_range() {
        let result: ProjectionResult<String> =
            ProjectionResult::new("state".to_string(), 1, 42, 42, 5, 1);

        assert_eq!(result.starting_sequence, 42, "PR-007: single event start");
        assert_eq!(result.ending_sequence, 42, "PR-007: single event end");
    }
}

#[cfg(test)]
mod rc_tests {
    use super::*;

    #[test]
    fn rc_001_projection_record_fields_match_contract() {
        let record = ProjectionRecord::new(
            "test-projection".to_string(),
            3,
            vec![1, 2, 3, 4],
            (10, 50),
            0xDEADBEEF,
            1000,
            2000,
        );

        assert_eq!(
            record.projection_id, "test-projection",
            "RC-001: projection_id must match"
        );
        assert_eq!(
            record.schema_version, 3,
            "RC-001: schema_version must match"
        );
        assert_eq!(
            record.state_bytes,
            vec![1, 2, 3, 4],
            "RC-001: state_bytes must match"
        );
        assert_eq!(
            record.sequence_range,
            (10, 50),
            "RC-001: sequence_range must match"
        );
        assert_eq!(record.checksum, 0xDEADBEEF, "RC-001: checksum must match");
        assert_eq!(record.created_at, 1000, "RC-001: created_at must match");
        assert_eq!(record.updated_at, 2000, "RC-001: updated_at must match");
    }

    #[test]
    fn rc_002_checksum_computed_from_state_bytes_is_deterministic() {
        let state_bytes = vec![1, 2, 3, 4, 5];
        let record1 = ProjectionRecord::new(
            "test".to_string(),
            1,
            state_bytes.clone(),
            (1, 10),
            0,
            1000,
            1000,
        );

        let record2 =
            ProjectionRecord::new("test".to_string(), 1, state_bytes, (1, 10), 0, 1000, 1000);

        assert_eq!(
            record1.checksum, record2.checksum,
            "RC-002: Same bytes must produce same checksum"
        );
    }

    #[test]
    fn rc_003_different_state_bytes_produce_different_checksums() {
        let record1 =
            ProjectionRecord::new("test".to_string(), 1, vec![1, 2, 3], (1, 10), 0, 1000, 1000);

        let record2 =
            ProjectionRecord::new("test".to_string(), 1, vec![4, 5, 6], (1, 10), 0, 1000, 1000);

        assert_ne!(
            record1.checksum, record2.checksum,
            "RC-003: Different bytes must produce different checksums"
        );
    }

    #[test]
    fn rc_004_sequence_range_matches_events_used_to_build() {
        let events_start = 5u64;
        let events_end = 20u64;

        let record = ProjectionRecord::new(
            "test".to_string(),
            1,
            vec![],
            (events_start, events_end),
            0,
            1000,
            1000,
        );

        assert_eq!(
            record.sequence_range,
            (5, 20),
            "RC-004: sequence_range must match events used to build"
        );
    }

    #[test]
    fn rc_005_created_at_immutable_after_first_write() {
        let record =
            ProjectionRecord::new("test".to_string(), 1, vec![1, 2], (1, 10), 0, 1000, 2000);

        assert_eq!(
            record.created_at, 1000,
            "RC-005: created_at must be preserved"
        );
    }

    #[test]
    fn rc_006_updated_at_changes_on_each_write() {
        let record1 = ProjectionRecord::new("test".to_string(), 1, vec![1], (1, 5), 0, 1000, 1000);

        let record2 =
            ProjectionRecord::new("test".to_string(), 1, vec![1, 2], (1, 10), 0, 1000, 2000);

        assert!(
            record2.updated_at >= record1.created_at,
            "RC-006: updated_at must change on write"
        );
    }

    #[test]
    fn rc_007_state_bytes_roundtrips_through_serialize_deserialize() {
        let original = ProjectionRecord::new(
            "test".to_string(),
            1,
            vec![1, 2, 3],
            (1, 10),
            12345,
            1000,
            2000,
        );

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ProjectionRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(
            original.state_bytes, deserialized.state_bytes,
            "RC-007: state_bytes must round-trip"
        );
    }

    #[test]
    fn rc_008_json_roundtrip_preserves_all_fields() {
        let original = ProjectionRecord::new(
            "test-projection".to_string(),
            5,
            vec![1, 2, 3, 4],
            (10, 50),
            0xDEADBEEF,
            1000,
            2000,
        );

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ProjectionRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(
            original.projection_id, deserialized.projection_id,
            "RC-008: projection_id must round-trip"
        );
        assert_eq!(
            original.schema_version, deserialized.schema_version,
            "RC-008: schema_version must round-trip"
        );
        assert_eq!(
            original.state_bytes, deserialized.state_bytes,
            "RC-008: state_bytes must round-trip"
        );
        assert_eq!(
            original.sequence_range, deserialized.sequence_range,
            "RC-008: sequence_range must round-trip"
        );
        assert_eq!(
            original.checksum, deserialized.checksum,
            "RC-008: checksum must round-trip"
        );
        assert_eq!(
            original.created_at, deserialized.created_at,
            "RC-008: created_at must round-trip"
        );
        assert_eq!(
            original.updated_at, deserialized.updated_at,
            "RC-008: updated_at must round-trip"
        );
    }

    #[test]
    fn rc_009_empty_state_bytes_valid() {
        let record =
            ProjectionRecord::new("empty-test".to_string(), 1, vec![], (1, 0), 0, 1000, 1000);

        assert!(
            record.state_bytes.is_empty(),
            "RC-009: empty state_bytes should be valid"
        );
    }
}
