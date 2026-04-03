//! Upcaster registry for resolving and applying upcaster chains.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::upcaster::{EventEnvelopeError, Upcaster, UpcasterError, MAX_SUPPORTED_VERSION};
use vo_types::events::EventEnvelope;

/// Registry for resolving and applying upcaster chains
pub trait UpcasterRegistry: Send + Sync {
    /// Register an upcaster for a specific source version
    fn register(&self, upcaster: Box<dyn Upcaster>) -> Result<(), UpcasterError>;

    /// Resolve the upcaster chain and apply all necessary transforms
    fn upcast_envelope(&self, envelope: EventEnvelope) -> Result<EventEnvelope, UpcasterError>;

    /// Get the current maximum supported version
    fn max_supported_version(&self) -> u8;
}

/// Builder for constructing an upcaster registry
pub trait UpcasterRegistryBuilder: Send + Sync {
    fn build() -> Box<dyn UpcasterRegistry>;
}

// =============================================================================
// Concrete Implementation: UpcasterRegistryImpl
// =============================================================================

/// Concrete implementation of UpcasterRegistry for production use.
pub struct UpcasterRegistryImpl {
    upcasters: Arc<Mutex<HashMap<u8, Box<dyn Upcaster>>>>,
    max_version: u8,
}

impl UpcasterRegistryImpl {
    /// Creates a new UpcasterRegistryImpl with the given maximum supported version.
    #[must_use]
    pub fn new(max_version: u8) -> Self {
        Self {
            upcasters: Arc::new(Mutex::new(HashMap::new())),
            max_version,
        }
    }
}

impl UpcasterRegistry for UpcasterRegistryImpl {
    /// Register an upcaster for a specific source version.
    ///
    /// # Errors
    ///
    /// Returns `UpcasterError::InvalidTargetVersion` if the source version exceeds
    /// the maximum supported version.
    ///
    /// Returns `UpcasterError::NoUpcasterRegistered` if an upcaster is already
    /// registered for the given source version.
    fn register(&self, upcaster: Box<dyn Upcaster>) -> Result<(), UpcasterError> {
        let source_version = upcaster.source_version();

        // Reject upcasters whose source version is at or above max
        // (since upcasters produce source_version + 1, this would exceed max)
        if source_version >= self.max_version {
            return Err(UpcasterError::InvalidTargetVersion(source_version));
        }

        let mut upcasters = self
            .upcasters
            .lock()
            .map_err(|_| UpcasterError::UpcastingFailed("lock poisoned".to_string()))?;

        if upcasters.contains_key(&source_version) {
            return Err(UpcasterError::NoUpcasterRegistered(source_version));
        }

        upcasters.insert(source_version, upcaster);
        Ok(())
    }

    /// Resolve the upcaster chain and apply all necessary transforms.
    ///
    /// # Errors
    ///
    /// Returns `UpcasterError::NoUpcasterRegistered` if no upcaster is registered
    /// for the current envelope version.
    ///
    /// Returns `UpcasterError::InvalidTargetVersion` if an upcaster produces a
    /// version exceeding the maximum supported version.
    ///
    /// Returns `UpcasterError::CircularChain` if the upcasters form a cycle.
    ///
    /// Returns `UpcasterError::InvalidUpcastedEnvelope` if an upcaster produces
    /// output that cannot be parsed as a valid envelope.
    fn upcast_envelope(&self, envelope: EventEnvelope) -> Result<EventEnvelope, UpcasterError> {
        // If already at or above max version, return unchanged
        if envelope.version >= self.max_version {
            return Ok(envelope);
        }

        let upcasters = self
            .upcasters
            .lock()
            .map_err(|_| UpcasterError::UpcastingFailed("lock poisoned".to_string()))?;

        let mut current_version = envelope.version;
        let mut current_payload = envelope.payload.clone();

        // Track visited versions to detect cycles
        let mut visited = HashMap::new();
        visited.insert(current_version, true);

        loop {
            // Check for circular chain - if we've seen this version before, error
            if visited.get(&current_version) == Some(&true) {
                return Err(UpcasterError::CircularChain(current_version));
            }

            // If we've reached max version, we're done
            if current_version >= self.max_version {
                break;
            }

            let upcaster = upcasters
                .get(&current_version)
                .ok_or(UpcasterError::NoUpcasterRegistered(current_version))?;

            // Serialize current state as envelope JSON for upcaster input
            let mut envelope_json = serde_json::Map::new();
            envelope_json.insert("version".to_string(), serde_json::json!(current_version));
            envelope_json.insert(
                "instance_id".to_string(),
                serde_json::json!(envelope.instance_id.clone()),
            );
            envelope_json.insert("sequence".to_string(), serde_json::json!(envelope.sequence));
            envelope_json.insert(
                "timestamp_ms".to_string(),
                serde_json::json!(envelope.timestamp_ms),
            );
            envelope_json.insert("payload".to_string(), current_payload);
            envelope_json.insert("metadata".to_string(), envelope.metadata.clone());

            let input_bytes = serde_json::to_vec(&envelope_json).map_err(|e| {
                UpcasterError::UpcastingFailed(format!("serialization error: {}", e))
            })?;

            let output_bytes = upcaster.upcast(&input_bytes)?;

            // Parse the output as JSON
            let output_json: serde_json::Value =
                serde_json::from_slice(&output_bytes).map_err(|_| {
                    UpcasterError::InvalidUpcastedEnvelope(
                        EventEnvelopeError::InvalidEnvelopeFormat,
                    )
                })?;

            let output_obj =
                output_json
                    .as_object()
                    .ok_or(UpcasterError::InvalidUpcastedEnvelope(
                        EventEnvelopeError::InvalidEnvelopeFormat,
                    ))?;

            // Extract new version from output
            let new_version = output_obj
                .get("version")
                .and_then(|v| v.as_u64())
                .map(u8::try_from)
                .ok_or_else(|| {
                    UpcasterError::InvalidUpcastedEnvelope(
                        EventEnvelopeError::MissingEnvelopeField("version".to_string()),
                    )
                })?
                .map_err(|_| {
                    UpcasterError::InvalidUpcastedEnvelope(
                        EventEnvelopeError::InvalidEnvelopeField(
                            "version exceeds u8 range".to_string(),
                        ),
                    )
                })?;

            // Check if new version exceeds maximum
            if new_version > self.max_version {
                return Err(UpcasterError::InvalidTargetVersion(new_version));
            }

            // Mark this version as visited BEFORE continuing
            visited.insert(current_version, true);

            // Update current state for next iteration
            current_version = new_version;
            current_payload = output_obj
                .get("payload")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
        }

        Ok(EventEnvelope {
            version: current_version,
            instance_id: envelope.instance_id,
            sequence: envelope.sequence,
            timestamp_ms: envelope.timestamp_ms,
            payload: current_payload,
            metadata: envelope.metadata,
        })
    }

    /// Get the current maximum supported version.
    fn max_supported_version(&self) -> u8 {
        self.max_version
    }
}

// =============================================================================
// Builder Implementation: DefaultUpcasterRegistryBuilder
// =============================================================================

/// Builder for constructing an UpcasterRegistryImpl.
pub struct DefaultUpcasterRegistryBuilder;

impl UpcasterRegistryBuilder for DefaultUpcasterRegistryBuilder {
    /// Build a new UpcasterRegistryImpl with MAX_SUPPORTED_VERSION as max version.
    fn build() -> Box<dyn UpcasterRegistry> {
        Box::new(UpcasterRegistryImpl::new(MAX_SUPPORTED_VERSION))
    }
}
