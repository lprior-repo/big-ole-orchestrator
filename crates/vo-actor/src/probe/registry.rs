//! Probe registry for managing probe definitions.

use super::types::{ProbeDefinition, ProbeId};
use std::collections::HashMap;

pub struct ProbeRegistry {
    probes: HashMap<ProbeId, ProbeDefinition>,
}

impl ProbeRegistry {
    /// Create new empty registry.
    pub fn new() -> Self {
        Self {
            probes: HashMap::new(),
        }
    }

    /// Register a probe definition.
    pub fn register(&mut self, definition: ProbeDefinition) -> ProbeId {
        let id = definition.id;
        self.probes.insert(id, definition);
        id
    }

    /// Unregister a probe by ID.
    pub fn unregister(&mut self, id: ProbeId) -> Option<ProbeDefinition> {
        self.probes.remove(&id)
    }

    /// Get a probe definition by ID.
    pub fn get(&self, id: &ProbeId) -> Option<&ProbeDefinition> {
        self.probes.get(id)
    }

    /// List all registered probes.
    pub fn list(&self) -> Vec<&ProbeDefinition> {
        self.probes.values().collect()
    }

    /// Get number of registered probes.
    pub fn len(&self) -> usize {
        self.probes.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }
}

impl Default for ProbeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
