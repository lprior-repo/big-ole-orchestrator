//! Core connector types (ADR-041).

use serde::{Deserialize, Serialize};

/// A prepared effect ready for commit (ADR-041 §2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedEffect {
    pub effect_id: String,
    pub payload: serde_json::Value,
    pub fence: u64,
}

/// Outcome of a commit or compensate operation (ADR-041 §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitOutcome {
    Committed { receipt: String },
    Failed,
    Ambiguous,
}

/// Outcome of a reconciliation operation (ADR-041 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileOutcome {
    Committed { receipt: String },
    NotCommitted,
    StillAmbiguous,
}

impl From<ReconcileOutcome> for vo_types::ReconcileAction {
    fn from(outcome: ReconcileOutcome) -> Self {
        match outcome {
            ReconcileOutcome::Committed { .. } => vo_types::ReconcileAction::Commit,
            ReconcileOutcome::NotCommitted => vo_types::ReconcileAction::Rollback,
            ReconcileOutcome::StillAmbiguous => vo_types::ReconcileAction::Retry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_prepared_effect_serialization() {
        let pe = PreparedEffect {
            effect_id: "fx-123".to_string(),
            payload: json!({"key": "value"}),
            fence: 42,
        };

        let serialized = serde_json::to_string(&pe).unwrap();
        let deserialized: PreparedEffect = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.effect_id, pe.effect_id);
        assert_eq!(deserialized.fence, pe.fence);
        assert_eq!(deserialized.payload, pe.payload);
    }

    #[test]
    fn test_prepared_effect_deserialization() {
        let json_str = r#"{"effect_id":"fx-456","payload":{"method":"POST"},"fence":10}"#;
        let pe: PreparedEffect = serde_json::from_str(json_str).unwrap();

        assert_eq!(pe.effect_id, "fx-456");
        assert_eq!(pe.fence, 10);
    }

    #[test]
    fn test_prepared_effect_empty_payload() {
        let pe = PreparedEffect {
            effect_id: "fx-empty".to_string(),
            payload: json!({}),
            fence: 0,
        };

        let serialized = serde_json::to_string(&pe).unwrap();
        let deserialized: PreparedEffect = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.payload, json!({}));
    }

    #[test]
    fn test_commit_outcome_committed() {
        let outcome = CommitOutcome::Committed {
            receipt: "receipt-123".to_string(),
        };
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        if let CommitOutcome::Committed { receipt } = outcome {
            assert_eq!(receipt, "receipt-123");
        }
    }

    #[test]
    fn test_commit_outcome_failed() {
        let outcome = CommitOutcome::Failed;
        assert!(matches!(outcome, CommitOutcome::Failed));
    }

    #[test]
    fn test_commit_outcome_ambiguous() {
        let outcome = CommitOutcome::Ambiguous;
        assert!(matches!(outcome, CommitOutcome::Ambiguous));
    }

    #[test]
    fn test_commit_outcome_debug() {
        let committed = CommitOutcome::Committed {
            receipt: "r".to_string(),
        };
        assert!(format!("{:?}", committed).contains("Committed"));

        let failed = CommitOutcome::Failed;
        assert!(format!("{:?}", failed).contains("Failed"));

        let ambiguous = CommitOutcome::Ambiguous;
        assert!(format!("{:?}", ambiguous).contains("Ambiguous"));
    }

    #[test]
    fn test_reconcile_outcome_committed() {
        let outcome = ReconcileOutcome::Committed {
            receipt: "r".to_string(),
        };
        assert!(matches!(outcome, ReconcileOutcome::Committed { .. }));
    }

    #[test]
    fn test_reconcile_outcome_not_committed() {
        let outcome = ReconcileOutcome::NotCommitted;
        assert!(matches!(outcome, ReconcileOutcome::NotCommitted));
    }

    #[test]
    fn test_reconcile_outcome_still_ambiguous() {
        let outcome = ReconcileOutcome::StillAmbiguous;
        assert!(matches!(outcome, ReconcileOutcome::StillAmbiguous));
    }

    #[test]
    fn test_reconcile_outcome_debug() {
        let committed = ReconcileOutcome::Committed {
            receipt: "r".to_string(),
        };
        assert!(format!("{:?}", committed).contains("Committed"));

        let not_committed = ReconcileOutcome::NotCommitted;
        assert!(format!("{:?}", not_committed).contains("NotCommitted"));

        let ambiguous = ReconcileOutcome::StillAmbiguous;
        assert!(format!("{:?}", ambiguous).contains("StillAmbiguous"));
    }

    #[test]
    fn test_reconcile_outcome_to_reconcile_action_committed() {
        let outcome = ReconcileOutcome::Committed {
            receipt: "r".to_string(),
        };
        let action: vo_types::ReconcileAction = outcome.into();
        assert_eq!(action, vo_types::ReconcileAction::Commit);
    }

    #[test]
    fn test_reconcile_outcome_to_reconcile_action_not_committed() {
        let outcome = ReconcileOutcome::NotCommitted;
        let action: vo_types::ReconcileAction = outcome.into();
        assert_eq!(action, vo_types::ReconcileAction::Rollback);
    }

    #[test]
    fn test_reconcile_outcome_to_reconcile_action_still_ambiguous() {
        let outcome = ReconcileOutcome::StillAmbiguous;
        let action: vo_types::ReconcileAction = outcome.into();
        assert_eq!(action, vo_types::ReconcileAction::Retry);
    }

    #[test]
    fn test_prepared_effect_with_nested_payload() {
        let nested = json!({
            "request": {
                "method": "POST",
                "path": "/api/charges",
                "body": {
                    "amount": 1000,
                    "currency": "usd",
                    "customer": "cus_123"
                }
            }
        });

        let pe = PreparedEffect {
            effect_id: "fx-nested".to_string(),
            payload: nested,
            fence: 999,
        };

        let serialized = serde_json::to_string(&pe).unwrap();
        let deserialized: PreparedEffect = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.effect_id, "fx-nested");
        assert_eq!(deserialized.payload["request"]["body"]["amount"], 1000);
    }

    #[test]
    fn test_prepared_effect_large_payload() {
        let large_payload = json!({
            "data": vec!["item1", "item2", "item3", "item4", "item5"],
            "metadata": {
                "created_at": "2024-01-01T00:00:00Z",
                "version": "1.0",
                "tags": ["tag1", "tag2", "tag3"]
            }
        });

        let pe = PreparedEffect {
            effect_id: "fx-large".to_string(),
            payload: large_payload,
            fence: 100,
        };

        let serialized = serde_json::to_string(&pe).unwrap();
        let deserialized: PreparedEffect = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.effect_id, "fx-large");
        assert_eq!(deserialized.payload["metadata"]["version"], "1.0");
    }

    #[test]
    fn test_prepared_effect_special_characters_in_effect_id() {
        let pe = PreparedEffect {
            effect_id: "fx-123-abc-def-456".to_string(),
            payload: json!({}),
            fence: 1,
        };

        let serialized = serde_json::to_string(&pe).unwrap();
        let deserialized: PreparedEffect = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.effect_id, "fx-123-abc-def-456");
    }

    #[test]
    fn test_commit_outcome_partial_eq() {
        let o1 = CommitOutcome::Committed {
            receipt: "r1".to_string(),
        };
        let o2 = CommitOutcome::Committed {
            receipt: "r1".to_string(),
        };
        let o3 = CommitOutcome::Committed {
            receipt: "r2".to_string(),
        };

        assert_eq!(o1, o2);
        assert_ne!(o1, o3);
    }

    #[test]
    fn test_reconcile_outcome_partial_eq() {
        let o1 = ReconcileOutcome::Committed {
            receipt: "r1".to_string(),
        };
        let o2 = ReconcileOutcome::Committed {
            receipt: "r1".to_string(),
        };
        let o3 = ReconcileOutcome::Committed {
            receipt: "r2".to_string(),
        };

        assert_eq!(o1, o2);
        assert_ne!(o1, o3);
    }

    #[test]
    fn test_reconcile_outcome_different_receipts() {
        let o1 = ReconcileOutcome::Committed {
            receipt: "receipt-a".to_string(),
        };
        let o2 = ReconcileOutcome::Committed {
            receipt: "receipt-b".to_string(),
        };

        assert_ne!(o1, o2);
    }
}
