//! Event schema upcasting support (ADR-035).
//!
//! Upcasters normalize older event schema versions to newer versions before replay
//! or projection building. The upcaster chain transforms payloads incrementally
//! from their recorded version up to the current MAX_SUPPORTED_VERSION.

use crate::events::envelope::EventEnvelope;
use crate::events::error::Error;
use crate::events::MAX_SUPPORTED_VERSION;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UpcasterError {
    #[error("No upcaster registered for version transition: {from} → {to}")]
    NoUpcasterRegistered { from: u8, to: u8 },
    #[error("Upcaster failed: {0}")]
    UpcastFailed(String),
    #[error("Invalid version: {0}")]
    InvalidVersion(u8),
    #[error("Upcast chain exhausted before reaching target version")]
    ChainExhausted,
    #[error("Invalid target version: {0}")]
    InvalidTargetVersion(u8),
    #[error("Duplicate registration for source version: {0}")]
    DuplicateRegistration(u8),
}

pub trait Upcaster: Send + Sync {
    fn source_version(&self) -> u8;
    fn target_version(&self) -> u8;
    fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError>;
}

pub struct VersionRegistry {
    upcasters: HashMap<(u8, u8), Box<dyn Upcaster>>,
}

impl VersionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            upcasters: HashMap::new(),
        }
    }

    pub fn register(&mut self, upcaster: Box<dyn Upcaster>) {
        let key = (upcaster.source_version(), upcaster.target_version());
        self.upcasters.insert(key, upcaster);
    }

    pub fn get(&self, from: u8, to: u8) -> Option<&dyn Upcaster> {
        self.upcasters.get(&(from, to)).map(|b| b.as_ref())
    }

    pub fn upcast_payload(
        &self,
        mut payload: serde_json::Value,
        mut from_version: u8,
        to_version: u8,
    ) -> Result<serde_json::Value, Error> {
        if from_version == to_version {
            return Ok(payload);
        }
        if from_version > to_version {
            return Err(Error::InvalidSchemaVersionFormat);
        }
        while from_version < to_version {
            let next_version = from_version + 1;
            let upcaster = self
                .get(from_version, next_version)
                .ok_or(Error::UpcasterNotFound {
                    from: from_version,
                    to: next_version,
                })?;
            payload = upcaster
                .upcast(&payload)
                .map_err(|e| Error::UpcastFailed(e.to_string()))?;
            from_version = next_version;
        }
        Ok(payload)
    }
}

impl Default for VersionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEnvelope {
    pub fn upcast_payload(
        &self,
        registry: &VersionRegistry,
        target_version: u8,
    ) -> Result<serde_json::Value, Error> {
        if self.schema_version > MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedEnvelopeVersion(self.schema_version));
        }
        if target_version > MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedSchemaVersion(target_version.into()));
        }
        if self.schema_version == target_version {
            return Ok(self.payload.clone());
        }
        registry.upcast_payload(self.payload.clone(), self.schema_version, target_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventMetadata;
    use serde_json::json;

    struct TestUpcaster {
        from: u8,
        to: u8,
    }
    impl TestUpcaster {
        fn new(from: u8, to: u8) -> Self {
            Self { from, to }
        }
    }
    impl Upcaster for TestUpcaster {
        fn source_version(&self) -> u8 {
            self.from
        }
        fn target_version(&self) -> u8 {
            self.to
        }
        fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
            let mut result = payload.clone();
            if let Some(obj) = result.as_object_mut() {
                obj.insert(
                    "upcasted_by".to_string(),
                    json!(format!("v{}→v{}", self.from, self.to)),
                );
                if let Some(v) = obj.get_mut("version") {
                    *v = json!(self.to);
                }
            }
            Ok(result)
        }
    }

    #[test]
    fn registry_registers_and_retrieves_upcaster() {
        let mut registry = VersionRegistry::new();
        registry.register(Box::new(TestUpcaster::new(0, 1)));
        let upcaster = registry.get(0, 1).expect("should find upcaster");
        assert_eq!(upcaster.source_version(), 0);
        assert_eq!(upcaster.target_version(), 1);
    }

    #[test]
    fn registry_returns_none_for_missing_upcaster() {
        let registry = VersionRegistry::new();
        assert!(registry.get(0, 1).is_none());
    }

    #[test]
    fn registry_upcast_payload_single_step() {
        let mut registry = VersionRegistry::new();
        registry.register(Box::new(TestUpcaster::new(0, 1)));
        let payload = json!({"type": "Test", "version": 0});
        let result = registry
            .upcast_payload(payload, 0, 1)
            .expect("upcast should succeed");
        assert_eq!(result["version"], 1);
        assert_eq!(result["upcasted_by"], "v0→v1");
    }

    #[test]
    fn registry_upcast_payload_multi_step() {
        let mut registry = VersionRegistry::new();
        registry.register(Box::new(TestUpcaster::new(0, 1)));
        registry.register(Box::new(TestUpcaster::new(1, 2)));
        let payload = json!({"type": "Test", "version": 0});
        let result = registry
            .upcast_payload(payload, 0, 2)
            .expect("upcast should succeed");
        assert_eq!(result["version"], 2);
        assert_eq!(result["upcasted_by"], "v1→v2");
    }

    #[test]
    fn registry_upcast_payload_same_version_returns_original() {
        let registry = VersionRegistry::new();
        let payload = json!({"type": "Test", "version": 1});
        let result = registry
            .upcast_payload(payload, 1, 1)
            .expect("upcast should succeed");
        assert_eq!(result["version"], 1);
    }

    #[test]
    fn registry_upcast_payload_returns_error_when_chain_broken() {
        let mut registry = VersionRegistry::new();
        registry.register(Box::new(TestUpcaster::new(0, 1)));
        let payload = json!({"type": "Test", "version": 0});
        let result = registry.upcast_payload(payload, 0, 2);
        assert!(result.is_err());
    }

    #[test]
    fn event_envelope_upcast_payload() {
        let mut registry = VersionRegistry::new();
        registry.register(Box::new(TestUpcaster::new(0, 1)));
        let envelope = EventEnvelope {
            schema_version: 0,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: json!({"type": "Test", "version": 0}),
            metadata: EventMetadata::default(),
        };
        let result = envelope
            .upcast_payload(&registry, 1)
            .expect("upcast should succeed");
        assert_eq!(result["version"], 1);
    }
}
