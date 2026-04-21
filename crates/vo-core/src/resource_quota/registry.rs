use super::{NamespaceQuota, QuotaError};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct NamespaceRegistry {
    quotas: HashMap<String, NamespaceQuota>,
}

impl NamespaceRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            quotas: HashMap::new(),
        }
    }

    pub fn register(&mut self, quota: NamespaceQuota) -> Result<(), QuotaError> {
        let namespace = quota.namespace.clone();
        self.quotas.insert(namespace, quota);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, namespace: &str) -> Option<&NamespaceQuota> {
        self.quotas.get(namespace)
    }

    pub fn remove(&mut self, namespace: &str) -> Option<NamespaceQuota> {
        self.quotas.remove(namespace)
    }

    #[must_use]
    pub fn list_namespaces(&self) -> Vec<&str> {
        self.quotas.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for NamespaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
