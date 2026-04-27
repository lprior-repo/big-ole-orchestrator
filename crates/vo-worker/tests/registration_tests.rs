#[cfg(test)]
mod registration_tests {
    use super::*;

    #[test]
    fn registry_new_is_empty() {
        let reg = ConnectorRegistry::new();
        assert_eq!(reg.len(), 0);
        assert!(reg.list().is_empty());
    }

    #[test]
    fn registry_register_increments_len() {
        let mut reg = ConnectorRegistry::new();
        reg.register("c1".to_string(), Box::new(AlwaysCommittedConnector));
        assert_eq!(reg.len(), 1);
        reg.register("c2".to_string(), Box::new(AlwaysFailedConnector));
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn registry_register_same_name_overwrites() {
        let mut reg = ConnectorRegistry::new();
        reg.register("c1".to_string(), Box::new(AlwaysCommittedConnector));
        reg.register("c1".to_string(), Box::new(AlwaysFailedConnector));
        assert_eq!(reg.len(), 1);
    }

    #[tokio::test]
    async fn registry_get_returns_none_for_missing() {
        let reg = ConnectorRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[tokio::test]
    async fn registry_get_returns_registered_connector() {
        let mut reg = ConnectorRegistry::new();
        reg.register("committed".to_string(), Box::new(AlwaysCommittedConnector));
        let c = reg.get("committed").expect("should exist");
        assert_eq!(c.connector_type(), "always-committed");
    }

    #[tokio::test]
    async fn registry_list_contains_all_names() {
        let mut reg = ConnectorRegistry::new();
        reg.register("a".to_string(), Box::new(AlwaysCommittedConnector));
        reg.register("b".to_string(), Box::new(AlwaysFailedConnector));
        reg.register("c".to_string(), Box::new(CrashOnCommitConnector::new(1)));
        let names = reg.list();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
        assert!(names.contains(&"c"));
    }

    #[tokio::test]
    async fn registry_connector_is_usable_after_retrieval() {
        let mut reg = ConnectorRegistry::new();
        reg.register("sql".to_string(), Box::new(SqlConnector::new()));
        let c = reg.get("sql").unwrap();
        let pe = c
            .prepare(serde_json::json!({"query": "SELECT 1"}), "tx-1".into(), 1)
            .await
            .unwrap();
        let outcome = c.commit(pe).await.unwrap();
        assert!(matches!(outcome, CommitOutcome::Committed { .. }));
    }

    #[tokio::test]
    async fn registry_multiple_gets_return_same_arc() {
        let mut reg = ConnectorRegistry::new();
        reg.register("c".to_string(), Box::new(AlwaysCommittedConnector));
        let a = reg.get("c").unwrap();
        let b = reg.get("c").unwrap();
        assert_eq!(a.connector_type(), b.connector_type());
    }
}