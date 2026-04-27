use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::types::{ProbeId, ProbeStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub probe_id: ProbeId,
    pub status: ProbeStatus,
    pub latency_ms: u64,
    pub consecutive_failures: u32,
    pub last_check_ms: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AggregatedStatus {
    pub overall: ProbeStatus,
    pub healthy_count: u32,
    pub unhealthy_count: u32,
    pub unknown_count: u32,
    pub results: HashMap<ProbeId, ProbeResult>,
}

impl AggregatedStatus {
    pub fn new() -> Self {
        Self {
            overall: ProbeStatus::Unknown,
            healthy_count: 0,
            unhealthy_count: 0,
            unknown_count: 0,
            results: HashMap::new(),
        }
    }
    pub fn update(&mut self, result: ProbeResult) {
        if let Some(old_result) = self.results.get(&result.probe_id) {
            match old_result.status {
                ProbeStatus::Healthy => self.healthy_count -= 1,
                ProbeStatus::Unhealthy => self.unhealthy_count -= 1,
                ProbeStatus::Unknown => self.unknown_count -= 1,
            }
        }
        match result.status {
            ProbeStatus::Healthy => self.healthy_count += 1,
            ProbeStatus::Unhealthy => self.unhealthy_count += 1,
            ProbeStatus::Unknown => self.unknown_count += 1,
        }
        self.results.insert(result.probe_id, result);
        self.overall = if self.unhealthy_count > 0 {
            ProbeStatus::Unhealthy
        } else if self.healthy_count > 0 && self.unknown_count == 0 {
            ProbeStatus::Healthy
        } else {
            ProbeStatus::Unknown
        };
    }
    pub fn is_healthy(&self) -> bool {
        self.overall == ProbeStatus::Healthy
    }
}

impl Default for AggregatedStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ProbeRegistry {
    probes: HashMap<ProbeId, super::types::ProbeDefinition>,
}

impl ProbeRegistry {
    pub fn new() -> Self {
        Self {
            probes: HashMap::new(),
        }
    }
    pub fn register(&mut self, definition: super::types::ProbeDefinition) -> ProbeId {
        let id = definition.id;
        self.probes.insert(id, definition);
        id
    }
    pub fn unregister(&mut self, id: ProbeId) -> Option<super::types::ProbeDefinition> {
        self.probes.remove(&id)
    }
    pub fn get(&self, id: &ProbeId) -> Option<&super::types::ProbeDefinition> {
        self.probes.get(id)
    }
    pub fn list(&self) -> Vec<&super::types::ProbeDefinition> {
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