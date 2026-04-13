//! Connector lifecycle management (ADR-041 §5).

use std::collections::HashMap;
use std::sync::Arc;
use crate::connector::Connector;

/// Registry for managing connector instances by type name.
pub struct ConnectorRegistry {
    connectors: HashMap<String, Arc<dyn Connector>>,
}

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

    pub fn list(&self) -> Vec<&str> {
        self.connectors.keys().map(String::as_str).collect()
    }
}
