//! Connector lifecycle management (ADR-041 §5).

<<<<<<< HEAD
use crate::connector::Connector;
use std::collections::HashMap;
use std::sync::Arc;
=======
use std::collections::HashMap;
use std::sync::Arc;
use crate::connector::Connector;
>>>>>>> origin/polecat/synth-mnw6kj8v

/// Registry for managing connector instances by type name.
pub struct ConnectorRegistry {
    connectors: HashMap<String, Arc<dyn Connector>>,
}

<<<<<<< HEAD
impl Default for ConnectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

=======
>>>>>>> origin/polecat/synth-mnw6kj8v
impl ConnectorRegistry {
    pub fn new() -> Self {
        Self {
            connectors: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: String, connector: Box<dyn Connector>) {
        self.connectors.insert(name, Arc::from(connector));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Connector>> {
        self.connectors.get(name).cloned()
    }

    pub fn len(&self) -> usize {
        self.connectors.len()
    }

<<<<<<< HEAD
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.connectors.is_empty()
    }

=======
>>>>>>> origin/polecat/synth-mnw6kj8v
    pub fn list(&self) -> Vec<&str> {
        self.connectors.keys().map(String::as_str).collect()
    }
}
<<<<<<< HEAD

#[cfg(test)]
mod tests {
    use super::*;
<<<<<<< HEAD
    use crate::connector::{
        CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome,
    };
=======
    use crate::connector::{CommitOutcome, Connector, ConnectorError, PreparedEffect, ReconcileOutcome};
>>>>>>> origin/vo-worker-tests
    use async_trait::async_trait;
    use serde_json::json;

    #[derive(Clone)]
    struct MockConnector {
        name: String,
    }

    #[async_trait]
    impl Connector for MockConnector {
<<<<<<< HEAD
        fn connector_type(&self) -> &str {
            &self.name
        }
        fn connector_version(&self) -> &str {
            "1.0.0"
        }
        fn supports_compensation(&self) -> bool {
            false
        }
=======
        fn connector_type(&self) -> &str { &self.name }
        fn connector_version(&self) -> &str { "1.0.0" }
        fn supports_compensation(&self) -> bool { false }
>>>>>>> origin/vo-worker-tests

        async fn prepare(
            &self,
            _effect_intent: serde_json::Value,
            effect_id: String,
            fence: u64,
        ) -> Result<PreparedEffect, ConnectorError> {
<<<<<<< HEAD
            Ok(PreparedEffect {
                effect_id,
                payload: json!({}),
                fence,
            })
        }

        async fn commit(&self, _prepared: PreparedEffect) -> Result<CommitOutcome, ConnectorError> {
            Ok(CommitOutcome::Committed {
                receipt: "mock".into(),
            })
        }

        async fn reconcile(&self, _effect_id: &str) -> Result<ReconcileOutcome, ConnectorError> {
=======
            Ok(PreparedEffect { effect_id, payload: json!({}), fence })
        }

        async fn commit(
            &self,
            _prepared: PreparedEffect,
        ) -> Result<CommitOutcome, ConnectorError> {
            Ok(CommitOutcome::Committed { receipt: "mock".into() })
        }

        async fn reconcile(
            &self,
            _effect_id: &str,
        ) -> Result<ReconcileOutcome, ConnectorError> {
>>>>>>> origin/vo-worker-tests
            Ok(ReconcileOutcome::NotCommitted)
        }
    }

    #[tokio::test]
    async fn test_registry_new_empty() {
        let registry = ConnectorRegistry::new();
        assert_eq!(registry.len(), 0);
        assert!(registry.list().is_empty());
    }

    #[tokio::test]
    async fn test_registry_register_single() {
        let mut registry = ConnectorRegistry::new();
<<<<<<< HEAD
        registry.register(
            "mock".to_string(),
            Box::new(MockConnector {
                name: "mock".to_string(),
            }),
        );
=======
        registry.register("mock".to_string(), Box::new(MockConnector { name: "mock".to_string() }));
>>>>>>> origin/vo-worker-tests

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.list(), vec!["mock"]);
    }

    #[tokio::test]
    async fn test_registry_register_multiple() {
        let mut registry = ConnectorRegistry::new();
<<<<<<< HEAD
        registry.register(
            "http".to_string(),
            Box::new(MockConnector {
                name: "http".to_string(),
            }),
        );
        registry.register(
            "sqs".to_string(),
            Box::new(MockConnector {
                name: "sqs".to_string(),
            }),
        );
        registry.register(
            "s3".to_string(),
            Box::new(MockConnector {
                name: "s3".to_string(),
            }),
        );
=======
        registry.register("http".to_string(), Box::new(MockConnector { name: "http".to_string() }));
        registry.register("sqs".to_string(), Box::new(MockConnector { name: "sqs".to_string() }));
        registry.register("s3".to_string(), Box::new(MockConnector { name: "s3".to_string() }));
>>>>>>> origin/vo-worker-tests

        assert_eq!(registry.len(), 3);
        let list = registry.list();
        assert!(list.contains(&"http"));
        assert!(list.contains(&"sqs"));
        assert!(list.contains(&"s3"));
    }

    #[tokio::test]
    async fn test_registry_get_existing() {
        let mut registry = ConnectorRegistry::new();
<<<<<<< HEAD
        let connector = MockConnector {
            name: "http".to_string(),
        };
=======
        let connector = MockConnector { name: "http".to_string() };
>>>>>>> origin/vo-worker-tests
        registry.register("http".to_string(), Box::new(connector.clone()));

        let retrieved = registry.get("http");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().connector_type(), "http");
    }

    #[tokio::test]
    async fn test_registry_get_nonexistent() {
        let registry = ConnectorRegistry::new();
        let retrieved = registry.get("nonexistent");
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_registry_get_after_register() {
        let mut registry = ConnectorRegistry::new();
<<<<<<< HEAD

        registry.register(
            "connector1".to_string(),
            Box::new(MockConnector {
                name: "connector1".to_string(),
            }),
        );
        registry.register(
            "connector2".to_string(),
            Box::new(MockConnector {
                name: "connector2".to_string(),
            }),
        );

        assert_eq!(
            registry.get("connector1").unwrap().connector_type(),
            "connector1"
        );
        assert_eq!(
            registry.get("connector2").unwrap().connector_type(),
            "connector2"
        );
=======
        
        registry.register("connector1".to_string(), Box::new(MockConnector { name: "connector1".to_string() }));
        registry.register("connector2".to_string(), Box::new(MockConnector { name: "connector2".to_string() }));

        assert_eq!(registry.get("connector1").unwrap().connector_type(), "connector1");
        assert_eq!(registry.get("connector2").unwrap().connector_type(), "connector2");
>>>>>>> origin/vo-worker-tests
    }

    #[tokio::test]
    async fn test_registry_list_order() {
        let mut registry = ConnectorRegistry::new();
<<<<<<< HEAD
        registry.register(
            "z".to_string(),
            Box::new(MockConnector {
                name: "z".to_string(),
            }),
        );
        registry.register(
            "a".to_string(),
            Box::new(MockConnector {
                name: "a".to_string(),
            }),
        );
        registry.register(
            "m".to_string(),
            Box::new(MockConnector {
                name: "m".to_string(),
            }),
        );
=======
        registry.register("z".to_string(), Box::new(MockConnector { name: "z".to_string() }));
        registry.register("a".to_string(), Box::new(MockConnector { name: "a".to_string() }));
        registry.register("m".to_string(), Box::new(MockConnector { name: "m".to_string() }));
>>>>>>> origin/vo-worker-tests

        let list = registry.list();
        assert_eq!(list.len(), 3);
        // HashMap iteration order is not guaranteed, just check all present
        assert!(list.contains(&"z"));
        assert!(list.contains(&"a"));
        assert!(list.contains(&"m"));
    }

    #[tokio::test]
    async fn test_registry_overwrite_connector() {
        let mut registry = ConnectorRegistry::new();
<<<<<<< HEAD

        registry.register(
            "test".to_string(),
            Box::new(MockConnector {
                name: "original".to_string(),
            }),
        );
        registry.register(
            "test".to_string(),
            Box::new(MockConnector {
                name: "updated".to_string(),
            }),
        );
=======
        
        registry.register("test".to_string(), Box::new(MockConnector { name: "original".to_string() }));
        registry.register("test".to_string(), Box::new(MockConnector { name: "updated".to_string() }));
>>>>>>> origin/vo-worker-tests

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get("test").unwrap().connector_type(), "updated");
    }

    #[tokio::test]
    async fn test_registry_empty_list() {
        let registry = ConnectorRegistry::new();
        let list = registry.list();
        assert!(list.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[tokio::test]
    async fn test_registry_register_clone() {
        let mut registry = ConnectorRegistry::new();
<<<<<<< HEAD
        let connector = MockConnector {
            name: "shared".to_string(),
        };

        registry.register("shared".to_string(), Box::new(connector.clone()));

        let retrieved1 = registry.get("shared");
        let retrieved2 = registry.get("shared");

=======
        let connector = MockConnector { name: "shared".to_string() };
        
        registry.register("shared".to_string(), Box::new(connector.clone()));
        
        let retrieved1 = registry.get("shared");
        let retrieved2 = registry.get("shared");
        
>>>>>>> origin/vo-worker-tests
        assert!(retrieved1.is_some());
        assert!(retrieved2.is_some());
        assert_eq!(retrieved1.unwrap().connector_type(), "shared");
        assert_eq!(retrieved2.unwrap().connector_type(), "shared");
    }

    #[tokio::test]
    async fn test_registry_connector_types() {
        let mut registry = ConnectorRegistry::new();
<<<<<<< HEAD
        registry.register(
            "http".to_string(),
            Box::new(MockConnector {
                name: "http".to_string(),
            }),
        );
        registry.register(
            "grpc".to_string(),
            Box::new(MockConnector {
                name: "grpc".to_string(),
            }),
        );
        registry.register(
            "amqp".to_string(),
            Box::new(MockConnector {
                name: "amqp".to_string(),
            }),
        );
=======
        registry.register("http".to_string(), Box::new(MockConnector { name: "http".to_string() }));
        registry.register("grpc".to_string(), Box::new(MockConnector { name: "grpc".to_string() }));
        registry.register("amqp".to_string(), Box::new(MockConnector { name: "amqp".to_string() }));
>>>>>>> origin/vo-worker-tests

        let http = registry.get("http").unwrap();
        let grpc = registry.get("grpc").unwrap();
        let amqp = registry.get("amqp").unwrap();

        assert_eq!(http.connector_type(), "http");
        assert_eq!(grpc.connector_type(), "grpc");
        assert_eq!(amqp.connector_type(), "amqp");
    }
}
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
