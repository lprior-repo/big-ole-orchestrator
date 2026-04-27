#[cfg(test)]
mod ambiguity_routing_tests {
    use super::*;

    #[tokio::test]
    async fn ambiguous_commit_routes_to_reconcile_not_retry() {
        let c = TimeoutOnCommitConnector::new(1);
        let pe = c
            .prepare(serde_json::json!({}), "fx-timeout".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let pe2 = c
            .prepare(serde_json::json!({}), "fx-timeout-2".into(), 2)
            .await
            .unwrap();
        let outcome2 = c.commit(pe2).await.unwrap();
        assert_eq!(outcome2, CommitOutcome::Ambiguous);

        let reconcile = c.reconcile("fx-timeout-2").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::NotCommitted));
    }

    #[tokio::test]
    async fn reconciliation_after_timeout_resolves_committed() {
        let c = TimeoutOnCommitConnector::new(3);
        let pe = c
            .prepare(serde_json::json!({}), "fx-resolve".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let reconcile = c.reconcile("fx-resolve").await.unwrap();
        assert!(
            matches!(reconcile, ReconcileOutcome::Committed { receipt } if receipt.contains("fx-resolve"))
        );
    }

    #[tokio::test]
    async fn ambiguous_then_reconcile_committed_prevents_double_commit() {
        let c = CrashOnCommitConnector::new(1);
        let pe = c
            .prepare(serde_json::json!({}), "fx-double".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));

        let pe2 = c
            .prepare(serde_json::json!({}), "fx-double-2".into(), 2)
            .await
            .unwrap();
        let result = c.commit(pe2).await;
        assert!(result.is_err());

        let reconcile = c.reconcile("fx-double-2").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn reconcile_outcome_still_ambiguous_triggers_retry_action() {
        use vo_types::ReconcileAction;
        let outcome = ReconcileOutcome::StillAmbiguous;
        let action = ReconcileAction::from(outcome);
        assert_eq!(action, ReconcileAction::Retry);
    }

    #[tokio::test]
    async fn reconcile_outcome_committed_triggers_commit_action() {
        use vo_types::ReconcileAction;
        let outcome = ReconcileOutcome::Committed {
            receipt: "found".into(),
        };
        let action = ReconcileAction::from(outcome);
        assert_eq!(action, ReconcileAction::Commit);
    }

    #[tokio::test]
    async fn reconcile_outcome_not_committed_triggers_rollback_action() {
        use vo_types::ReconcileAction;
        let outcome = ReconcileOutcome::NotCommitted;
        let action = ReconcileAction::from(outcome);
        assert_eq!(action, ReconcileAction::Rollback);
    }

    #[tokio::test]
    async fn sql_connector_crash_then_reconcile_resolves() {
        let c = SqlConnector::new();
        c.crash_on_commit.store(true, Ordering::SeqCst);

        let pe = c
            .prepare(serde_json::json!({"query": "INSERT"}), "tx-crash".into(), 1)
            .await
            .unwrap();
        let result = c.commit(pe).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_retryable());

        let reconcile = c.reconcile("tx-crash").await.unwrap();
        assert_eq!(reconcile, ReconcileOutcome::NotCommitted);
    }

    #[tokio::test]
    async fn sql_connector_crash_after_commit_reconcile_finds_committed() {
        let c = SqlConnector::new();
        let pe = c
            .prepare(serde_json::json!({"query": "INSERT"}), "tx-ok".into(), 1)
            .await
            .unwrap();
        let _ = c.commit(pe).await.unwrap();

        c.crash_on_commit.store(true, Ordering::SeqCst);
        let pe2 = c
            .prepare(serde_json::json!({"query": "INSERT"}), "tx-ok-2".into(), 2)
            .await
            .unwrap();
        let result = c.commit(pe2).await;
        assert!(result.is_err());

        let reconcile = c.reconcile("tx-ok").await.unwrap();
        assert!(matches!(reconcile, ReconcileOutcome::Committed { .. }));
    }
}