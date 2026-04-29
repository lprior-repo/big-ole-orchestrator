#[cfg(test)]
mod ambiguity_detection_tests {
    use super::*;

    #[tokio::test]
    async fn unknown_state_connector_returns_ambiguous_on_commit() {
        let c = UnknownStateConnector::new();
        let pe = c
            .prepare(serde_json::json!({}), "fx-unknown".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert_eq!(outcome, CommitOutcome::Ambiguous);
    }

    #[tokio::test]
    async fn unknown_state_reconcile_initially_still_ambiguous() {
        let c = UnknownStateConnector::new();
        let reconcile = c.reconcile("fx-unknown").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::StillAmbiguous);
    }

    #[tokio::test]
    async fn unknown_state_resolves_after_server_recovers() {
        let c = UnknownStateConnector::new();
        c.reconcile_unknown.store(true, Ordering::SeqCst);
        let r1 = c.reconcile("fx-resolve").await.unwrap();
        assert_eq!(r1, ReconcileOutcome::StillAmbiguous);

        c.reconcile_unknown.store(false, Ordering::SeqCst);
        let r2 = c.reconcile("fx-resolve").await.unwrap();
        assert_eq!(r2, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn timeout_connector_transitions_from_committed_to_ambiguous() {
        let c = TimeoutOnCommitConnector::new(2);
        for i in 0..2 {
            let pe = c
                .prepare(serde_json::json!({}), format!("fx-ok-{}", i), i as u64 + 1)
                .await
                .unwrap();
            let outcome = c.commit(pe).await.unwrap();
            assert!(matches!(outcome, CommitOutcome::Committed { .. }));
        }

        let pe3 = c
            .prepare(serde_json::json!({}), "fx-amb".into(), 3)
            .await
            .unwrap();
        let outcome3 = c.commit(pe3).await.unwrap();
        assert_eq!(outcome3, CommitOutcome::Ambiguous);
    }

    #[tokio::test]
    async fn reconcile_action_mapping_for_all_outcomes() {
        use vo_types::ReconcileAction;

        let committed = ReconcileOutcome::Committed {
            receipt: "r".into(),
        };
        assert_eq!(ReconcileAction::from(committed), ReconcileAction::Commit);

        let not_committed = ReconcileOutcome::NotCommitted;
        assert_eq!(
            ReconcileAction::from(not_committed),
            ReconcileAction::Rollback
        );

        let still_ambiguous = ReconcileOutcome::StillAmbiguous;
        assert_eq!(
            ReconcileAction::from(still_ambiguous),
            ReconcileAction::Retry
        );
    }

    #[tokio::test]
    async fn commit_outcome_equality() {
        assert_eq!(
            CommitOutcome::Committed {
                receipt: "a".into()
            },
            CommitOutcome::Committed {
                receipt: "a".into()
            }
        );
        assert_ne!(
            CommitOutcome::Committed {
                receipt: "a".into()
            },
            CommitOutcome::Committed {
                receipt: "b".into()
            }
        );
        assert_eq!(CommitOutcome::Failed, CommitOutcome::Failed);
        assert_eq!(CommitOutcome::Ambiguous, CommitOutcome::Ambiguous);
    }

    #[tokio::test]
    async fn reconcile_outcome_equality() {
        assert_eq!(
            ReconcileOutcome::Committed {
                receipt: "r".into()
            },
            ReconcileOutcome::Committed {
                receipt: "r".into()
            }
        );
        assert_eq!(
            ReconcileOutcome::NotCommitted,
            ReconcileOutcome::NotCommitted
        );
        assert_eq!(
            ReconcileOutcome::StillAmbiguous,
            ReconcileOutcome::StillAmbiguous
        );
    }

    #[tokio::test]
    async fn error_classification_retryable_vs_terminal() {
        let retryable = ConnectorError::retryable("timeout");
        assert!(retryable.is_retryable());

        let terminal = ConnectorError::terminal("bad request");
        assert!(!terminal.is_retryable());

        let no_support = ConnectorError::compensation_not_supported("http");
        assert!(no_support.is_retryable());
    }

    #[tokio::test]
    async fn http_connector_ambiguous_maps_to_retry_action() {
        use vo_types::ReconcileAction;
        let c = HttpConnector::new("https://api.example.com");
        let reconcile = c.reconcile("any").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::StillAmbiguous);
        let action = ReconcileAction::from(reconcile);
        assert_eq!(action, ReconcileAction::Retry);
    }
}
