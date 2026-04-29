//! Timer domain commands: UpsertTimer, DeleteTimer, TimerOp.

use serde::{Deserialize, Serialize};
use vo_types::{FireAtMs, TimerId};

/// Timer operation for atomic timer management.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerOp {
    /// Upsert (insert or update) a timer.
    Upsert {
        timer_id: TimerId,
        fire_at: FireAtMs,
    },
    /// Delete a timer.
    Delete { timer_id: TimerId },
}

#[cfg(test)]
mod tests {
    use super::{FireAtMs, TimerOp};
    use crate::db_writer_message::message::DbWriterMessage;
    use vo_types::{InstanceId, TimerId};

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_timer_id() -> TimerId {
        TimerId::parse("timer-1").expect("valid timer id")
    }

    fn valid_fire_at() -> FireAtMs {
        FireAtMs::try_from(1712200000000u64).expect("valid fire_at")
    }

    // ========================================================================
    // B05, B06: snake_case tag serialization
    // ========================================================================

    #[test]
    fn upsert_timer_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::UpsertTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"upsert_timer\""),
            "expected snake_case tag 'upsert_timer', got: {json}"
        );
    }

    #[test]
    fn delete_timer_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::DeleteTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"delete_timer\""),
            "expected snake_case tag 'delete_timer', got: {json}"
        );
    }

    // ========================================================================
    // B14, B15: Serde round-trip (DbWriterMessage variants)
    // ========================================================================

    #[test]
    fn upsert_timer_round_trips_through_serde_json() {
        let msg = DbWriterMessage::UpsertTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn delete_timer_round_trips_through_serde_json() {
        let msg = DbWriterMessage::DeleteTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    // ========================================================================
    // B20, B21: Serde round-trip (TimerOp)
    // ========================================================================

    #[test]
    fn timer_op_upsert_round_trips_through_serde_json() {
        let op = TimerOp::Upsert {
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        };
        let json = serde_json::to_string(&op).expect("serialize");
        let recovered: TimerOp = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(op, recovered);
    }

    #[test]
    fn timer_op_delete_round_trips_through_serde_json() {
        let op = TimerOp::Delete {
            timer_id: valid_timer_id(),
        };
        let json = serde_json::to_string(&op).expect("serialize");
        let recovered: TimerOp = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(op, recovered);
    }

    // ========================================================================
    // B41: TimerOp PartialEq
    // ========================================================================

    #[test]
    fn timer_op_different_variants_compare_unequal() {
        let op1 = TimerOp::Upsert {
            timer_id: TimerId::parse("t1").expect("valid"),
            fire_at: FireAtMs::try_from(100u64).expect("valid"),
        };
        let op2 = TimerOp::Delete {
            timer_id: TimerId::parse("t1").expect("valid"),
        };
        assert_ne!(op1, op2);
    }
}
