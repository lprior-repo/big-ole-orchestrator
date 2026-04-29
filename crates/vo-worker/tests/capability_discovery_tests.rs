#[cfg(test)]
mod capability_discovery_tests {
    use super::*;

    #[tokio::test]
    async fn http_connector_reports_type() {
        let c = HttpConnector::new("https://api.example.com");
        assert_eq!(c.connector_type(), "http");
    }

    #[tokio::test]
    async fn http_connector_reports_version() {
        let c = HttpConnector::new("https://api.example.com");
        assert_eq!(c.connector_version(), "1.0.0");
    }

    #[tokio::test]
    async fn http_connector_no_compensation() {
        let c = HttpConnector::new("https://api.example.com");
        assert!(!c.supports_compensation());
    }

    #[tokio::test]
    async fn sql_connector_reports_type() {
        let c = SqlConnector::new();
        assert_eq!(c.connector_type(), "sql");
    }

    #[tokio::test]
    async fn sql_connector_reports_version() {
        let c = SqlConnector::new();
        assert_eq!(c.connector_version(), "1.0.0");
    }

    #[tokio::test]
    async fn sql_connector_supports_compensation() {
        let c = SqlConnector::new();
        assert!(c.supports_compensation());
    }

    #[tokio::test]
    async fn compensating_connector_reports_capability() {
        let c = CompensatingConnector::new();
        assert_eq!(c.connector_type(), "compensating");
        assert!(c.supports_compensation());
    }

    #[tokio::test]
    async fn crash_connector_reports_capability() {
        let c = CrashOnCommitConnector::new(1);
        assert!(c.supports_compensation());
    }

    #[tokio::test]
    async fn registry_discovery_multiple_types() {
        let mut reg = ConnectorRegistry::new();
        reg.register(
            "http".to_string(),
            Box::new(HttpConnector::new("https://api.example.com")),
        );
        reg.register("sql".to_string(), Box::new(SqlConnector::new()));
        reg.register("noop".to_string(), Box::new(AlwaysCommittedConnector));

        let http = reg.get("http").unwrap();
        let sql = reg.get("sql").unwrap();
        let noop = reg.get("noop").unwrap();

        assert_eq!(http.connector_type(), "http");
        assert!(!http.supports_compensation());

        assert_eq!(sql.connector_type(), "sql");
        assert!(sql.supports_compensation());

        assert_eq!(noop.connector_type(), "always-committed");
        assert!(!noop.supports_compensation());
    }
}
