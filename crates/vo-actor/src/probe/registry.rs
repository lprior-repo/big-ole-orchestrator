use std::collections::HashMap;
use std::time::Duration;

use super::types::{ProbeDefinition, ProbeId};

#[derive(Debug, Clone)]
pub struct ProbeRegistry {
    probes: HashMap<ProbeId, ProbeDefinition>,
}

impl ProbeRegistry {
    pub fn new() -> Self {
        Self {
            probes: HashMap::new(),
        }
    }

    pub fn register(&mut self, definition: ProbeDefinition) -> ProbeId {
        let id = definition.id;
        self.probes.insert(id, definition);
        id
    }

    pub fn unregister(&mut self, id: ProbeId) -> Option<ProbeDefinition> {
        self.probes.remove(&id)
    }

    pub fn get(&self, id: &ProbeId) -> Option<&ProbeDefinition> {
        self.probes.get(id)
    }

    pub fn list(&self) -> Vec<&ProbeDefinition> {
        self.probes.values().collect()
    }

    pub fn len(&self) -> usize {
        self.probes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }
}

impl Default for ProbeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{BackoffConfig, ProbeConfig};

    fn make_definition(name: &str) -> ProbeDefinition {
        ProbeDefinition {
            id: ProbeId::new(),
            name: name.to_string(),
            config: ProbeConfig::http("http://localhost"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        }
    }

    #[test]
    fn test_probe_registry_register() {
        let mut registry = ProbeRegistry::new();
        let definition = make_definition("test");
        let id = registry.register(definition);
        assert!(registry.get(&id).is_some());
    }

    #[test]
    fn test_probe_registry_unregister() {
        let mut registry = ProbeRegistry::new();
        let definition = make_definition("test");
        let id = registry.register(definition);
        let removed = registry.unregister(id);
        assert!(removed.is_some());
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn test_probe_registry_unregister_nonexistent() {
        let mut registry = ProbeRegistry::new();
        let id = ProbeId::new();
        assert!(registry.unregister(id).is_none());
    }

    #[test]
    fn test_probe_registry_get() {
        let mut registry = ProbeRegistry::new();
        let definition = make_definition("test");
        let id = registry.register(definition);
        assert!(registry.get(&id).is_some());
    }

    #[test]
    fn test_probe_registry_get_nonexistent() {
        let registry = ProbeRegistry::new();
        let id = ProbeId::new();
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn test_probe_registry_list() {
        let mut registry = ProbeRegistry::new();
        for i in 0..3 {
            let definition = make_definition(&format!("test{}", i));
            registry.register(definition);
        }
        assert_eq!(registry.list().len(), 3);
    }

    #[test]
    fn test_probe_registry_len() {
        let mut registry = ProbeRegistry::new();
        assert_eq!(registry.len(), 0);
        let definition = make_definition("test");
        registry.register(definition);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_probe_registry_is_empty() {
        let registry = ProbeRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_probe_registry_is_empty_after_register() {
        let mut registry = ProbeRegistry::new();
        let definition = make_definition("test");
        registry.register(definition);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_thread_safety_concurrent_register() {
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let registry = Arc::new(Mutex::new(ProbeRegistry::new()));
        let mut handles = vec![];

        for i in 0..10 {
            let reg = Arc::clone(&registry);
            let handle = std::thread::spawn(move || {
                let definition = make_definition(&format!("test{}", i));
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let mut reg = reg.lock().await;
                    reg.register(definition);
                });
            });
            handles.push(handle);
        }

        for h in handles {
            h.join().unwrap();
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let reg = registry.lock().await;
            assert_eq!(reg.len(), 10);
        });
    }

    #[test]
    fn qa_smoke_registry_crud_lifecycle() {
        let mut registry = ProbeRegistry::new();
        assert!(registry.is_empty());

        let defs: Vec<ProbeDefinition> = (0..5)
            .map(|i| ProbeDefinition {
                id: ProbeId::new(),
                name: format!("probe-{}", i),
                config: ProbeConfig::http(format!("http://localhost:{}", 8080 + i)),
                interval: Duration::from_secs(30),
                backoff: BackoffConfig::default(),
                failure_threshold: 3,
                success_threshold: 2,
            })
            .collect();

        let mut ids = vec![];
        for def in defs {
            ids.push(registry.register(def));
        }
        assert_eq!(registry.len(), 5);

        let removed = registry.unregister(ids[2]);
        assert!(removed.is_some());
        assert!(registry.get(&ids[2]).is_none());
        assert_eq!(registry.len(), 4);

        let list = registry.list();
        assert_eq!(list.len(), 4);
    }
}
