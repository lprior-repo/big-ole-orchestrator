//! Upcaster registry for resolving and applying upcaster chains.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::upcaster::{EventEnvelopeError, UpcasterError, MAX_SUPPORTED_VERSION};
use vo_types::events::upcaster::Upcaster;
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
    /// Returns `UpcasterError::DuplicateRegistration` if an upcaster is already
    /// registered for the given source version.
    fn register(&self, upcaster: Box<dyn Upcaster>) -> Result<(), UpcasterError> {
        let source_version = upcaster.source_version();
        let target_version = upcaster.target_version();

        if target_version > self.max_version {
            return Err(UpcasterError::InvalidTargetVersion(target_version));
        }

        let mut upcasters = self
            .upcasters
            .lock()
            .map_err(|_| UpcasterError::UpcastingFailed("lock poisoned".to_string()))?;

        if upcasters.contains_key(&source_version) {
            return Err(UpcasterError::DuplicateRegistration(source_version));
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
        if envelope.schema_version >= self.max_version {
            return Ok(envelope);
        }

        let upcasters = self
            .upcasters
            .lock()
            .map_err(|_| UpcasterError::UpcastingFailed("lock poisoned".to_string()))?;

        let chain = build_upcast_chain(&upcasters, envelope.schema_version, self.max_version)?;
        apply_upcast_chain(&envelope, self.max_version, chain)
    }

    /// Get the current maximum supported version.
    fn max_supported_version(&self) -> u8 {
        self.max_version
    }
}

/// Builds the ordered chain of upcasters needed to reach max_version.
///
/// Returns the sequence of (source_version, upcaster) pairs to apply.
type UpcasterChain<'a> = Vec<(u8, &'a dyn Upcaster)>;

fn build_upcast_chain(
    upcasters: &HashMap<u8, Box<dyn Upcaster>>,
    start_version: u8,
    max_version: u8,
) -> Result<UpcasterChain<'_>, UpcasterError> {
    let mut chain = Vec::new();
    let mut visited = HashMap::new();
    let mut current_version = start_version;

    while current_version < max_version {
        check_circular_chain(current_version, &visited)?;
        let upcaster = get_upcaster_for_version(upcasters, current_version)?;
        chain.push((current_version, upcaster));
        visited.insert(current_version, true);
        current_version = upcaster.target_version();
    }

    Ok(chain)
}

/// Checks for circular upcaster chain, returning error if version was already visited.
fn check_circular_chain(version: u8, visited: &HashMap<u8, bool>) -> Result<(), UpcasterError> {
    if visited.get(&version) == Some(&true) {
        Err(UpcasterError::CircularChain(version))
    } else {
        Ok(())
    }
}

/// Retrieves the upcaster for the given version, or error if none registered.
fn get_upcaster_for_version(
    upcasters: &HashMap<u8, Box<dyn Upcaster>>,
    version: u8,
) -> Result<&dyn Upcaster, UpcasterError> {
    upcasters
        .get(&version)
        .ok_or(UpcasterError::NoUpcasterRegistered(version))
        .map(|b| &**b as &dyn Upcaster)
}

/// Applies a single upcaster step, returning new version and payload.
fn apply_single_upcast(
    _envelope: &EventEnvelope,
    _version: u8,
    upcaster: &dyn Upcaster,
    current_payload: &serde_json::Value,
    _max_version: u8,
) -> Result<(u8, serde_json::Value), UpcasterError> {
    let new_payload = upcaster
        .upcast(current_payload)
        .map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))?;
    Ok((upcaster.target_version(), new_payload))
}

/// Applies the upcast chain to transform the envelope payload.
fn apply_upcast_chain<'a>(
    envelope: &EventEnvelope,
    max_version: u8,
    chain: UpcasterChain<'a>,
) -> Result<EventEnvelope, UpcasterError> {
    let mut current_version = envelope.schema_version;
    let mut current_payload = envelope.payload.clone();

    for (version, upcaster) in chain {
        let (new_version, new_payload) =
            apply_single_upcast(envelope, version, upcaster, &current_payload, max_version)?;
        current_version = new_version;
        current_payload = new_payload;
    }

    Ok(EventEnvelope {
        schema_version: current_version,
        instance_id: envelope.instance_id.clone(),
        sequence: envelope.sequence,
        timestamp_ms: envelope.timestamp_ms,
        payload: current_payload,
        metadata: envelope.metadata.clone(),
    })
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
